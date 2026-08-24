//! The vault: a locked/unlocked state machine over the store.
//!
//! Every operation that touches secret material lives here, because this is
//! the only layer holding keys. [`crate::storage`] below it moves opaque bytes;
//! callers above it hold a `Vault` and can do nothing without unlocking it.
//!
//! Locking drops [`UnlockedState`], which zeroizes every derived key and the
//! in-memory index in one step. There is no path that clears three keys and
//! forgets the fourth.

use std::path::Path;

use zeroize::Zeroizing;

use crate::crypto::aead::{self, Sealed};
use crate::crypto::fingerprint;
use crate::crypto::kdf::{self, KdfParams, SALT_LEN};
use crate::crypto::keys::{KeySlot, SymmetricKey, VaultKeys, VaultMasterKey};
use crate::error::{Error, Result};
use crate::model::{
    self, Environment, NewSecret, SecretId, SecretPayload, SecretRecord, SecretSummary,
    SecretUpdate,
};
use crate::storage::{meta_keys, Store, StoredRecord, RECORD_VERSION};

pub const CRYPTO_VERSION: u32 = 1;

/// Keys and decrypted records held only while unlocked.
struct UnlockedState {
    keys: VaultKeys,
    /// The in-memory index. Search runs here rather than in SQL, which is why
    /// the database needs no plaintext columns at all
    /// (docs/CRYPTOGRAPHY.md section 4).
    index: Vec<SecretRecord>,
}

impl Drop for UnlockedState {
    fn drop(&mut self) {
        // VaultKeys zeroizes itself. The decrypted index must be cleared too:
        // it holds every secret value in the vault.
        self.index.clear();
    }
}

pub struct Vault {
    store: Store,
    state: Option<UnlockedState>,
}

impl Vault {
    /// Open an existing vault file without unlocking it.
    pub fn open(path: &Path) -> Result<Self> {
        let store = Store::open(path)?;
        if !store.is_initialised()? {
            return Err(Error::VaultNotFound);
        }
        Ok(Self { store, state: None })
    }

    /// Create a new vault, protected by `password`.
    ///
    /// Refuses if one already exists: the alternative is overwriting somebody's
    /// keys, which destroys every record irrecoverably.
    pub fn create(path: &Path, password: &Zeroizing<String>, params: KdfParams) -> Result<Self> {
        let store = Store::open(path)?;
        if store.is_initialised()? {
            return Err(Error::VaultExists);
        }

        let salt = kdf::generate_salt()?;
        let kek = kdf::derive_kek(password, &salt, params)?;

        let vmk = VaultMasterKey::generate()?;
        let wrapped = vmk.wrap(&kek, KeySlot::Password)?;

        store.set_meta(meta_keys::CRYPTO_VERSION, &CRYPTO_VERSION.to_le_bytes())?;
        store.set_meta(meta_keys::KDF_SALT, &salt)?;
        store.set_meta(meta_keys::KDF_PARAMS, &serde_json::to_vec(&params)?)?;
        store.set_meta(&wrapped_key(KeySlot::Password), wrapped.as_bytes())?;

        let keys = VaultKeys::derive_from(&vmk)?;
        Ok(Self {
            store,
            state: Some(UnlockedState {
                keys,
                index: Vec::new(),
            }),
        })
    }

    pub fn is_unlocked(&self) -> bool {
        self.state.is_some()
    }

    /// Unlock with the master password.
    ///
    /// Every failure -- absent slot, wrong password, tampered wrapper, corrupt
    /// parameters -- surfaces as `AuthenticationFailed`. Distinguishing them
    /// would tell an attacker holding the file which guess was closer
    /// (INV-006).
    pub fn unlock(&mut self, password: &Zeroizing<String>) -> Result<()> {
        let stored_version = self
            .store
            .get_meta(meta_keys::CRYPTO_VERSION)?
            .ok_or(Error::VaultNotFound)?;
        let version = u32::from_le_bytes(
            stored_version
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or(Error::AuthenticationFailed)?,
        );
        if version > CRYPTO_VERSION {
            return Err(Error::UnsupportedVersion {
                found: version,
                supported: CRYPTO_VERSION,
            });
        }

        let salt_bytes = self
            .store
            .get_meta(meta_keys::KDF_SALT)?
            .ok_or(Error::AuthenticationFailed)?;
        let salt: [u8; SALT_LEN] = salt_bytes
            .as_slice()
            .try_into()
            .map_err(|_| Error::AuthenticationFailed)?;

        let params: KdfParams = self
            .store
            .get_meta(meta_keys::KDF_PARAMS)?
            .ok_or(Error::AuthenticationFailed)
            .and_then(|b| serde_json::from_slice(&b).map_err(|_| Error::AuthenticationFailed))?;

        let wrapped_bytes = self
            .store
            .get_meta(&wrapped_key(KeySlot::Password))?
            .ok_or(Error::AuthenticationFailed)?;
        let wrapped = Sealed::from_bytes(wrapped_bytes).map_err(|_| Error::AuthenticationFailed)?;

        let kek = kdf::derive_kek(password, &salt, params)?;
        let vmk = VaultMasterKey::unwrap_from(&kek, &wrapped, KeySlot::Password)?;
        let keys = VaultKeys::derive_from(&vmk)?;

        let index = load_index(&self.store, &keys.record)?;
        self.state = Some(UnlockedState { keys, index });
        Ok(())
    }

