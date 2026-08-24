// Tests report failure by panicking, so the core crate's no-panic lints are
// relaxed here. Integration tests are their own crate, so this cannot be
// inherited from lib.rs.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! End-to-end vault behaviour against a real file on disk.
//!
//! These are the Phase 0 exit criteria: create, add, close, reopen, unlock,
//! reveal -- and confirm that nothing readable is left in the file.

use envryn_core::crypto::kdf::KdfParams;
use envryn_core::model::{Environment, NewSecret, SecretPayload, SecretUpdate};
use envryn_core::vault::{api_key, Vault};
use envryn_core::Error;
use zeroize::Zeroizing;

/// Deliberately weak, for test speed only. Production uses 64 MiB.
const FAST: KdfParams = KdfParams {
    memory_kib: 19 * 1024,
    iterations: 2,
    parallelism: 1,
};

fn pw(s: &str) -> Zeroizing<String> {
    Zeroizing::new(s.to_string())
}

struct TempVault {
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

fn temp() -> TempVault {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("envryn.db");
    TempVault { _dir: dir, path }
}

#[test]
fn create_persist_reopen_and_reveal() {
    let t = temp();

    let id = {
        let mut vault = Vault::create(&t.path, &pw("correct horse battery staple"), FAST).unwrap();
        let summary = vault
            .create_secret(api_key(
                "GROQ_API_KEY",
                "Rescripto",
                Environment::Development,
                "gsk_9dK2mQ4vTz81LpXw0aBn7Rc5",
            ))
            .unwrap();
        vault.lock();
        summary.id
    };

    // Reopen from disk as a fresh process would.
    let mut vault = Vault::open(&t.path).unwrap();
    assert!(!vault.is_unlocked());
    assert!(matches!(vault.list(), Err(Error::Locked)));

    vault.unlock(&pw("correct horse battery staple")).unwrap();

    let listed = vault.list().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "GROQ_API_KEY");

    let revealed = vault.reveal(id).unwrap();
    match revealed.payload {
        SecretPayload::ApiKey { value } => assert_eq!(value, "gsk_9dK2mQ4vTz81LpXw0aBn7Rc5"),
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn wrong_password_fails_and_leaves_the_vault_locked() {
    let t = temp();
    {
        let mut v = Vault::create(&t.path, &pw("right-password"), FAST).unwrap();
        v.create_secret(api_key("K", "P", Environment::Production, "v"))
            .unwrap();
        v.lock();
    }

    let mut vault = Vault::open(&t.path).unwrap();
    assert!(matches!(
        vault.unlock(&pw("wrong-password")),
        Err(Error::AuthenticationFailed)
    ));
    assert!(
        !vault.is_unlocked(),
        "a failed unlock must not open the vault"
    );
    assert!(matches!(vault.list(), Err(Error::Locked)));

    // The correct password still works afterwards.
    vault.unlock(&pw("right-password")).unwrap();
    assert_eq!(vault.count().unwrap(), 1);
}

/// The headline claim: a stolen vault file reveals nothing. Scans the database
/// and every sidecar (WAL, SHM) for the secret value *and* its metadata.
#[test]
fn nothing_readable_is_written_to_disk() {
    let t = temp();
    let secret = "sk-proj-AVERYDISTINCTIVESECRETVALUE";
    let name = "DISTINCTIVE_NAME_TOKEN";
    let project = "DistinctiveProjectName";

    {
        let mut vault = Vault::create(&t.path, &pw("master-password"), FAST).unwrap();
        vault
            .create_secret(NewSecret {
                name: name.into(),
                project: project.into(),
                environment: Environment::Production,
                payload: SecretPayload::ApiKey {
                    value: secret.into(),
                },
                notes: Some("DISTINCTIVE_NOTE_BODY".into()),
                tags: vec!["DISTINCTIVE_TAG".into()],
                provider: Some("OpenAI".into()),
            })
            .unwrap();
        vault.lock();
    }

    let mut haystack = Vec::new();
    for suffix in ["", "-wal", "-shm"] {
        let p = t.path.with_file_name(format!(
            "{}{suffix}",
            t.path.file_name().and_then(|n| n.to_str()).unwrap()
        ));
        if let Ok(bytes) = std::fs::read(&p) {
            haystack.extend_from_slice(&bytes);
        }
    }
    assert!(!haystack.is_empty(), "expected to read the vault file");

    for needle in [
        secret,
        name,
        project,
        "DISTINCTIVE_NOTE_BODY",
        "DISTINCTIVE_TAG",
    ] {
        assert!(
            !haystack
                .windows(needle.len())
                .any(|w| w == needle.as_bytes()),
            "`{needle}` was found in plaintext on disk"
        );
    }
}

#[test]
fn locking_denies_every_read() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();
    let id = vault
        .create_secret(api_key("K", "P", Environment::Staging, "v"))
        .unwrap()
        .id;

    vault.lock();

    assert!(matches!(vault.list(), Err(Error::Locked)));
    assert!(matches!(vault.reveal(id), Err(Error::Locked)));
    assert!(matches!(vault.search("K"), Err(Error::Locked)));
    assert!(matches!(vault.count(), Err(Error::Locked)));
    assert!(matches!(vault.duplicates_of(id), Err(Error::Locked)));
}

#[test]
fn creating_over_an_existing_vault_is_refused() {
    let t = temp();
    let _v = Vault::create(&t.path, &pw("p"), FAST).unwrap();
    assert!(matches!(
        Vault::create(&t.path, &pw("other"), FAST),
        Err(Error::VaultExists)
    ));
}

#[test]
fn opening_a_nonexistent_vault_errors() {
    let t = temp();
    assert!(matches!(Vault::open(&t.path), Err(Error::VaultNotFound)));
}

#[test]
fn update_and_delete_persist() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();

