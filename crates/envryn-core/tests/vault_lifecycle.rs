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

/// Adversarial: a bit-flipped (corrupted or tampered) vault file must fail
/// cleanly -- an authentication/format error, never a panic and never a
/// partial or silently-wrong unlock. This is the direct test for
/// `docs/SECURITY_INVARIANTS.md` section 11's "no best-effort parse of an
/// unknown/damaged format" claim, exercised against a real SQLite file on
/// disk rather than asserted from the doc alone.
#[test]
fn a_corrupted_vault_file_fails_cleanly_instead_of_panicking_or_silently_succeeding() {
    let t = temp();
    {
        let mut v = Vault::create(&t.path, &pw("right-password"), FAST).unwrap();
        v.create_secret(api_key("K", "P", Environment::Production, "v"))
            .unwrap();
        v.lock();
    }

    // Flip bytes through the middle third of the real file on disk -- avoids
    // only touching the SQLite header (a narrower, less interesting check)
    // and instead corrupts actual page content, the way real disk damage or
    // a hostile edit would.
    let mut bytes = std::fs::read(&t.path).unwrap();
    let start = bytes.len() / 3;
    let end = (bytes.len() * 2) / 3;
    let mut i = start;
    while i < end {
        bytes[i] ^= 0xff;
        i += 7;
    }
    std::fs::write(&t.path, &bytes).unwrap();

    // `Vault::open` itself must not panic on a corrupted file...
    let opened = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Vault::open(&t.path)));
    let mut vault = match opened {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => return, // a clean refusal at open time is an equally acceptable outcome
        Err(_) => panic!("Vault::open panicked on a corrupted file -- must fail cleanly instead"),
    };

    // ...and if it does open (SQLite itself may tolerate page-level damage
    // outside the corrupted region), unlocking against the real, undamaged
    // password must not panic and must not silently return plaintext that
    // doesn't match what was written.
    let unlocked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vault.unlock(&pw("right-password"))
    }));
    match unlocked {
        Err(_) => panic!("Vault::unlock panicked on a corrupted file -- must fail cleanly instead"),
        Ok(Err(_)) => {} // clean authentication/format failure -- the expected, safe outcome
        Ok(Ok(())) => {
            // Unlock reported success despite corruption -- only acceptable if
            // AEAD authentication still genuinely caught the damage on read.
            let listed = vault.list();
            if let Ok(records) = listed {
                for r in &records {
                    assert!(
                        vault.reveal(r.id).is_err(),
                        "corrupted vault: unlock succeeded AND a corrupted record still revealed \
                         successfully -- AEAD authentication should have caught this"
                    );
                }
            }
        }
    }
}

/// Adversarial: a large, multi-script Unicode value must round-trip through
/// real AEAD encryption/decryption and real SQLite storage byte-for-byte --
/// no truncation, no lossy re-encoding, no panic on a value near (but under)
/// the enforced size cap.
#[test]
fn a_large_unicode_secret_value_round_trips_exactly() {
    let t = temp();
    // Mixes multi-byte scripts (Japanese, Cyrillic, emoji, a 4-byte
    // supplementary-plane symbol) with plain ASCII padding to stay a
    // realistic-but-large size (well under the 256 KiB cap) while still
    // exercising multi-byte UTF-8 boundaries throughout.
    let value = format!(
        "\u{1F510}日本語の秘密鍵Секретный ключ{}\u{1D306}END",
        "A".repeat(50_000)
    );

    let id = {
        let mut vault = Vault::create(&t.path, &pw("unicode-password"), FAST).unwrap();
        let summary = vault
            .create_secret(api_key(
                "Big Unicode Secret",
                "P",
                Environment::Development,
                &value,
            ))
            .unwrap();
        vault.lock();
        summary.id
    };

    let mut vault = Vault::open(&t.path).unwrap();
    vault.unlock(&pw("unicode-password")).unwrap();
    let revealed = vault.reveal(id).unwrap();
    match revealed.payload {
        SecretPayload::ApiKey { value: got } => {
            assert_eq!(got, value, "value did not round-trip exactly")
        }
        other => panic!("unexpected payload: {other:?}"),
    }
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

// --- Platform (DPAPI) key protection -----------------------------------

#[test]
fn platform_protection_unlocks_without_the_password() {
    let t = temp();
    let id;
    {
        let mut vault = Vault::create(&t.path, &pw("master-password"), FAST).unwrap();
        id = vault
            .create_secret(api_key("K", "P", Environment::Production, "value"))
            .unwrap()
            .id;
        assert!(!vault.platform_protection_enabled().unwrap());

        vault
            .enable_platform_protection(&pw("master-password"))
            .unwrap();
        assert!(vault.platform_protection_enabled().unwrap());
        vault.lock();
    }

    let mut vault = Vault::open(&t.path).unwrap();
    assert!(vault.platform_protection_enabled().unwrap());
    vault.unlock_with_platform().unwrap();

    match vault.reveal(id).unwrap().payload {
        SecretPayload::ApiKey { value } => assert_eq!(value, "value"),
        other => panic!("unexpected payload: {other:?}"),
    }
}

/// INV-007: the password slot must keep working after platform protection is
/// enabled -- an alternate unlock path must never subtract from the original.
#[test]
fn platform_protection_does_not_disturb_the_password_slot() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("master-password"), FAST).unwrap();
    vault
        .enable_platform_protection(&pw("master-password"))
        .unwrap();
    vault.lock();

    let mut vault = Vault::open(&t.path).unwrap();
    vault.unlock(&pw("master-password")).unwrap();
    assert!(vault.is_unlocked());
}

