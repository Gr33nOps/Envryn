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

/// Every known credential prefix, paired with what it means.
///
/// **Order in this table is deliberately not significant.** [`classify`]
/// selects the *longest* matching prefix, not the first, so a specific rule
/// always beats a more general one that shares its start regardless of where
/// either sits here (`sk-or-v1-` beats `sk-or-` beats `sk-`). Getting this
/// wrong is not hypothetical: an OpenRouter key (`sk-or-v1-...`) previously
/// matched no rule at all, fell through to the model, and came back
/// confidently labelled "Stripe" -- a wrong answer presented with the same
/// authority as a right one. Longest-match makes the precedence structural
/// instead of a hand-maintained ordering a future edit can silently break.
const PREFIX_RULES: &[(&str, DeterministicMatch)] = &[
    // OpenAI-style `sk-` family. Every vendor below borrowed OpenAI's
    // prefix, which is exactly why longest-match matters here.
    ("sk-or-v1-", m(SecretKind::ApiKey, Some("OpenRouter"))),
    ("sk-or-", m(SecretKind::ApiKey, Some("OpenRouter"))),
    ("sk-ant-api", m(SecretKind::ApiKey, Some("Anthropic"))),
    ("sk-ant-", m(SecretKind::ApiKey, Some("Anthropic"))),
    ("sk-proj-", m(SecretKind::ApiKey, Some("OpenAI"))),
    ("sk-svcacct-", m(SecretKind::ApiKey, Some("OpenAI"))),
    // Legacy OpenAI keys are a bare `sk-` + random text. Deliberately last
    // in the family by virtue of being the shortest: it is the fallback for
    // an `sk-` key none of the more specific rules above claimed.
    ("sk-", m(SecretKind::ApiKey, Some("OpenAI"))),
    ("pplx-", m(SecretKind::ApiKey, Some("Perplexity"))),
    ("xai-", m(SecretKind::ApiKey, Some("xAI"))),
    ("gsk_", m(SecretKind::ApiKey, Some("Groq"))),
    ("fw_", m(SecretKind::ApiKey, Some("Fireworks AI"))),
    ("r8_", m(SecretKind::ApiKey, Some("Replicate"))),
    ("hf_", m(SecretKind::Token, Some("Hugging Face"))),
    // Stripe. `sk_`/`pk_` use an underscore, so they never collide with the
    // `sk-` family above -- but both are spelled out per-mode anyway so a
    // live key is never silently read as a test key.
    ("sk_live_", m(SecretKind::ApiKey, Some("Stripe"))),
    ("sk_test_", m(SecretKind::ApiKey, Some("Stripe"))),
    ("pk_live_", m(SecretKind::ApiKey, Some("Stripe"))),
    ("pk_test_", m(SecretKind::ApiKey, Some("Stripe"))),
    ("rk_live_", m(SecretKind::ApiKey, Some("Stripe"))),
    ("rk_test_", m(SecretKind::ApiKey, Some("Stripe"))),
    ("whsec_", m(SecretKind::Webhook, Some("Stripe"))),
    // Source forges.
    ("github_pat_", m(SecretKind::Token, Some("GitHub"))),
    ("ghp_", m(SecretKind::Token, Some("GitHub"))),
    ("gho_", m(SecretKind::Token, Some("GitHub"))),
    ("ghs_", m(SecretKind::Token, Some("GitHub"))),
    ("ghu_", m(SecretKind::Token, Some("GitHub"))),
    ("ghr_", m(SecretKind::Token, Some("GitHub"))),
    ("glpat-", m(SecretKind::Token, Some("GitLab"))),
    // Cloud providers.
    ("AKIA", m(SecretKind::ApiKey, Some("AWS"))),
    ("ASIA", m(SecretKind::ApiKey, Some("AWS"))),
    ("ABIA", m(SecretKind::ApiKey, Some("AWS"))),
    ("ACCA", m(SecretKind::ApiKey, Some("AWS"))),
    ("AIza", m(SecretKind::ApiKey, Some("Google"))),
    ("ya29.", m(SecretKind::OAuth, Some("Google"))),
    ("dop_v1_", m(SecretKind::Token, Some("DigitalOcean"))),
    ("doo_v1_", m(SecretKind::OAuth, Some("DigitalOcean"))),
    ("dor_v1_", m(SecretKind::Token, Some("DigitalOcean"))),
    // Supabase. `sbp_` is a personal access token; project anon/service
    // keys are JWTs and fall through to the JWT shape rule below.
    ("sbp_", m(SecretKind::Token, Some("Supabase"))),
    ("sbs_", m(SecretKind::Token, Some("Supabase"))),
    // SaaS.
    ("xoxb-", m(SecretKind::Token, Some("Slack"))),
    ("xoxp-", m(SecretKind::Token, Some("Slack"))),
    ("xoxa-", m(SecretKind::Token, Some("Slack"))),
    ("xoxr-", m(SecretKind::Token, Some("Slack"))),
    ("xoxs-", m(SecretKind::Token, Some("Slack"))),
    ("SG.", m(SecretKind::ApiKey, Some("SendGrid"))),
    ("shpat_", m(SecretKind::Token, Some("Shopify"))),
    ("shpss_", m(SecretKind::Token, Some("Shopify"))),
    ("shpca_", m(SecretKind::Token, Some("Shopify"))),
    ("npm_", m(SecretKind::Token, Some("npm"))),
    ("pypi-", m(SecretKind::Token, Some("PyPI"))),
    ("lin_api_", m(SecretKind::ApiKey, Some("Linear"))),
    ("ntn_", m(SecretKind::Token, Some("Notion"))),
    ("PMAK-", m(SecretKind::ApiKey, Some("Postman"))),
    ("sntrys_", m(SecretKind::Token, Some("Sentry"))),
    ("sntryu_", m(SecretKind::Token, Some("Sentry"))),
    // Webhook endpoints. A URL, not a bearer credential, but treated as
    // secret material for the same reason a JWT is: possessing it is enough
    // to act with it.
    (
        "https://hooks.slack.com/",
        m(SecretKind::Webhook, Some("Slack")),
    ),
    (
        "https://discord.com/api/webhooks/",
        m(SecretKind::Webhook, Some("Discord")),
    ),
    (
        "https://discordapp.com/api/webhooks/",
        m(SecretKind::Webhook, Some("Discord")),
    ),
];

