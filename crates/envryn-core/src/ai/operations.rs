//! Every AI capability the product offers, and the exposure level fixed to
//! each one (`docs/AI_DATA_ACCESS.md` section 1). Adding a capability means
//! adding a variant here -- a small, obvious diff in one file, which is
//! exactly where a security reviewer should be forced to look.

/// How much vault data an operation is permitted to see.
///
/// Mirrors `docs/AI_DATA_ACCESS.md` section 1 exactly. There is no
/// `Forbidden` variant -- the whole point is that "the AI sees the whole
/// decrypted vault automatically" has no representation in this type at all,
/// so it cannot be reached by adding a match arm somewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExposureLevel {
    /// No vault data. Only what the user typed into the AI prompt itself.
    L0,
    /// Metadata only -- name, type, project, environment, tags, dates.
    /// Never a value.
    L1,
    /// One specific secret value, for one operation, for the shortest
    /// possible lifetime.
    L2,
    /// Several user-selected records, values included.
    L3,
}

/// One AI capability and the input it was actually given.
///
/// **The security property lives here.** Every variant carries either a
/// plain, bounded, caller-supplied value (never a handle into the vault) or
/// nothing sensitive at all. There is no variant that accepts "give me
/// everything" -- a caller wanting to analyse many records must name them
/// one at a time, and [`crate::ai::budgets`] caps how many a single
/// operation may include.
#[derive(Debug, Clone)]
pub enum AiOperation {
    /// L0. Parse a natural-language search query into a filter the vault
    /// engine executes -- the model never retrieves a record itself.
    ParseSearchIntent { query: String },

    /// L1. Classify `.env` variable **names** only, during import preview.
    /// The deterministic parser has already stripped the values before this
    /// is ever constructed -- see `docs/AI_DATA_ACCESS.md`'s Tier 1 table:
    /// "classification of `DATABASE_URL` follows from the name; the value
    /// adds nothing."
    ClassifyEnvNames { names: Vec<String> },

    /// L2. Classify a value the user just pasted into the create-secret
    /// form. There is no `SecretId` yet -- the record does not exist until
    /// the user saves it -- so this necessarily carries the value itself,
    /// not a reference. Bounded by `budgets::MAX_VALUE_BYTES`.
    ClassifyPastedValue { value: String },

    /// L2. Suggest a name for a value plus its already-detected provider.
    /// Same lifetime and bound as `ClassifyPastedValue`.
    SuggestName {
        value: String,
        provider: Option<String>,
    },

    /// L3. Extract structured fields from a block the user explicitly
    /// submitted (e.g. pasting a whole `.pem` bundle or a connection
    /// string). Bounded by `budgets::MAX_BLOCK_BYTES`.
    ExtractStructuredFields { block: String },
}

impl AiOperation {
    /// The exposure level for this operation. Fixed per variant -- not a
    /// runtime setting, and not something a caller can widen by constructing
    /// the operation differently.
    pub fn level(&self) -> ExposureLevel {
        match self {
            AiOperation::ParseSearchIntent { .. } => ExposureLevel::L0,
            AiOperation::ClassifyEnvNames { .. } => ExposureLevel::L1,
            AiOperation::ClassifyPastedValue { .. } => ExposureLevel::L2,
            AiOperation::SuggestName { .. } => ExposureLevel::L2,
            AiOperation::ExtractStructuredFields { .. } => ExposureLevel::L3,
        }
    }

    /// A stable name for logging (`docs/AI_SECURITY.md` section 6: only the
    /// operation name, never its content, is ever logged).
    pub fn name(&self) -> &'static str {
        match self {
            AiOperation::ParseSearchIntent { .. } => "parse_search_intent",
            AiOperation::ClassifyEnvNames { .. } => "classify_env_names",
            AiOperation::ClassifyPastedValue { .. } => "classify_pasted_value",
            AiOperation::SuggestName { .. } => "suggest_name",
            AiOperation::ExtractStructuredFields { .. } => "extract_structured_fields",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_operation_has_its_documented_level() {
        assert_eq!(
            AiOperation::ParseSearchIntent {
                query: "prod db".into()
            }
            .level(),
            ExposureLevel::L0
        );
        assert_eq!(
            AiOperation::ClassifyEnvNames {
                names: vec!["DATABASE_URL".into()]
            }
            .level(),
            ExposureLevel::L1
        );
        assert_eq!(
            AiOperation::ClassifyPastedValue {
                value: "sk-proj-abc".into()
            }
            .level(),
            ExposureLevel::L2
        );
        assert_eq!(
            AiOperation::SuggestName {
                value: "sk-proj-abc".into(),
                provider: Some("OpenAI".into())
            }
            .level(),
            ExposureLevel::L2
        );
        assert_eq!(
            AiOperation::ExtractStructuredFields {
                block: "host=..".into()
            }
            .level(),
            ExposureLevel::L3
        );
    }

    #[test]
    fn levels_order_from_least_to_most_exposure() {
        assert!(ExposureLevel::L0 < ExposureLevel::L1);
        assert!(ExposureLevel::L1 < ExposureLevel::L2);
        assert!(ExposureLevel::L2 < ExposureLevel::L3);
    }
}