/// The reverse of INV-007: removing platform protection must never touch the
/// password slot, since disabling a convenience feature must not be able to
/// lock someone out.
#[test]
fn disabling_platform_protection_preserves_password_unlock() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("master-password"), FAST).unwrap();
    vault
        .enable_platform_protection(&pw("master-password"))
        .unwrap();
    vault.disable_platform_protection().unwrap();
    assert!(!vault.platform_protection_enabled().unwrap());
    vault.lock();

    let mut vault = Vault::open(&t.path).unwrap();
    vault.unlock(&pw("master-password")).unwrap();
    assert!(vault.is_unlocked());
    assert!(matches!(
        vault.unlock_with_platform(),
        Err(Error::AuthenticationFailed)
    ));
}

/// The Windows Hello gate cannot be turned on without a platform slot for it
/// to gate -- gating an unlock path that does not exist is a dangling
/// setting, not a meaningful one.
#[test]
fn enabling_the_hello_gate_requires_platform_protection_first() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("master-password"), FAST).unwrap();
    assert!(!vault.hello_gate_enabled().unwrap());
    assert!(matches!(
        vault.enable_hello_gate(),
        Err(Error::InvalidInput(_))
    ));
    assert!(!vault.hello_gate_enabled().unwrap());
}

/// Disabling the platform slot must clear a Hello gate along with it, not
/// leave a gate pointing at an unlock path that no longer exists. Sets the
/// flag directly via `Store` rather than `enable_hello_gate` -- this
/// environment has no enrolled Windows Hello credential for
/// `platform::hello_enroll` to succeed against, and this test is about the
/// bookkeeping in `disable_platform_protection`, not about exercising real
/// Windows Hello hardware (see `platform::hello`'s own `#[ignore]`d test for
/// that).
#[test]
fn disabling_platform_protection_also_clears_the_hello_gate_flag() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("master-password"), FAST).unwrap();
    vault
        .enable_platform_protection(&pw("master-password"))
        .unwrap();

    let store = envryn_core::storage::Store::open(&t.path).unwrap();
    store
        .set_meta(envryn_core::storage::meta_keys::HELLO_GATE_ENABLED, &[1u8])
        .unwrap();
    drop(store);
    assert!(vault.hello_gate_enabled().unwrap());

    vault.disable_platform_protection().unwrap();
    assert!(!vault.hello_gate_enabled().unwrap());
    assert!(!vault.platform_protection_enabled().unwrap());
}

#[test]
fn enabling_platform_protection_requires_the_correct_password() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("real-password"), FAST).unwrap();
    assert!(matches!(
        vault.enable_platform_protection(&pw("wrong-password")),
        Err(Error::AuthenticationFailed)
    ));
    assert!(!vault.platform_protection_enabled().unwrap());
}

#[test]
fn platform_unlock_fails_cleanly_when_never_enabled() {
    let t = temp();
    {
        let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();
        vault.lock();
    }
    let mut vault = Vault::open(&t.path).unwrap();
    assert!(matches!(
        vault.unlock_with_platform(),
        Err(Error::AuthenticationFailed)
    ));
}

// --- Backup / restore -----------------------------------------------------

#[test]
fn backup_and_restore_through_two_real_vaults() {
    use envryn_core::backup;

    let source = temp();
    let restored = temp();

    let mut vault = Vault::create(&source.path, &pw("master-password"), FAST).unwrap();
    vault
        .create_secret(api_key("K1", "P1", Environment::Production, "value-one"))
        .unwrap();
    vault
        .create_secret(api_key("K2", "P2", Environment::Development, "value-two"))
        .unwrap();

    let records = vault.export_all().unwrap();
    let file = backup::create(&records, &pw("backup-password")).unwrap();

    // Restoring is independent of the source vault's password.
    let recovered = backup::restore(&file, &pw("backup-password")).unwrap();
    assert_eq!(recovered.len(), 2);

    let mut new_vault = Vault::create(&restored.path, &pw("new-master-password"), FAST).unwrap();
    for record in recovered {
        new_vault.import_record(record).unwrap();
    }
    new_vault.lock();

    // The restored vault is unlocked by its own new password, not the
    // original vault's password or the backup password.
    let mut new_vault = Vault::open(&restored.path).unwrap();
    new_vault.unlock(&pw("new-master-password")).unwrap();
    assert_eq!(new_vault.count().unwrap(), 2);

    let names: Vec<String> = new_vault
        .list()
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert!(names.contains(&"K1".to_string()));
    assert!(names.contains(&"K2".to_string()));
}