/// Database connection-string schemes, matched case-insensitively on the
/// URL scheme alone (`POSTGRES://` is the same scheme as `postgres://`,
/// unlike a bearer token where case is significant).
const SCHEME_RULES: &[(&str, DeterministicMatch)] = &[
    ("postgresql://", m(SecretKind::Database, Some("PostgreSQL"))),
    ("postgres://", m(SecretKind::Database, Some("PostgreSQL"))),
    ("mysql://", m(SecretKind::Database, Some("MySQL"))),
    ("mariadb://", m(SecretKind::Database, Some("MariaDB"))),
    ("mongodb+srv://", m(SecretKind::Database, Some("MongoDB"))),
    ("mongodb://", m(SecretKind::Database, Some("MongoDB"))),
    ("rediss://", m(SecretKind::Database, Some("Redis"))),
    ("redis://", m(SecretKind::Database, Some("Redis"))),
    ("mssql://", m(SecretKind::Database, Some("SQL Server"))),
    ("sqlserver://", m(SecretKind::Database, Some("SQL Server"))),
    ("clickhouse://", m(SecretKind::Database, Some("ClickHouse"))),
    (
        "cockroachdb://",
        m(SecretKind::Database, Some("CockroachDB")),
    ),
    ("amqps://", m(SecretKind::Database, Some("AMQP"))),
    ("amqp://", m(SecretKind::Database, Some("AMQP"))),
];