    /// Lock the vault.
    ///
    /// Infallible by design. A lock path that can return an error is a lock
    /// path that can leave the vault open, so the WAL checkpoint is attempted
    /// and its result deliberately discarded -- failing to tidy the sidecar
    /// file is not a reason to keep keys in memory.
    pub fn lock(&mut self) {
        self.state = None;
        let _ = self.store.checkpoint();
    }

    fn state(&self) -> Result<&UnlockedState> {
        self.state.as_ref().ok_or(Error::Locked)
    }

    fn state_mut(&mut self) -> Result<&mut UnlockedState> {
        self.state.as_mut().ok_or(Error::Locked)
    }

    // --- reads --------------------------------------------------------------

    /// List records as summaries.
    ///
    /// Returns [`SecretSummary`], which has no field capable of holding secret
    /// material. Listing therefore cannot leak a value regardless of what the
    /// caller does with the result (specification section 24).
    pub fn list(&self) -> Result<Vec<SecretSummary>> {
        Ok(self.state()?.index.iter().map(|r| r.summary()).collect())
    }

    /// Reveal one record's secret material.
    ///
    /// The deliberate counterpart to `list`: obtaining a value is a distinct,
    /// named, single-record operation, so "reveal" is auditable at the call
    /// site and gateable in the UI.
    pub fn reveal(&self, id: SecretId) -> Result<SecretRecord> {
        self.state()?
            .index
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or(Error::NotFound)
    }

    pub fn count(&self) -> Result<usize> {
        Ok(self.state()?.index.len())
    }

    /// Search the in-memory index.
    ///
    /// Matches on name, project, provider and tags -- never on the secret
    /// value. Matching values would let someone confirm a guessed credential
    /// by typing it into the search box.
    pub fn search(&self, query: &str) -> Result<Vec<SecretSummary>> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return self.list();
        }
        Ok(self
            .state()?
            .index
            .iter()
            .filter(|r| {
                r.name.to_lowercase().contains(&needle)
                    || r.project.to_lowercase().contains(&needle)
                    || r.provider
                        .as_deref()
                        .is_some_and(|p| p.to_lowercase().contains(&needle))
                    || r.tags.iter().any(|t| t.to_lowercase().contains(&needle))
            })
            .map(|r| r.summary())
            .collect())
    }

    /// Live records sharing an exact value with `id`.
    ///
    /// Deterministic, via keyed fingerprints. The AI is not involved
    /// (docs/CRYPTOGRAPHY.md section 5).
    pub fn duplicates_of(&self, id: SecretId) -> Result<Vec<SecretId>> {
        let state = self.state()?;
        let record = state
            .index
            .iter()
            .find(|r| r.id == id)
            .ok_or(Error::NotFound)?;

        let Some(material) = record.payload.fingerprint_material() else {
            return Ok(Vec::new());
        };
        let fp = fingerprint::fingerprint(&state.keys.fingerprint, material)?;

        Ok(self
            .store
            .find_by_fingerprint(&fp)?
            .into_iter()
            .filter(|other| *other != id)
            .collect())
    }

    // --- writes -------------------------------------------------------------

    pub fn create_secret(&mut self, input: NewSecret) -> Result<SecretSummary> {
        model::validate_new(&input)?;

        let now = now_ms();
        let record = SecretRecord {
            id: SecretId::new(),
            name: input.name.trim().to_string(),
            project: input.project,
            environment: input.environment,
            payload: input.payload,
            notes: input.notes,
            tags: input.tags,
            provider: input.provider,
            created_ms: now,
            updated_ms: now,
            rotated_ms: None,
        };

        let state = self.state()?;
        let stored = seal_record(&record, &state.keys)?;
        self.store.insert(&stored)?;

        let summary = record.summary();
        self.state_mut()?.index.insert(0, record);
        Ok(summary)
    }

    pub fn update_secret(&mut self, id: SecretId, update: SecretUpdate) -> Result<SecretSummary> {
        let state = self.state()?;
        let position = state
            .index
            .iter()
            .position(|r| r.id == id)
            .ok_or(Error::NotFound)?;

        let mut record = state.index.get(position).ok_or(Error::NotFound)?.clone();

        if let Some(name) = update.name {
            record.name = name.trim().to_string();
        }
        if let Some(project) = update.project {
            record.project = project;
        }
        if let Some(environment) = update.environment {
            record.environment = environment;
        }
        if let Some(payload) = update.payload {
            model::validate_payload(&payload)?;
            record.payload = payload;
        }
        if let Some(notes) = update.notes {
            record.notes = notes;
        }
        if let Some(tags) = update.tags {
            record.tags = tags;
        }
        if let Some(provider) = update.provider {
            record.provider = provider;
        }

        let now = now_ms();
        record.updated_ms = now;
        if update.mark_rotated {
            record.rotated_ms = Some(now);
        }

        if record.name.is_empty() {
            return Err(Error::InvalidInput("a secret needs a name"));
        }

        let stored = seal_record(&record, &self.state()?.keys)?;
        self.store.update(&stored)?;

        let summary = record.summary();
        let state = self.state_mut()?;
        if let Some(slot) = state.index.get_mut(position) {
            *slot = record;
        }
        Ok(summary)
    }

    pub fn delete_secret(&mut self, id: SecretId) -> Result<()> {
        self.state()?;
        self.store.soft_delete(id, now_ms())?;
        self.state_mut()?.index.retain(|r| r.id != id);
        Ok(())
    }

    /// Change the master password.
    ///
    /// Rewraps the VMK and nothing else, so this is instant regardless of vault
    /// size and cannot half-complete, leaving some records readable and others
    /// not (docs/CRYPTOGRAPHY.md section 2).
    pub fn change_password(
        &mut self,
        current: &Zeroizing<String>,
        new: &Zeroizing<String>,
        params: KdfParams,
    ) -> Result<()> {
        let salt_bytes = self
            .store
            .get_meta(meta_keys::KDF_SALT)?
            .ok_or(Error::AuthenticationFailed)?;
        let salt: [u8; SALT_LEN] = salt_bytes
            .as_slice()
            .try_into()
            .map_err(|_| Error::AuthenticationFailed)?;

        let old_params: KdfParams = self
            .store
            .get_meta(meta_keys::KDF_PARAMS)?
            .ok_or(Error::AuthenticationFailed)
            .and_then(|b| serde_json::from_slice(&b).map_err(|_| Error::AuthenticationFailed))?;

        let wrapped_bytes = self
            .store
            .get_meta(&wrapped_key(KeySlot::Password))?
            .ok_or(Error::AuthenticationFailed)?;
        let wrapped = Sealed::from_bytes(wrapped_bytes).map_err(|_| Error::AuthenticationFailed)?;

        let old_kek = kdf::derive_kek(current, &salt, old_params)?;
        let vmk = VaultMasterKey::unwrap_from(&old_kek, &wrapped, KeySlot::Password)?;

        // A fresh salt, so the new password is not derived over the old one's
        // parameters and an attacker with both wrappers gains nothing.
        let new_salt = kdf::generate_salt()?;
        let new_kek = kdf::derive_kek(new, &new_salt, params)?;
        let rewrapped = vmk.wrap(&new_kek, KeySlot::Password)?;

        self.store.set_meta(meta_keys::KDF_SALT, &new_salt)?;
        self.store
            .set_meta(meta_keys::KDF_PARAMS, &serde_json::to_vec(&params)?)?;
        self.store
            .set_meta(&wrapped_key(KeySlot::Password), rewrapped.as_bytes())?;
        Ok(())
    }
}

