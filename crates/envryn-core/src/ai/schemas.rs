//! Strict response types for every AI feature.
//!
//! Every type here derives `Deserialize` with `#[serde(deny_unknown_fields)]`
//! and only ever holds enum-valued fields where the vault model already has
//! an enum -- so a model response is validated against the same domain
//! model the vault itself uses, not a separate looser shape
//! (`docs/AI_SECURITY.md` section 5). There is no `#[serde(default)]`
//! anywhere in this file: a field the model omitted is a malformed
//! response, not a value to guess at.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{Environment, SecretKind};

/// **The one type here that tolerates a *missing* field, and only a missing
/// one.** Every field is optional-by-absence: a 1.5B model asked for five
/// keys routinely returns three, and rejecting the whole response over an
/// omitted `tags: []` is why natural-language search answered "No match
/// found" essentially always -- the filter never survived parsing, so the
/// search never ran.
///
/// `deny_unknown_fields` is deliberately still here. The security property
/// it carries (a model cannot express a field, therefore cannot express an
/// action -- see `gateway`'s `a_fully_persuaded_model_still_cannot_express_an_action`)
/// is about *extra* fields and is completely unaffected by defaulting absent
/// ones. Tolerating an omission is a robustness fix; tolerating an addition
/// would have been a security regression, and is not what this does.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, default)]
#[ts(export)]
pub struct SearchFilterOutput {
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

impl SearchFilterOutput {
    /// True when this filter would not narrow anything -- every field empty.
    /// A model that returns `{}` (or that only echoed back nulls) has told
    /// us nothing, and the caller should fall back to plain matching rather
    /// than "filter by nothing", which would match the entire vault.
    pub fn is_empty(&self) -> bool {
        self.project.is_none()
            && self.environment.is_none()
            && self.kind.is_none()
            && self.tags.is_empty()
            && self.text.as_deref().is_none_or(|t| t.trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct EnvNameEntry {
    pub name: String,
    pub kind: SecretKind,
}

/// `names` defaults to empty for the same reason `SearchFilterOutput`'s
/// fields do: a model that returns `{}` has found nothing, which is a
/// legitimate answer, not a malformed response worth failing the whole
/// import over. `deny_unknown_fields` still rejects anything *extra*.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, default)]
#[ts(export)]
pub struct EnvNameClassificationOutput {
    pub names: Vec<EnvNameEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ClassificationOutput {
    pub kind: SecretKind,
    pub provider: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct NameSuggestionOutput {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ExtractedField {
    pub label: String,
    pub value: String,
}

/// See [`EnvNameClassificationOutput`] -- `fields` defaults to empty so "the
/// model found nothing" reads as an empty result the UI can explain, not as
/// a parse failure the user sees as a generic error.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, default)]
#[ts(export)]
pub struct ExtractedFieldsOutput {
    pub fields: Vec<ExtractedField>,
}