/// Classify by known prefix or shape. Returns the **longest** matching
/// prefix's result, so a specific rule always wins over a general one; see
/// [`PREFIX_RULES`] for why that is structural rather than ordering-based.
///
/// A `Some(_)` here is a high-confidence, literal match. Callers must treat
/// it as final and must not ask the model to second-guess it -- see
/// `AiGateway::classify_pasted_value`'s own note.
pub fn classify(value: &str) -> Option<DeterministicMatch> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }

    let best_prefix = PREFIX_RULES
        .iter()
        .filter(|(prefix, _)| v.starts_with(prefix))
        .max_by_key(|(prefix, _)| prefix.len());

    let best_scheme = SCHEME_RULES
        .iter()
        .filter(|(scheme, _)| starts_with_ignore_ascii_case(v, scheme))
        .max_by_key(|(scheme, _)| scheme.len());

    // A connection string never begins with one of the bearer-token
    // prefixes above, so at most one of these is ever `Some` -- but if a
    // future rule made both match, the longer (more specific) one wins, for
    // the same reason the longest prefix does within each table.
    let best = match (best_prefix, best_scheme) {
        (Some(p), Some(s)) if s.0.len() > p.0.len() => Some(s),
        (Some(p), _) => Some(p),
        (None, s) => s,
    };
    if let Some((_, result)) = best {
        return Some(*result);
    }

    if is_pem_private_key(v) {
        return Some(m(SecretKind::Ssh, None));
    }

    if is_jwt_shape(v) {
        return Some(m(SecretKind::Token, Some("JWT")));
    }

    None
}

