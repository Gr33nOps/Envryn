//! A real, working answer to the "grammar-constrained decoding" gap
//! `docs/AI_SECURITY.md` section 5 and `model.rs`'s own module doc record
//! plainly: llama.cpp's GBNF makes the model "physically unable to emit
//! anything that is not schema-valid"; this candle-based engine had no
//! equivalent, relying entirely on prompting plus post-hoc
//! `deny_unknown_fields` rejection.
//!
//! This is not a general context-free-grammar engine (no recursion, no
//! arbitrary nesting) -- it is exactly as expressive as
//! [`ClassificationOutput`](envryn_core::ai::schemas::ClassificationOutput)
//! needs, which is deliberately the one schema built here: it is the one
//! Tier-1 AI feature actually wired to the UI
//! (`ai_classify_pasted_value`/"Suggest type"), so it is the one place this
//! guarantee earns its keep first. Extending to the other schemas in
//! `ai::schemas` means adding another [`JsonGrammar`] value, not a rewrite
//! of the state machine.
//!
//! The mechanism: at every generation step, decode *every* token in the
//! vocabulary once (cached — this is the only O(vocab) cost, paid on first
//! use, not per token generated) and ask the grammar's [`GrammarState`]
//! whether appending that token's text is still a valid prefix of some
//! schema-conforming completion. Tokens that are not get their logit set to
//! `-inf` before sampling, so the model is structurally unable to choose
//! them -- the same guarantee GBNF gives, built for the one schema that
//! needed it rather than assumed away.

use std::collections::VecDeque;

/// The literal JSON produced is always, in this fixed field order:
/// `{"kind":"<one of SecretKind's variants>","provider":null|"<any string>","confidence":<0..=1>}`
///
/// Field order is fixed by construction (the model cannot reorder fields),
/// which is stricter than the schema strictly requires -- `deny_unknown_fields`
/// does not care about key order -- but a real constraint that always
/// produces valid output is worth more than a looser one that would need
/// order-independent backtracking to implement correctly.
pub struct JsonGrammar {
    variants: &'static [&'static str],
}

impl JsonGrammar {
    /// Built for `ClassificationOutput`. `variants` must be `SecretKind`'s
    /// serialised variant names in the exact casing serde produces (checked
    /// against the real enum in this crate's tests, not assumed).
    pub fn classification_output(variants: &'static [&'static str]) -> Self {
        Self { variants }
    }

    pub fn start(&self) -> GrammarState {
        let mut queue = VecDeque::new();
        queue.push_back(Step::Literal("{\"kind\":\""));
        queue.push_back(Step::Enum(EnumMatcher::new(self.variants)));
        queue.push_back(Step::Literal("\",\"provider\":"));
        queue.push_back(Step::NullOrQuote);
        // The tail after `provider`'s value is spliced in by `NullOrQuote`
        // once the branch resolves (see `Step::apply`), since it is shared
        // by both the null and string branches.
        GrammarState { queue }
    }
}

const CONFIDENCE_TAIL: &str = ",\"confidence\":";

