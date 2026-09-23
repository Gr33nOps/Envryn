//! Search-query parsing: turn a typed query into a structured filter.
//!
//! "production stripe keys" becomes environment = Production plus the free
//! text "stripe keys", using exact word lists for the environments and secret
//! kinds the vault actually has. It is instant and offline, and anything it
//! does not recognise stays as free text, so a partly understood query still
//! narrows the list instead of returning nothing.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{Environment, SecretKind};

/// The structured part of a search query. Every field is optional: a query
/// that names only an environment leaves the rest empty.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct SearchFilter {
    #[ts(optional = nullable)]
    pub project: Option<String>,
    #[ts(optional = nullable)]
    pub environment: Option<Environment>,
    #[ts(optional = nullable)]
    pub kind: Option<SecretKind>,
    pub tags: Vec<String>,
    #[ts(optional = nullable)]
    pub text: Option<String>,
}

impl SearchFilter {
    /// True when this filter would not narrow anything, so the caller should
    /// fall back to plain matching rather than "filter by nothing".
    pub fn is_empty(&self) -> bool {
        self.project.is_none()
            && self.environment.is_none()
            && self.kind.is_none()
            && self.tags.is_empty()
            && self.text.as_deref().is_none_or(|t| t.trim().is_empty())
    }
}

/// Words that map onto an [`Environment`], including the shorthands people
/// actually type. Matched case-insensitively against whole words only.
const ENVIRONMENT_WORDS: &[(&str, Environment)] = &[
    ("production", Environment::Production),
    ("prod", Environment::Production),
    ("live", Environment::Production),
    ("staging", Environment::Staging),
    ("stage", Environment::Staging),
    ("development", Environment::Development),
    ("dev", Environment::Development),
    ("local", Environment::Development),
    ("unassigned", Environment::Unassigned),
];

/// Words that map onto a [`SecretKind`]. Multi-word entries are matched as
/// phrases before single words, so "api key" beats a bare "key".
const KIND_PHRASES: &[(&str, SecretKind)] = &[
    ("api key", SecretKind::ApiKey),
    ("api keys", SecretKind::ApiKey),
    ("apikey", SecretKind::ApiKey),
    ("access token", SecretKind::Token),
    ("secure note", SecretKind::Note),
    ("connection string", SecretKind::Database),
    ("env var", SecretKind::EnvVar),
    ("environment variable", SecretKind::EnvVar),
    ("ssh key", SecretKind::Ssh),
    ("private key", SecretKind::Ssh),
];

const KIND_WORDS: &[(&str, SecretKind)] = &[
    ("token", SecretKind::Token),
    ("tokens", SecretKind::Token),
    ("database", SecretKind::Database),
    ("databases", SecretKind::Database),
    ("db", SecretKind::Database),
    ("postgres", SecretKind::Database),
    ("mysql", SecretKind::Database),
    ("mongo", SecretKind::Database),
    ("redis", SecretKind::Database),
    ("ssh", SecretKind::Ssh),
    ("oauth", SecretKind::OAuth),
    ("webhook", SecretKind::Webhook),
    ("webhooks", SecretKind::Webhook),
    ("note", SecretKind::Note),
    ("notes", SecretKind::Note),
];

/// Noise words that carry no filtering meaning. Stripped from the residual
/// free-text term so "show me my production stripe keys" leaves "stripe",
/// not "show me my stripe".
const STOP_WORDS: &[&str] = &[
    "show", "me", "my", "the", "a", "an", "all", "find", "search", "for", "get", "list", "give",
    "please", "what", "whats", "where", "is", "are", "in", "on", "of", "to", "and", "with", "any",
    "some", "that", "this", "i", "have", "has", "do", "does", "can", "you",
];

