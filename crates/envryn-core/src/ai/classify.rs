//! Deterministic classification: known credential prefixes and shapes,
//! matched in plain Rust before any model is ever consulted.
//!
//! `docs/ARCHITECTURE.md` section 6 and `docs/AI_DATA_ACCESS.md` section 3:
//! "Classification runs a rules engine before the model... The AI is the
//! fallback for values the rules do not recognise, never the primary path."
//! This is why: matches here are instant, work with the AI subsystem fully
//! disabled or never installed, and never send a credential anywhere --
//! not even to a local model. [`crate::ai::gateway::AiGateway::classify_pasted_value`]
//! is the fallback for whatever this module returns `None` for.

use serde::Serialize;
use ts_rs::TS;

use crate::model::SecretKind;

/// A high-confidence classification. `provider` is a short, stable string
/// suitable for display and for [`crate::ai::gateway::AiGateway::suggest_name`]'s
/// input -- never inferred, only ever a literal match on a known prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct DeterministicMatch {
    pub kind: SecretKind,
    pub provider: Option<&'static str>,
}

const fn m(kind: SecretKind, provider: Option<&'static str>) -> DeterministicMatch {
    DeterministicMatch { kind, provider }
}

/// Ordered, most-specific-first. The first matching rule wins; callers get
/// exactly one classification, never a ranked list to guess between.
pub fn classify(value: &str) -> Option<DeterministicMatch> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }

    let prefix_rules: &[(&str, DeterministicMatch)] = &[
        ("sk-proj-", m(SecretKind::ApiKey, Some("OpenAI"))),
        ("sk-ant-", m(SecretKind::ApiKey, Some("Anthropic"))),
        ("github_pat_", m(SecretKind::Token, Some("GitHub"))),
        ("ghp_", m(SecretKind::Token, Some("GitHub"))),
        ("gho_", m(SecretKind::Token, Some("GitHub"))),
        ("ghs_", m(SecretKind::Token, Some("GitHub"))),
        ("gsk_", m(SecretKind::ApiKey, Some("Groq"))),
        ("xoxb-", m(SecretKind::Token, Some("Slack"))),
        ("xoxp-", m(SecretKind::Token, Some("Slack"))),
        ("xoxa-", m(SecretKind::Token, Some("Slack"))),
        ("AKIA", m(SecretKind::ApiKey, Some("AWS"))),
        ("ASIA", m(SecretKind::ApiKey, Some("AWS"))),
        ("AIza", m(SecretKind::ApiKey, Some("Google"))),
        ("SG.", m(SecretKind::ApiKey, Some("SendGrid"))),
        ("whsec_", m(SecretKind::Webhook, Some("Stripe"))),
        ("sk_live_", m(SecretKind::ApiKey, Some("Stripe"))),
        ("sk_test_", m(SecretKind::ApiKey, Some("Stripe"))),
        ("pk_live_", m(SecretKind::ApiKey, Some("Stripe"))),
        ("pk_test_", m(SecretKind::ApiKey, Some("Stripe"))),
        ("postgres://", m(SecretKind::Database, Some("PostgreSQL"))),
        ("postgresql://", m(SecretKind::Database, Some("PostgreSQL"))),
        ("mysql://", m(SecretKind::Database, Some("MySQL"))),
        ("redis://", m(SecretKind::Database, Some("Redis"))),
        ("rediss://", m(SecretKind::Database, Some("Redis"))),
        ("mongodb+srv://", m(SecretKind::Database, Some("MongoDB"))),
        ("mongodb://", m(SecretKind::Database, Some("MongoDB"))),
    ];

    for (prefix, result) in prefix_rules {
        if v.starts_with(prefix) {
            return Some(*result);
        }
    }

    if is_pem_private_key(v) {
        return Some(m(SecretKind::Ssh, None));
    }

    if is_jwt_shape(v) {
        return Some(m(SecretKind::Token, Some("JWT")));
    }

    None
}

