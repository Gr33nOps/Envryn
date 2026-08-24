//! Vault data types.
//!
//! The central distinction here is between [`SecretRecord`] -- the whole
//! record, including secret material -- and [`SecretSummary`], which carries
//! everything needed to render a list and nothing that must stay hidden.
//!
//! Listing returns summaries. Obtaining a value requires a separate, explicit
//! call. That is specification section 24 ("results must never automatically
//! display secret values") expressed as a type rather than as a rule the UI is
//! trusted to follow: a list endpoint physically cannot leak a value, because
//! the type it returns has nowhere to put one.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::error::{Error, Result};

/// Opaque record identifier. UUIDv7, so ids sort by creation time without
/// carrying a separate sequence column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretId(pub Uuid);

impl SecretId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| Error::InvalidInput("malformed record id"))
    }
}

impl Default for SecretId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SecretId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Deployment environment. `Unassigned` is a first-class value rather than an
/// `Option`, because "this credential has no environment" is a state the
/// cleanup assistant reports on, not missing data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
    Unassigned,
}

impl Environment {
    pub fn as_str(self) -> &'static str {
        match self {
            Environment::Development => "Development",
            Environment::Staging => "Staging",
            Environment::Production => "Production",
            Environment::Unassigned => "Unassigned",
        }
    }
}

/// The kind of credential. Kept in sync with [`SecretPayload`] by
/// [`SecretPayload::kind`]; the UI's form-field map is generated from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecretKind {
    ApiKey,
    Token,
    EnvVar,
    Database,
    Ssh,
    OAuth,
    Webhook,
    Note,
    Custom,
}

/// The secret material itself.
///
/// A typed union rather than a flat string, because SSH and database
/// credentials are inherently multi-field. Modelling them as one blob of text
/// pushes parsing into the UI and makes structured import impossible.
///
/// Every variant zeroizes on drop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(tag = "kind")]
pub enum SecretPayload {
    ApiKey {
        value: String,
    },
    Token {
        value: String,
    },
    EnvVar {
        key: String,
        value: String,
    },
    Database {
        host: String,
        port: u16,
        database: String,
        username: String,
        password: String,
    },
    Ssh {
        private_key: String,
        passphrase: Option<String>,
        host: Option<String>,
        username: Option<String>,
    },
    OAuth {
        client_id: String,
        client_secret: String,
    },
    Webhook {
        endpoint: String,
        secret: String,
    },
    Note {
        body: String,
    },
    Custom {
        fields: Vec<CustomField>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
pub struct CustomField {
    pub label: String,
    pub value: String,
}

impl SecretPayload {
    pub fn kind(&self) -> SecretKind {
        match self {
            SecretPayload::ApiKey { .. } => SecretKind::ApiKey,
            SecretPayload::Token { .. } => SecretKind::Token,
            SecretPayload::EnvVar { .. } => SecretKind::EnvVar,
            SecretPayload::Database { .. } => SecretKind::Database,
            SecretPayload::Ssh { .. } => SecretKind::Ssh,
            SecretPayload::OAuth { .. } => SecretKind::OAuth,
            SecretPayload::Webhook { .. } => SecretKind::Webhook,
            SecretPayload::Note { .. } => SecretKind::Note,
            SecretPayload::Custom { .. } => SecretKind::Custom,
        }
    }

    /// The value used for duplicate fingerprinting.
    ///
    /// Returns the single field that identifies the credential. Multi-field
    /// payloads fingerprint the part that actually grants access -- the
    /// password, not the hostname -- so that the same database password used
    /// in two projects is recognised as a duplicate even when the hosts differ.
    pub fn fingerprint_material(&self) -> Option<&str> {
        match self {
            SecretPayload::ApiKey { value } | SecretPayload::Token { value } => Some(value),
            SecretPayload::EnvVar { value, .. } => Some(value),
            SecretPayload::Database { password, .. } => Some(password),
            SecretPayload::Ssh { private_key, .. } => Some(private_key),
            SecretPayload::OAuth { client_secret, .. } => Some(client_secret),
            SecretPayload::Webhook { secret, .. } => Some(secret),
            // A note is prose. Fingerprinting it would report duplicates for
            // any two notes that happen to share wording, which is noise.
            SecretPayload::Note { .. } => None,
            SecretPayload::Custom { fields } => fields.first().map(|f| f.value.as_str()),
        }
    }
}

/// A full record, secret material included.
///
/// Never leaves the Rust core in this form except through an explicit reveal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRecord {
    pub id: SecretId,
    pub name: String,
    pub project: String,
    pub environment: Environment,
    pub payload: SecretPayload,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub provider: Option<String>,
    /// Unix milliseconds.
    pub created_ms: i64,
    pub updated_ms: i64,
    /// Last rotation, for the review suggestions in specification section 26.
    #[serde(default)]
    pub rotated_ms: Option<i64>,
}

impl SecretRecord {
    pub fn summary(&self) -> SecretSummary {
        SecretSummary {
            id: self.id,
            name: self.name.clone(),
            kind: self.payload.kind(),
            project: self.project.clone(),
            environment: self.environment,
            provider: self.provider.clone(),
            tags: self.tags.clone(),
            has_notes: self.notes.as_ref().is_some_and(|n| !n.is_empty()),
            created_ms: self.created_ms,
            updated_ms: self.updated_ms,
            rotated_ms: self.rotated_ms,
        }
    }
}

/// Everything needed to render a record in a list, and nothing more.
///
/// Structurally incapable of carrying secret material: there is no field for
/// it. `has_notes` is a flag rather than the note text for the same reason --
/// a note can contain a credential (specification section 32).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretSummary {
    pub id: SecretId,
    pub name: String,
    pub kind: SecretKind,
    pub project: String,
    pub environment: Environment,
    pub provider: Option<String>,
    pub tags: Vec<String>,
    pub has_notes: bool,
    pub created_ms: i64,
    pub updated_ms: i64,
    pub rotated_ms: Option<i64>,
}