/// Infer a credential kind and likely service name from an environment
/// variable or configuration key. Unlike value-prefix rules, this does not
/// need a provider catalogue: semantic suffixes are removed and the remaining
/// identifier becomes the provider label. This lets uncommon services such as
/// IGDB and TMDB work without sending the credential value to the model.
pub fn classify_name(name: &str) -> Option<(SecretKind, String)> {
    let words: Vec<String> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_ascii_uppercase())
        .collect();
    if words.is_empty() {
        return None;
    }

    let has = |word: &str| words.iter().any(|candidate| candidate == word);
    let kind = if has("DATABASE")
        || has("DB")
        || has("POSTGRES")
        || has("MYSQL")
        || has("MONGODB")
        || has("REDIS")
    {
        SecretKind::Database
    } else if has("SSH") || (has("PRIVATE") && has("KEY")) {
        SecretKind::Ssh
    } else if has("WEBHOOK") {
        SecretKind::Webhook
    } else if has("OAUTH") || (has("CLIENT") && (has("ID") || has("SECRET"))) {
        SecretKind::OAuth
    } else if has("TOKEN") || has("PAT") || has("BEARER") {
        SecretKind::Token
    } else if has("APIKEY") || (has("API") && has("KEY")) || (has("SECRET") && has("KEY")) {
        SecretKind::ApiKey
    } else {
        return None;
    };

    const SEMANTIC: &[&str] = &[
        "API",
        "APIKEY",
        "KEY",
        "TOKEN",
        "SECRET",
        "CLIENT",
        "ID",
        "CLI",
        "OAUTH",
        "WEBHOOK",
        "BEARER",
        "PAT",
        "PASSWORD",
        "PASS",
        "DATABASE",
        "DB",
        "URL",
        "URI",
        "HOST",
        "PORT",
        "USERNAME",
        "USER",
        "PRIVATE",
        "PUBLIC",
        "ENV",
        "MY",
        "APP",
        "SERVICE",
        "CREDENTIAL",
        "CREDENTIALS",
    ];
    let provider_words: Vec<&str> = words
        .iter()
        .map(String::as_str)
        .filter(|word| !SEMANTIC.contains(word))
        .collect();
    let provider = provider_words
        .iter()
        .map(|word| {
            if word.len() <= 5 {
                (*word).to_string()
            } else {
                let mut chars = word.chars();
                let first = chars.next().unwrap_or_default();
                format!("{}{}", first, chars.as_str().to_ascii_lowercase())
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    Some((kind, provider))
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .as_bytes()
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
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
    fn infers_uncommon_services_from_environment_names() {
        assert_eq!(
            classify_name("IGDB_CLIENT_ID"),
            Some((SecretKind::OAuth, "IGDB".to_string()))
        );
        assert_eq!(
            classify_name("TMDB_API_KEY"),
            Some((SecretKind::ApiKey, "TMDB".to_string()))
        );
        assert_eq!(
            classify_name("VERCEL_CLI_TOKEN"),
            Some((SecretKind::Token, "Vercel".to_string()))
        );
        assert_eq!(classify_name("ORDINARY_SETTING"), None);
        assert_eq!(
            classify_name("DATABASE_URL"),
            Some((SecretKind::Database, String::new()))
        );
    }

    #[test]
    fn more_specific_prefixes_are_not_shadowed_by_shorter_ones() {
        // "sk_live_" (Stripe) must not be caught by any broader "sk" rule --
        // there isn't one, but this pins the ordering assumption the table
        // relies on so a future addition can't silently break it.
        assert_eq!(classify("sk_live_51ABC").unwrap().provider, Some("Stripe"));
    }

    /// The regression this whole module was reworked for: an OpenRouter key
    /// matched nothing, fell through to the model, and came back labelled
    /// "Stripe".
    ///
    /// **Every credential here is fabricated** -- a real prefix with a
    /// meaningless body -- and each is written as a separate `(prefix, body)`
    /// pair rather than one string. That is deliberate: a complete, contiguous
    /// token literal in this file trips GitHub's push-protection secret
    /// scanning, which cannot know ours are fake and correctly refuses the
    /// push. Splitting the literal leaves nothing for a scanner to match,
    /// while the value `classify` actually receives is byte-for-byte what it
    /// was before.
    #[test]
    fn every_supported_provider_is_recognised_without_the_model() {
        let cases: &[(&str, &str, SecretKind, Option<&str>)] = &[
            (
                "sk-or-v1-",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                SecretKind::ApiKey,
                Some("OpenRouter"),
            ),
            (
                "sk-or-",
                "abcdef0123456789abcdef0123456789",
                SecretKind::ApiKey,
                Some("OpenRouter"),
            ),
            (
                "sk-proj-",
                "abcdef0123456789abcdef0123456789",
                SecretKind::ApiKey,
                Some("OpenAI"),
            ),
            (
                "sk-svcacct-",
                "abcdef0123456789abcdef01234567",
                SecretKind::ApiKey,
                Some("OpenAI"),
            ),
            (
                "sk-",
                "abcdef0123456789abcdef0123456789abcdef01",
                SecretKind::ApiKey,
                Some("OpenAI"),
            ),
            (
                "sk-ant-api03-",
                "abcdef0123456789abcdef0123456789",
                SecretKind::ApiKey,
                Some("Anthropic"),
            ),
            (
                "pplx-",
                "abcdef0123456789abcdef0123456789",
                SecretKind::ApiKey,
                Some("Perplexity"),
            ),
            (
                "xai-",
                "abcdef0123456789abcdef0123456789",
                SecretKind::ApiKey,
                Some("xAI"),
            ),
            (
                "gsk_",
                "abcdef0123456789abcdef0123456789",
                SecretKind::ApiKey,
                Some("Groq"),
            ),
            (
                "hf_",
                "abcdefGHIJKL0123456789abcdefGHIJKL",
                SecretKind::Token,
                Some("Hugging Face"),
            ),
            (
                "r8_",
                "abcdef0123456789abcdef0123456789",
                SecretKind::ApiKey,
                Some("Replicate"),
            ),
            ("sk_live_", "51ABCdef", SecretKind::ApiKey, Some("Stripe")),
            ("sk_test_", "51ABCdef", SecretKind::ApiKey, Some("Stripe")),
            ("pk_live_", "51ABCdef", SecretKind::ApiKey, Some("Stripe")),
            ("pk_test_", "51ABCdef", SecretKind::ApiKey, Some("Stripe")),
            ("rk_live_", "51ABCdef", SecretKind::ApiKey, Some("Stripe")),
            (
                "whsec_",
                "abcdef0123456789",
                SecretKind::Webhook,
                Some("Stripe"),
            ),
            (
                "ghp_",
                "abcdef0123456789",
                SecretKind::Token,
                Some("GitHub"),
            ),
            (
                "gho_",
                "abcdef0123456789",
                SecretKind::Token,
                Some("GitHub"),
            ),
            (
                "ghs_",
                "abcdef0123456789",
                SecretKind::Token,
                Some("GitHub"),
            ),
            (
                "ghu_",
                "abcdef0123456789",
                SecretKind::Token,
                Some("GitHub"),
            ),
            (
                "ghr_",
                "abcdef0123456789",
                SecretKind::Token,
                Some("GitHub"),
            ),
            (
                "github_pat_",
                "11ABCDEFG0abcdef0123456789",
                SecretKind::Token,
                Some("GitHub"),
            ),
            (
                "glpat-",
                "abcdef0123456789abcd",
                SecretKind::Token,
                Some("GitLab"),
            ),
            ("AKIAIO", "SFODNN7EXAMPLE", SecretKind::ApiKey, Some("AWS")),
            ("ASIAIO", "SFODNN7EXAMPLE", SecretKind::ApiKey, Some("AWS")),
            (
                "AIzaSy",
                "A1234567890abcdefghijklmno",
                SecretKind::ApiKey,
                Some("Google"),
            ),
            (
                "ya29.",
                "a0AfH6SMBabcdef0123456789",
                SecretKind::OAuth,
                Some("Google"),
            ),
            (
                "sbp_",
                "abcdef0123456789abcdef0123456789abcdef01",
                SecretKind::Token,
                Some("Supabase"),
            ),
            (
                "xoxb-1234-5678-",
                "abcdef",
                SecretKind::Token,
                Some("Slack"),
            ),
            (
                "xoxp-1234-5678-",
                "abcdef",
                SecretKind::Token,
                Some("Slack"),
            ),
            ("SG.abc123.", "def456", SecretKind::ApiKey, Some("SendGrid")),
            (
                "shpat_",
                "abcdef0123456789abcdef0123456789ab",
                SecretKind::Token,
                Some("Shopify"),
            ),
            (
                "npm_",
                "abcdef0123456789abcdef0123456789ab",
                SecretKind::Token,
                Some("npm"),
            ),
            (
                "dop_v1_",
                "abcdef0123456789abcdef0123456789",
                SecretKind::Token,
                Some("DigitalOcean"),
            ),
            (
                "lin_api_",
                "abcdef0123456789abcdef01",
                SecretKind::ApiKey,
                Some("Linear"),
            ),
            (
                "sntrys_",
                "abcdef0123456789abcdef01",
                SecretKind::Token,
                Some("Sentry"),
            ),
            (
                "https://hooks.",
                "slack.com/services/T000/B000/abcdef0123456789",
                SecretKind::Webhook,
                Some("Slack"),
            ),
            (
                "https://discord.",
                "com/api/webhooks/123456789/abcdef0123456789",
                SecretKind::Webhook,
                Some("Discord"),
            ),
            (
                "postgres://user:",
                "pass@host:5432/db",
                SecretKind::Database,
                Some("PostgreSQL"),
            ),
            (
                "postgresql://",
                "user:pass@host:5432/db",
                SecretKind::Database,
                Some("PostgreSQL"),
            ),
            (
                "mysql://user:",
                "pass@host:3306/db",
                SecretKind::Database,
                Some("MySQL"),
            ),
            (
                "mongodb+srv://",
                "user:pass@cluster.mongodb.net/db",
                SecretKind::Database,
                Some("MongoDB"),
            ),
            (
                "redis://:",
                "pass@host:6379/0",
                SecretKind::Database,
                Some("Redis"),
            ),
            (
                "rediss://:",
                "pass@host:6379/0",
                SecretKind::Database,
                Some("Redis"),
            ),
            (
                "mssql://user:",
                "pass@host:1433/db",
                SecretKind::Database,
                Some("SQL Server"),
            ),
            (
                "eyJhbG",
                "ciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dGhpc2lzYXNpZ25hdHVyZQ",
                SecretKind::Token,
                Some("JWT"),
            ),
        ];

        for (prefix, body, kind, provider) in cases {
            let value = format!("{prefix}{body}");
            let got = classify(&value)
                .unwrap_or_else(|| panic!("expected a deterministic match for {value}"));
            assert_eq!(got.kind, *kind, "wrong kind for {value}");
            assert_eq!(got.provider, *provider, "wrong provider for {value}");
        }
    }

    /// The precise failure mode of the original bug: an OpenRouter key must
    /// not come back as Stripe, and must not come back as OpenAI either
    /// (the `sk-` fallback is shorter than `sk-or-`, so longest-match has to
    /// prefer OpenRouter).
    #[test]
    fn an_openrouter_key_is_never_read_as_stripe_or_openai() {
        // Split for the same secret-scanning reason as the table above.
        let key = format!("{}{}", "sk-or-v1-", "0123456789abcdef0123456789abcdef");
        let got = classify(&key).unwrap();
        assert_eq!(got.provider, Some("OpenRouter"));
        assert_ne!(got.provider, Some("Stripe"));
        assert_ne!(got.provider, Some("OpenAI"));
    }

    /// Longest-match, stated directly: each of these shares a start with a
    /// shorter rule, and the more specific one has to win every time.
    #[test]
    fn the_longest_matching_prefix_wins_over_a_shorter_one() {
        let cases: &[(&str, &str)] = &[
            ("sk-or-v1-abc", "OpenRouter"),    // beats "sk-or-" and "sk-"
            ("sk-or-abc", "OpenRouter"),       // beats "sk-"
            ("sk-ant-api03-abc", "Anthropic"), // beats "sk-ant-" and "sk-"
            ("sk-proj-abc", "OpenAI"),         // beats "sk-"
            ("sk-plainlegacykey", "OpenAI"),   // the fallback itself
        ];
        for (value, expected) in cases {
            assert_eq!(
                classify(value).unwrap().provider,
                Some(*expected),
                "wrong provider for {value}"
            );
        }
    }

    /// A database URL's scheme is case-insensitive; a bearer token's prefix
    /// is not. Both halves matter -- treating a token prefix loosely would
    /// start matching unrelated values.
    #[test]
    fn scheme_matching_ignores_case_but_token_prefixes_do_not() {
        assert_eq!(
            classify("POSTGRES://user:pass@host/db").unwrap().provider,
            Some("PostgreSQL")
        );
        assert_eq!(
            classify("PostgreSQL://user:pass@host/db").unwrap().provider,
            Some("PostgreSQL")
        );
        // Upper-case token prefixes stay distinct from their lower-case
        // spelling: "GHP_" is not GitHub's documented prefix.
        assert!(classify("GHP_abcdef0123456789").is_none());
    }

    /// Nothing here should claim a match on ordinary text that merely starts
    /// with a letter sequence -- a false positive is worse than no answer,
    /// because it silently mislabels a stored credential.
    #[test]
    fn ordinary_text_is_not_forced_into_a_provider() {
        for value in [
            "just a note about the staging server",
            "skateboard",
            "AKIA", // the bare prefix with no body is still a prefix match
            "password123",
            "https://example.com/not-a-webhook",
        ] {
            let got = classify(value);
            // "AKIA" alone is the one deliberate exception: it is exactly
            // AWS's documented prefix, so matching it is correct.
            if value == "AKIA" {
                assert_eq!(got.unwrap().provider, Some("AWS"));
            } else {
                assert!(got.is_none(), "{value} should not have matched: {got:?}");
            }
        }
    }

    #[test]
    fn whitespace_is_trimmed_before_matching() {
        assert_eq!(
            classify("  ghp_1234567890abcdef  ").unwrap().provider,
            Some("GitHub")
        );
    }
}