fn wrapped_key(slot: KeySlot) -> String {
    format!("{}{}", meta_keys::WRAPPED_VMK, slot.as_str())
}

/// AAD binding a record's ciphertext to its identity and format version, so a
/// blob cannot be moved between rows or rolled back to an earlier format.
fn record_aad(id: SecretId, version: i64) -> Vec<u8> {
    format!("envryn/v1/record/{id}/{version}").into_bytes()
}

fn seal_record(record: &SecretRecord, keys: &VaultKeys) -> Result<StoredRecord> {
    let plaintext = Zeroizing::new(serde_json::to_vec(record)?);
    let sealed = aead::seal(
        &keys.record,
        &plaintext,
        &record_aad(record.id, RECORD_VERSION),
    )?;

    let fingerprint = match record.payload.fingerprint_material() {
        Some(material) => Some(fingerprint::fingerprint(&keys.fingerprint, material)?),
        None => None,
    };

    Ok(StoredRecord {
        id: record.id,
        record_version: RECORD_VERSION,
        sealed: sealed.into_bytes(),
        fingerprint,
        created_ms: record.created_ms,
        updated_ms: record.updated_ms,
    })
}

fn open_record(stored: &StoredRecord, record_key: &SymmetricKey) -> Result<SecretRecord> {
    if stored.record_version > RECORD_VERSION {
        return Err(Error::UnsupportedVersion {
            found: stored.record_version as u32,
            supported: RECORD_VERSION as u32,
        });
    }
    let sealed = Sealed::from_bytes(stored.sealed.clone())?;
    let plaintext = aead::open(
        record_key,
        &sealed,
        &record_aad(stored.id, stored.record_version),
    )?;
    Ok(serde_json::from_slice(&plaintext)?)
}

fn load_index(store: &Store, record_key: &SymmetricKey) -> Result<Vec<SecretRecord>> {
    let mut index = Vec::new();
    for stored in store.list()? {
        index.push(open_record(&stored, record_key)?);
    }
    Ok(index)
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Convenience for building an API-key record in tests and simple callers.
pub fn api_key(name: &str, project: &str, environment: Environment, value: &str) -> NewSecret {
    NewSecret {
        name: name.to_string(),
        project: project.to_string(),
        environment,
        payload: SecretPayload::ApiKey {
            value: value.to_string(),
        },
        notes: None,
        tags: Vec::new(),
        provider: None,
    }
}