    let id = vault
        .create_secret(api_key("OLD", "P", Environment::Development, "v1"))
        .unwrap()
        .id;

    vault
        .update_secret(
            id,
            SecretUpdate {
                name: Some("NEW".into()),
                payload: Some(SecretPayload::ApiKey { value: "v2".into() }),
                mark_rotated: true,
                ..Default::default()
            },
        )
        .unwrap();

    vault.lock();
    vault.unlock(&pw("p")).unwrap();

    let revealed = vault.reveal(id).unwrap();
    assert_eq!(revealed.name, "NEW");
    assert!(revealed.rotated_ms.is_some());
    match revealed.payload {
        SecretPayload::ApiKey { value } => assert_eq!(value, "v2"),
        other => panic!("unexpected payload: {other:?}"),
    }

    vault.delete_secret(id).unwrap();
    vault.lock();
    vault.unlock(&pw("p")).unwrap();
    assert_eq!(vault.count().unwrap(), 0);
    assert!(matches!(vault.reveal(id), Err(Error::NotFound)));
}

/// Changing the password rewraps the VMK, so records stay readable and the old
/// password stops working -- both halves matter.
#[test]
fn password_change_preserves_records() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("old-password"), FAST).unwrap();
    let id = vault
        .create_secret(api_key("K", "P", Environment::Production, "the-value"))
        .unwrap()
        .id;

    vault
        .change_password(&pw("old-password"), &pw("new-password"), FAST)
        .unwrap();
    vault.lock();

    let mut vault = Vault::open(&t.path).unwrap();
    assert!(matches!(
        vault.unlock(&pw("old-password")),
        Err(Error::AuthenticationFailed)
    ));

    vault.unlock(&pw("new-password")).unwrap();
    match vault.reveal(id).unwrap().payload {
        SecretPayload::ApiKey { value } => assert_eq!(value, "the-value"),
        other => panic!("unexpected payload: {other:?}"),
    }
}