#[test]
fn restore_fails_with_the_wrong_backup_password() {
    use envryn_core::backup;

    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();
    vault
        .create_secret(api_key("K", "P", Environment::Production, "v"))
        .unwrap();

    let file = backup::create(&vault.export_all().unwrap(), &pw("right-backup-password")).unwrap();
    assert!(matches!(
        backup::restore(&file, &pw("wrong-backup-password")),
        Err(Error::AuthenticationFailed)
    ));
}

// --- HLC stamping on real writes -------------------------------------------

#[test]
fn writes_are_stamped_with_the_local_device_id() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();
    vault.set_local_device_id(4242).unwrap();

    let id = vault
        .create_secret(api_key("K", "P", Environment::Production, "v"))
        .unwrap()
        .id;

    let conn = rusqlite::Connection::open(&t.path).unwrap();
    let hlc_device: String = conn
        .query_row(
            "SELECT hlc_device FROM secrets WHERE id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hlc_device, "4242");
}

/// Two writes to two different records must produce strictly increasing
/// HLCs -- this is what a peer relies on to know which of two writes is
/// newer during reconciliation.
#[test]
fn successive_writes_advance_the_clock() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();
    vault.set_local_device_id(1).unwrap();

    let id_a = vault
        .create_secret(api_key("A", "P", Environment::Production, "a"))
        .unwrap()
        .id;
    let id_b = vault
        .create_secret(api_key("B", "P", Environment::Production, "b"))
        .unwrap()
        .id;

    let conn = rusqlite::Connection::open(&t.path).unwrap();
    let read_hlc = |id: &str| -> (i64, i64) {
        conn.query_row(
            "SELECT updated_ms, hlc_counter FROM secrets WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    };
    let (wall_a, counter_a) = read_hlc(&id_a.to_string());
    let (wall_b, counter_b) = read_hlc(&id_b.to_string());
    assert!(
        (wall_b, counter_b) > (wall_a, counter_a),
        "second write's HLC must be strictly newer than the first"
    );
}

// --- Trusted devices --------------------------------------------------------

#[test]
fn trusted_devices_round_trip_and_revoke() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();

    let fp = [7u8; 32];
    vault
        .add_trusted_device("device-1", &fp, "Android Phone")
        .unwrap();

    let listed = vault.list_trusted_devices().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Android Phone");
    assert_eq!(listed[0].fingerprint_hex, "07".repeat(32));

    assert_eq!(vault.trusted_fingerprints().unwrap(), vec![fp.to_vec()]);

    vault
        .rename_trusted_device("device-1", "Work Phone")
        .unwrap();
    assert_eq!(vault.list_trusted_devices().unwrap()[0].name, "Work Phone");

    vault.revoke_trusted_device("device-1").unwrap();
    assert!(vault.list_trusted_devices().unwrap().is_empty());
    assert!(vault.trusted_fingerprints().unwrap().is_empty());
}

#[test]
fn trusted_devices_persist_across_lock() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();
    vault
        .add_trusted_device("device-1", &[1u8; 32], "Laptop")
        .unwrap();
    vault.lock();

    let mut vault = Vault::open(&t.path).unwrap();
    vault.unlock(&pw("p")).unwrap();
    assert_eq!(vault.list_trusted_devices().unwrap().len(), 1);
}

#[test]
fn trusted_device_names_are_not_stored_in_plaintext() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();
    vault
        .add_trusted_device("device-1", &[1u8; 32], "DISTINCTIVE_DEVICE_NAME")
        .unwrap();
    vault.lock();

    let bytes = std::fs::read(&t.path).unwrap();
    assert!(
        !bytes
            .windows(b"DISTINCTIVE_DEVICE_NAME".len())
            .any(|w| w == b"DISTINCTIVE_DEVICE_NAME"),
        "device name was found in plaintext on disk"
    );
}

#[test]
fn revoking_an_unknown_device_errors() {
    let t = temp();
    let mut vault = Vault::create(&t.path, &pw("p"), FAST).unwrap();
    assert!(matches!(
        vault.revoke_trusted_device("nonexistent"),
        Err(Error::NotFound)
    ));
}