/// What a caller supplies to create a record. Timestamps and id are assigned
/// by the vault, never by the caller.
#[derive(Debug, Clone, Deserialize)]
pub struct NewSecret {
    pub name: String,
    pub project: String,
    pub environment: Environment,
    pub payload: SecretPayload,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub provider: Option<String>,
}

/// A partial update. `None` means "leave unchanged", which is why every field
/// is an `Option` even where the underlying field is already optional.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SecretUpdate {
    pub name: Option<String>,
    pub project: Option<String>,
    pub environment: Option<Environment>,
    pub payload: Option<SecretPayload>,
    pub notes: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub provider: Option<Option<String>>,
    /// Set true to stamp the rotation date as now.
    #[serde(default)]
    pub mark_rotated: bool,
}

/// Validation limits.
///
/// Bounded so that a malformed or hostile import cannot produce a record that
/// is impossible to display, or a single row large enough to be a denial of
/// service against unlock (which decrypts every record).
pub mod limits {
    pub const MAX_NAME: usize = 200;
    pub const MAX_PROJECT: usize = 120;
    pub const MAX_TAGS: usize = 32;
    pub const MAX_TAG: usize = 60;
    pub const MAX_NOTES: usize = 64 * 1024;
    pub const MAX_VALUE: usize = 256 * 1024;
}

pub(crate) fn validate_new(input: &NewSecret) -> Result<()> {
    validate_common(
        &input.name,
        &input.project,
        &input.tags,
        input.notes.as_deref(),
    )?;
    validate_payload(&input.payload)
}

fn validate_common(name: &str, project: &str, tags: &[String], notes: Option<&str>) -> Result<()> {
    if name.trim().is_empty() {
        return Err(Error::InvalidInput("a secret needs a name"));
    }
    if name.len() > limits::MAX_NAME {
        return Err(Error::InvalidInput("name is too long"));
    }
    if project.len() > limits::MAX_PROJECT {
        return Err(Error::InvalidInput("project name is too long"));
    }
    if tags.len() > limits::MAX_TAGS {
        return Err(Error::InvalidInput("too many tags"));
    }
    if tags.iter().any(|t| t.len() > limits::MAX_TAG) {
        return Err(Error::InvalidInput("tag is too long"));
    }
    if notes.is_some_and(|n| n.len() > limits::MAX_NOTES) {
        return Err(Error::InvalidInput("note is too long"));
    }
    Ok(())
}

