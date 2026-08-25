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

use crate::model::{Environment, SecretKind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchFilterOutput {
    pub project: Option<String>,
    pub environment: Option<Environment>,
    pub kind: Option<SecretKind>,
    pub tags: Vec<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvNameEntry {
    pub name: String,
    pub kind: SecretKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvNameClassificationOutput {
    pub names: Vec<EnvNameEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassificationOutput {
    pub kind: SecretKind,
    pub provider: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameSuggestionOutput {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractedField {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractedFieldsOutput {
    pub fields: Vec<ExtractedField>,
}