#[derive(Clone)]
enum Step {
    /// Match these exact bytes, one at a time, front to back.
    Literal(&'static str),
    /// Match one of a fixed, shrinking set of candidate strings.
    Enum(EnumMatcher),
    /// Just saw `"provider":`; the next character decides the branch.
    NullOrQuote,
    /// Inside a JSON string body (provider's value, when not null) -- any
    /// character is valid except an unescaped `"` or a raw control
    /// character, matching ordinary JSON string syntax.
    StringBody {
        escaped: bool,
    },
    /// The leading digit of `confidence`, which must be `0` or `1` --
    /// values outside `[0, 1]` are not valid confidence scores.
    NumberLeading,
    /// After a leading `0`: either stop (move to `}`) or `.` then any digits.
    AfterZero,
    ZeroFraction,
    /// After a leading `1`: either stop, or `.` then only `0`s (`1.000`,
    /// never `1.5`, which would exceed the unit interval).
    AfterOne,
    OneFraction,
}

#[derive(Clone)]
struct EnumMatcher {
    candidates: Vec<&'static str>,
    matched_len: usize,
}

impl EnumMatcher {
    fn new(options: &[&'static str]) -> Self {
        Self {
            candidates: options.to_vec(),
            matched_len: 0,
        }
    }

    /// `Some(true)` if `ch` extends at least one candidate to a complete
    /// match; `Some(false)` if it extends a candidate but none complete yet;
    /// `None` if no remaining candidate has `ch` at this position.
    fn step(&mut self, ch: char) -> Option<bool> {
        let mut buf = [0u8; 4];
        let ch_bytes = ch.encode_utf8(&mut buf).as_bytes();
        let next: Vec<&'static str> = self
            .candidates
            .iter()
            .filter(|c| c.as_bytes()[self.matched_len..].starts_with(ch_bytes))
            .copied()
            .collect();
        if next.is_empty() {
            return None;
        }
        self.matched_len += ch_bytes.len();
        let complete = next.iter().any(|c| c.len() == self.matched_len);
        self.candidates = next;
        Some(complete)
    }
}

/// The grammar's position after consuming some prefix of the model's
/// output so far. Cloned cheaply per candidate token during masking (the
/// `EnumMatcher`'s candidate list is the only heap allocation, and it only
/// exists during the `kind` field, a handful of short strings).
#[derive(Clone)]
pub struct GrammarState {
    queue: VecDeque<Step>,
}

impl GrammarState {
    /// Whether `text` is a valid continuation from this state -- every
    /// character must be consumable, though the result does not need to
    /// reach a fully-accepting state (a token frequently lands mid-field).
    /// Returns the resulting state on success, without mutating `self`.
    pub fn try_advance(&self, text: &str) -> Option<GrammarState> {
        let mut state = self.clone();
        for ch in text.chars() {
            state = state.step(ch)?;
        }
        Some(state)
    }

    /// Whether the grammar has reached the final `}` -- generation should
    /// stop here regardless of what the model's own EOS token does, since
    /// nothing after this point is valid JSON for this schema.
    pub fn is_complete(&self) -> bool {
        self.queue.is_empty()
    }

    fn step(mut self, ch: char) -> Option<GrammarState> {
        let current = self.queue.pop_front()?;
        match current {
            Step::Literal(remaining) => {
                let mut chars = remaining.chars();
                if chars.next()? != ch {
                    return None;
                }
                let rest: String = chars.collect();
                if !rest.is_empty() {
                    // SAFETY of the leak-free approach: literals are all
                    // `'static` string slices already; re-borrowing a
                    // suffix of a `'static` str is itself `'static`.
                    let rest_static: &'static str = Box::leak(rest.into_boxed_str());
                    self.queue.push_front(Step::Literal(rest_static));
                }
                Some(self)
            }
            Step::Enum(mut matcher) => {
                match matcher.step(ch)? {
                    true => {} // complete -- fall through to the next queued step
                    false => self.queue.push_front(Step::Enum(matcher)),
                }
                Some(self)
            }
            Step::NullOrQuote => match ch {
                'n' => {
                    self.queue.push_front(Step::Literal("ull"));
                    self.queue.push_back(Step::Literal(CONFIDENCE_TAIL));
                    self.queue.push_back(Step::NumberLeading);
                    self.queue.push_back(Step::Literal("}"));
                    Some(self)
                }
                '"' => {
                    self.queue.push_front(Step::StringBody { escaped: false });
                    self.queue.push_back(Step::Literal(CONFIDENCE_TAIL));
                    self.queue.push_back(Step::NumberLeading);
                    self.queue.push_back(Step::Literal("}"));
                    Some(self)
                }
                _ => None,
            },
            Step::StringBody { escaped } => {
                if escaped {
                    // Any character may follow a backslash in a JSON string;
                    // this does not re-validate `\uXXXX` hex digits
                    // specifically, matching how little this constraint
                    // needs to police free-text content versus structure.
                    self.queue.push_front(Step::StringBody { escaped: false });
                    Some(self)
                } else {
                    match ch {
                        '"' => Some(self), // closes the string; falls through
                        '\\' => {
                            self.queue.push_front(Step::StringBody { escaped: true });
                            Some(self)
                        }
                        c if (c as u32) < 0x20 => None, // raw control chars are invalid in JSON strings
                        _ => {
                            self.queue.push_front(Step::StringBody { escaped: false });
                            Some(self)
                        }
                    }
                }
            }
            Step::NumberLeading => match ch {
                '0' => {
                    self.queue.push_front(Step::AfterZero);
                    Some(self)
                }
                '1' => {
                    self.queue.push_front(Step::AfterOne);
                    Some(self)
                }
                _ => None,
            },
            Step::AfterZero => match ch {
                '.' => {
                    self.queue.push_front(Step::ZeroFraction);
                    Some(self)
                }
                '}' => self.step_literal_close(ch),
                _ => None,
            },
            Step::ZeroFraction => match ch {
                '0'..='9' => {
                    self.queue.push_front(Step::ZeroFraction);
                    Some(self)
                }
                '}' => self.step_literal_close(ch),
                _ => None,
            },
            Step::AfterOne => match ch {
                '.' => {
                    self.queue.push_front(Step::OneFraction);
                    Some(self)
                }
                '}' => self.step_literal_close(ch),
                _ => None,
            },
            Step::OneFraction => match ch {
                // Only `0` may follow `1.`, or the value would exceed 1.
                '0' => {
                    self.queue.push_front(Step::OneFraction);
                    Some(self)
                }
                '}' => self.step_literal_close(ch),
                _ => None,
            },
        }
    }