pub(crate) fn validate_payload(payload: &SecretPayload) -> Result<()> {
    let too_long = |s: &str| s.len() > limits::MAX_VALUE;
    let ok = match payload {
        SecretPayload::ApiKey { value } | SecretPayload::Token { value } => !too_long(value),
        SecretPayload::EnvVar { key, value } => !too_long(key) && !too_long(value),
        SecretPayload::Database {
            host,
            database,
            username,
            password,
            ..
        } => ![host, database, username, password]
            .iter()
            .any(|s| too_long(s)),
        SecretPayload::Ssh {
            private_key,
            passphrase,
            host,
            username,
        } => {
            !too_long(private_key)
                && !passphrase.as_deref().is_some_and(too_long)
                && !host.as_deref().is_some_and(too_long)
                && !username.as_deref().is_some_and(too_long)
        }
        SecretPayload::OAuth {
            client_id,
            client_secret,
        } => !too_long(client_id) && !too_long(client_secret),
        SecretPayload::Webhook { endpoint, secret } => !too_long(endpoint) && !too_long(secret),
        SecretPayload::Note { body } => body.len() <= limits::MAX_NOTES,
        SecretPayload::Custom { fields } => {
            fields.len() <= limits::MAX_TAGS && !fields.iter().any(|f| too_long(&f.value))
        }
    };
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidInput("value exceeds the maximum size"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> SecretRecord {
        SecretRecord {
            id: SecretId::new(),
            name: "GROQ_API_KEY".into(),
            project: "Rescripto".into(),
            environment: Environment::Development,
            payload: SecretPayload::ApiKey {
                value: "gsk_super_secret".into(),
            },
            notes: Some("dev key".into()),
            tags: vec!["ai".into()],
            provider: Some("Groq".into()),
            created_ms: 1,
            updated_ms: 2,
            rotated_ms: None,
        }
    }

    /// The load-bearing property of the summary type: serialising it must not
    /// produce the secret value anywhere, no matter what the record holds.
    #[test]
    fn summary_never_carries_secret_material() {
        let rec = record();
        let json = serde_json::to_string(&rec.summary()).unwrap();
        assert!(!json.contains("gsk_super_secret"));
        assert!(
            !json.contains("dev key"),
            "note text leaked into the summary"
        );
        assert!(json.contains("GROQ_API_KEY"), "the name should be present");
    }

    #[test]
    fn summary_reports_notes_as_a_flag() {
        let mut rec = record();
        assert!(rec.summary().has_notes);
        rec.notes = None;
        assert!(!rec.summary().has_notes);
        rec.notes = Some(String::new());
        assert!(!rec.summary().has_notes, "an empty note is not a note");
    }

    #[test]
    fn payload_kind_matches_variant() {
        assert_eq!(
            SecretPayload::ApiKey { value: "x".into() }.kind(),
            SecretKind::ApiKey
        );
        assert_eq!(
            SecretPayload::Note { body: "x".into() }.kind(),
            SecretKind::Note
        );
    }

    /// Fingerprinting the password rather than the host means the same
    /// credential reused across two databases is still spotted.
    #[test]
    fn database_fingerprints_the_password() {
        let payload = SecretPayload::Database {
            host: "db.example.com".into(),
            port: 5432,
            database: "main".into(),
            username: "admin".into(),
            password: "hunter2".into(),
        };
        assert_eq!(payload.fingerprint_material(), Some("hunter2"));
    }

    #[test]
    fn notes_are_not_fingerprinted() {
        assert_eq!(
            SecretPayload::Note {
                body: "prose".into()
            }
            .fingerprint_material(),
            None
        );
    }

    #[test]
    fn record_round_trips_through_json() {
        let rec = record();
        let json = serde_json::to_vec(&rec).unwrap();
        let back: SecretRecord = serde_json::from_slice(&json).unwrap();
        assert_eq!(rec, back);
    }

    #[test]
    fn ids_are_unique_and_parse() {
        let a = SecretId::new();
        let b = SecretId::new();
        assert_ne!(a, b);
        assert_eq!(SecretId::parse(&a.to_string()).unwrap(), a);
        assert!(SecretId::parse("not-a-uuid").is_err());
    }

    #[test]
    fn blank_name_is_rejected() {
        let input = NewSecret {
            name: "   ".into(),
            project: "p".into(),
            environment: Environment::Unassigned,
            payload: SecretPayload::ApiKey { value: "v".into() },
            notes: None,
            tags: vec![],
            provider: None,
        };
        assert!(validate_new(&input).is_err());
    }

    #[test]
    fn oversized_value_is_rejected() {
        let huge = "x".repeat(limits::MAX_VALUE + 1);
        assert!(validate_payload(&SecretPayload::ApiKey { value: huge }).is_err());
    }

    #[test]
    fn too_many_tags_is_rejected() {
        let input = NewSecret {
            name: "n".into(),
            project: "p".into(),
            environment: Environment::Unassigned,
            payload: SecretPayload::ApiKey { value: "v".into() },
            notes: None,
            tags: (0..limits::MAX_TAGS + 1).map(|i| i.to_string()).collect(),
            provider: None,
        };
        assert!(validate_new(&input).is_err());
    }
}