#[test]
fn password_change_requires_the_current_password() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("real"), FAST).unwrap();
    assert!(matches!(
        vault.change_password(&pw("guess"), &pw("new"), FAST),
        Err(Error::AuthenticationFailed)
    ));
    // The original password must still work after a failed attempt.
    vault.lock();
    vault.unlock(&pw("real")).unwrap();
}

#[test]
fn duplicates_are_detected_by_value_not_by_name() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();

    let a = vault
        .create_secret(api_key(
            "NAME_A",
            "ProjA",
            Environment::Development,
            "shared",
        ))
        .unwrap()
        .id;
    let b = vault
        .create_secret(api_key(
            "NAME_B",
            "ProjB",
            Environment::Production,
            "shared",
        ))
        .unwrap()
        .id;
    let c = vault
        .create_secret(api_key(
            "NAME_C",
            "ProjC",
            Environment::Production,
            "unique",
        ))
        .unwrap()
        .id;

    assert_eq!(vault.duplicates_of(a).unwrap(), vec![b]);
    assert_eq!(vault.duplicates_of(b).unwrap(), vec![a]);
    assert!(vault.duplicates_of(c).unwrap().is_empty());
}

/// Search must never match on the secret value, or the search box becomes an
/// oracle for confirming a guessed credential.
#[test]
fn search_matches_metadata_but_never_values() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();
    vault
        .create_secret(NewSecret {
            name: "GITHUB_TOKEN".into(),
            project: "Rescripto".into(),
            environment: Environment::Production,
            payload: SecretPayload::ApiKey {
                value: "ghp_SEARCHABLE_MARKER".into(),
            },
            notes: None,
            tags: vec!["deployment".into()],
            provider: Some("GitHub".into()),
        })
        .unwrap();

    assert_eq!(vault.search("github").unwrap().len(), 1, "name match");
    assert_eq!(vault.search("rescripto").unwrap().len(), 1, "project match");
    assert_eq!(vault.search("deployment").unwrap().len(), 1, "tag match");

    assert!(
        vault.search("ghp_SEARCHABLE_MARKER").unwrap().is_empty(),
        "search matched a secret value -- this is a credential-confirmation oracle"
    );
}

#[test]
fn multi_field_payloads_round_trip() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();

    let id = vault
        .create_secret(NewSecret {
            name: "Primary Postgres".into(),
            project: "NameVetta".into(),
            environment: Environment::Production,
            payload: SecretPayload::Database {
                host: "db.namevetta.io".into(),
                port: 5432,
                database: "main".into(),
                username: "app".into(),
                password: "9fTz-secret".into(),
            },
            notes: None,
            tags: vec![],
            provider: None,
        })
        .unwrap()
        .id;

    vault.lock();
    vault.unlock(&pw("p")).unwrap();

    match vault.reveal(id).unwrap().payload {
        SecretPayload::Database {
            host,
            port,
            database,
            username,
            password,
        } => {
            assert_eq!(host, "db.namevetta.io");
            assert_eq!(port, 5432);
            assert_eq!(database, "main");
            assert_eq!(username, "app");
            assert_eq!(password, "9fTz-secret");
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}

/// A vault holding the same credential as another must produce a different
/// fingerprint, because fingerprints are keyed under that vault's own VMK.
#[test]
fn fingerprints_do_not_correlate_across_vaults() {
    let a = temp();
    let b = temp();

    let read_fingerprint = |path: &std::path::Path| -> Vec<u8> {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.query_row("SELECT fingerprint FROM secrets LIMIT 1", [], |r| {
            r.get::<_, Vec<u8>>(0)
        })
        .unwrap()
    };

    for t in [&a, &b] {
        let mut v = Vault::create(&t.path, &pw("p"), FAST).unwrap();
        v.create_secret(api_key("K", "P", Environment::Production, "changeme"))
            .unwrap();
        v.lock();
    }

    assert_ne!(
        read_fingerprint(&a.path),
        read_fingerprint(&b.path),
        "identical credentials produced identical fingerprints across vaults"
    );
}