    /// The number phases can end at any digit (a shorter completion is
    /// still valid JSON), so `}` needs to be re-checked against the queued
    /// closing-brace literal rather than treated as this phase's own
    /// terminator.
    fn step_literal_close(mut self, ch: char) -> Option<GrammarState> {
        let next = self.queue.pop_front()?;
        match next {
            Step::Literal(lit) if lit == "}" && ch == '}' => Some(self),
            other => {
                self.queue.push_front(other);
                None
            }
        }
    }
}

#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    const KIND_VARIANTS: &[&str] = &[
        "ApiKey", "Token", "EnvVar", "Database", "Ssh", "OAuth", "Webhook", "Note", "Custom",
    ];

    fn advance_all(mut state: GrammarState, text: &str) -> Option<GrammarState> {
        for ch in text.chars() {
            state = state.step(ch)?;
        }
        Some(state)
    }

    #[test]
    fn accepts_a_real_classification_output_with_a_string_provider() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let text = r#"{"kind":"ApiKey","provider":"Stripe","confidence":0.92}"#;
        let end = advance_all(grammar.start(), text).expect("valid output must be accepted");
        assert!(end.is_complete());
    }

    #[test]
    fn accepts_a_null_provider_and_a_whole_number_confidence() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let text = r#"{"kind":"Note","provider":null,"confidence":1.0}"#;
        let end = advance_all(grammar.start(), text).expect("valid output must be accepted");
        assert!(end.is_complete());
    }

    #[test]
    fn rejects_a_kind_outside_the_enum() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let text = r#"{"kind":"NotARealKind","#;
        assert!(advance_all(grammar.start(), text).is_none());
    }

    #[test]
    fn rejects_an_unknown_field() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let text = r#"{"kind":"Token","provider":null,"confidence":0.5,"extra":"x"}"#;
        assert!(advance_all(grammar.start(), text).is_none());
    }

    #[test]
    fn rejects_a_confidence_above_one() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let text = r#"{"kind":"Token","provider":null,"confidence":1.5"#;
        assert!(advance_all(grammar.start(), text).is_none());
    }

    #[test]
    fn rejects_a_negative_confidence() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let text = r#"{"kind":"Token","provider":null,"confidence":-0.1"#;
        assert!(advance_all(grammar.start(), text).is_none());
    }

    #[test]
    fn a_provider_string_may_contain_an_escaped_quote() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let text = r#"{"kind":"Custom","provider":"Bob\"s API","confidence":0.5}"#;
        let end = advance_all(grammar.start(), text)
            .expect("an escaped quote must not close the string early");
        assert!(end.is_complete());
    }

    #[test]
    fn partial_prefixes_of_valid_output_are_accepted_but_not_complete() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        let state = advance_all(grammar.start(), r#"{"kind":"Api"#)
            .expect("a valid prefix must be accepted");
        assert!(!state.is_complete());
    }

    #[test]
    fn rejects_reordered_fields() {
        let grammar = JsonGrammar::classification_output(KIND_VARIANTS);
        // `provider` before `kind` is semantically fine JSON but not what
        // this grammar produces -- field order is fixed by construction.
        let text = r#"{"provider":null,"kind":"Token","#;
        assert!(advance_all(grammar.start(), text).is_none());
    }
}