/// Parse what can be parsed with certainty. Anything not recognised is left
/// in [`SearchFilter::text`] for substring matching, so a query this
/// function only partly understands still narrows correctly rather than
/// returning nothing.
pub fn parse_query(query: &str) -> SearchFilter {
    let lowered = query.trim().to_lowercase();
    if lowered.is_empty() {
        return SearchFilter::default();
    }

    let mut remaining = lowered.clone();
    let mut filter = SearchFilter::default();

    // Phrases first: "api key" must be consumed before the bare word "key"
    // or "api" can be considered separately.
    for (phrase, kind) in KIND_PHRASES {
        if filter.kind.is_none() && contains_word_sequence(&remaining, phrase) {
            filter.kind = Some(*kind);
            remaining = remove_word_sequence(&remaining, phrase);
        }
    }

    let mut residual: Vec<String> = Vec::new();
    for word in remaining.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        if cleaned.is_empty() {
            continue;
        }

        if let Some((_, env)) = ENVIRONMENT_WORDS.iter().find(|(w, _)| *w == cleaned) {
            if filter.environment.is_none() {
                filter.environment = Some(*env);
                continue;
            }
        }
        if let Some((_, kind)) = KIND_WORDS.iter().find(|(w, _)| *w == cleaned) {
            if filter.kind.is_none() {
                filter.kind = Some(*kind);
                continue;
            }
        }
        if STOP_WORDS.contains(&cleaned) {
            continue;
        }
        residual.push(cleaned.to_string());
    }

    if !residual.is_empty() {
        filter.text = Some(residual.join(" "));
    }
    filter
}

/// True when `query` contains `phrase` as a whole-word sequence.
fn contains_word_sequence(query: &str, phrase: &str) -> bool {
    let q: Vec<&str> = query.split_whitespace().collect();
    let p: Vec<&str> = phrase.split_whitespace().collect();
    if p.is_empty() || p.len() > q.len() {
        return false;
    }
    q.windows(p.len()).any(|w| w == p.as_slice())
}

fn remove_word_sequence(query: &str, phrase: &str) -> String {
    let q: Vec<&str> = query.split_whitespace().collect();
    let p: Vec<&str> = phrase.split_whitespace().collect();
    if p.is_empty() {
        return query.to_string();
    }
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    // `get`-based rather than indexed: this crate denies `indexing_slicing`.
    while let Some(word) = q.get(i) {
        let matches_here = q.get(i..i + p.len()).is_some_and(|w| w == p.as_slice());
        if matches_here {
            i += p.len();
        } else {
            out.push(word);
            i += 1;
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_environment_word_becomes_a_structured_filter() {
        for (word, expected) in [
            ("production", Environment::Production),
            ("prod", Environment::Production),
            ("staging", Environment::Staging),
            ("dev", Environment::Development),
        ] {
            let f = parse_query(word);
            assert_eq!(f.environment, Some(expected), "for {word}");
        }
    }

    #[test]
    fn a_kind_word_becomes_a_structured_filter() {
        assert_eq!(parse_query("tokens").kind, Some(SecretKind::Token));
        assert_eq!(parse_query("databases").kind, Some(SecretKind::Database));
        assert_eq!(parse_query("webhook").kind, Some(SecretKind::Webhook));
    }

    #[test]
    fn a_multi_word_kind_phrase_beats_its_component_words() {
        let f = parse_query("api key");
        assert_eq!(f.kind, Some(SecretKind::ApiKey));
        // "api" and "key" must both have been consumed by the phrase, not
        // left behind to be substring-matched against every record name.
        assert_eq!(f.text, None);
    }

    /// A sentence a person would actually type resolves to exact
    /// structured filters plus one residual term.
    #[test]
    fn a_natural_sentence_resolves_to_filters() {
        let f = parse_query("show me my production stripe keys");
        assert_eq!(f.environment, Some(Environment::Production));
        assert_eq!(f.text.as_deref(), Some("stripe keys"));

        let f = parse_query("all staging database credentials");
        assert_eq!(f.environment, Some(Environment::Staging));
        assert_eq!(f.kind, Some(SecretKind::Database));
        assert_eq!(f.text.as_deref(), Some("credentials"));
    }

    #[test]
    fn a_bare_provider_name_is_left_as_free_text() {
        let f = parse_query("openrouter");
        assert_eq!(f.text.as_deref(), Some("openrouter"));
        assert_eq!(f.environment, None);
        assert_eq!(f.kind, None);
    }

    #[test]
    fn stop_words_alone_produce_an_empty_filter() {
        assert!(parse_query("show me all the").is_empty());
        assert!(parse_query("").is_empty());
        assert!(parse_query("   ").is_empty());
    }

    #[test]
    fn parsing_is_case_insensitive() {
        let f = parse_query("PRODUCTION Tokens");
        assert_eq!(f.environment, Some(Environment::Production));
        assert_eq!(f.kind, Some(SecretKind::Token));
    }

    #[test]
    fn an_unrecognised_query_still_yields_usable_free_text() {
        let f = parse_query("acme-payments");
        assert_eq!(f.text.as_deref(), Some("acme-payments"));
        assert!(!f.is_empty());
    }
}