fn is_pem_private_key(v: &str) -> bool {
    v.starts_with("-----BEGIN ") && v.contains("PRIVATE KEY-----")
}

/// Three dot-separated segments, each non-empty and base64url-alphabet
/// (`A-Za-z0-9-_`), with no attempt to decode or verify a signature -- shape
/// detection only. A JWT is not secret by construction (many are meant to be
/// shown to a browser), but Envryn treats one pasted into the vault as
/// credential material worth classifying and protecting like any other
/// token; verifying it is out of scope for a classifier that never contacts
/// a network.
fn is_jwt_shape(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    let is_b64url = |s: &str| {
        !s.is_empty()
            && s.len() >= 4
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    parts.iter().all(|p| is_b64url(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_known_prefixes() {
        let cases: &[(&str, SecretKind, Option<&str>)] = &[
            ("sk-proj-abc123XYZ", SecretKind::ApiKey, Some("OpenAI")),
            ("sk-ant-api03-xyz", SecretKind::ApiKey, Some("Anthropic")),
            ("ghp_1234567890abcdef", SecretKind::Token, Some("GitHub")),
            (
                "github_pat_11ABCDEFG_xyz",
                SecretKind::Token,
                Some("GitHub"),
            ),
            ("xoxb-1234-5678-abcdef", SecretKind::Token, Some("Slack")),
            ("AKIAIOSFODNN7EXAMPLE", SecretKind::ApiKey, Some("AWS")),
            (
                "AIzaSyA1234567890abcdefghijklmno",
                SecretKind::ApiKey,
                Some("Google"),
            ),
            ("SG.abc123.def456", SecretKind::ApiKey, Some("SendGrid")),
            ("whsec_abc123def456", SecretKind::Webhook, Some("Stripe")),
            ("sk_live_51ABC", SecretKind::ApiKey, Some("Stripe")),
            (
                "postgres://user:pass@host:5432/db",
                SecretKind::Database,
                Some("PostgreSQL"),
            ),
            (
                "mongodb+srv://user:pass@cluster.mongodb.net/db",
                SecretKind::Database,
                Some("MongoDB"),
            ),
        ];
        for (value, kind, provider) in cases {
            let got = classify(value).unwrap_or_else(|| panic!("expected a match for {value}"));
            assert_eq!(got.kind, *kind, "wrong kind for {value}");
            assert_eq!(got.provider, *provider, "wrong provider for {value}");
        }
    }

    #[test]
    fn recognises_a_pem_private_key() {
        let pem = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk\n-----END OPENSSH PRIVATE KEY-----";
        assert_eq!(classify(pem).unwrap().kind, SecretKind::Ssh);
    }

    #[test]
    fn recognises_jwt_shape() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dGhpc2lzYXNpZ25hdHVyZQ";
        let got = classify(jwt).unwrap();
        assert_eq!(got.kind, SecretKind::Token);
        assert_eq!(got.provider, Some("JWT"));
    }

    #[test]
    fn a_two_segment_string_is_not_mistaken_for_a_jwt() {
        assert!(!is_jwt_shape("only.two"));
    }

    #[test]
    fn an_unrecognised_value_returns_none() {
        assert!(classify("my totally ordinary note about lunch").is_none());
        assert!(classify("").is_none());
        assert!(classify("   ").is_none());
    }

    #[test]
    fn more_specific_prefixes_are_not_shadowed_by_shorter_ones() {
        // "sk_live_" (Stripe) must not be caught by any broader "sk" rule --
        // there isn't one, but this pins the ordering assumption the table
        // relies on so a future addition can't silently break it.
        assert_eq!(classify("sk_live_51ABC").unwrap().provider, Some("Stripe"));
    }

    #[test]
    fn whitespace_is_trimmed_before_matching() {
        assert_eq!(
            classify("  ghp_1234567890abcdef  ").unwrap().provider,
            Some("GitHub")
        );
    }
}
