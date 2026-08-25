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
use crate::crypto::keys::{KeySlot, SymmetricKey, VaultKeys, VaultMasterKey, KEY_LEN};
use crate::error::{Error, Result};
use crate::model::{
    self, Environment, NewSecret, SecretId, SecretPayload, SecretRecord, SecretSummary,
    SecretUpdate, TrustedDevice,
};
use crate::platform;
use crate::storage::{meta_keys, Hlc, Store, StoredRecord, RECORD_VERSION};

pub const CRYPTO_VERSION: u32 = 1;

/// Keys and decrypted records held only while unlocked.
struct UnlockedState {
    keys: VaultKeys,
    /// The in-memory index. Search runs here rather than in SQL, which is why
    /// the database needs no plaintext columns at all
    /// (docs/CRYPTOGRAPHY.md section 4).
    index: Vec<SecretRecord>,
    /// Numeric id this device stamps its own writes with, for HLC
    /// tie-breaking. Zero until [`Vault::set_local_device_id`] is called
    /// (which the IPC layer does once, right after unlock, from the
    /// installation's `DeviceIdentity` fingerprint) -- a vault that has
    /// never been paired simply never needs to break a tie against a peer,
    /// so zero is a harmless default rather than a placeholder that must be
    /// fixed before anything works.
    local_device_id: u64,
    /// The most recent HLC this device has produced or observed, advanced by
    /// [`Vault::tick_hlc`]. Starts at `Hlc::ZERO` each session rather than
    /// being persisted and reloaded -- real wall-clock time dominates
    /// ordering in every practical case, and the only scenario that would
    /// need persisted counter state (two writes racing within the same
    /// millisecond, separated by an app restart) is not worth the added
    /// complexity to close.
    last_hlc: Hlc,
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

    /// Create a new vault, protected by `password`, with a freshly generated
    /// VMK.
    ///
    /// Refuses if one already exists: the alternative is overwriting somebody's
    /// keys, which destroys every record irrecoverably.
    pub fn create(path: &Path, password: &Zeroizing<String>, params: KdfParams) -> Result<Self> {
        Self::create_with_vmk(path, password, params, VaultMasterKey::generate()?)
    }

    /// Create a new vault seeded with an *existing* VMK rather than a fresh
    /// random one.
    ///
    /// This is what makes device pairing work: two devices that hold the
    /// same VMK derive identical record and fingerprint subkeys, so sealed
    /// rows sync as opaque bytes with no re-encryption at either end (see
    /// docs/CRYPTOGRAPHY.md section 2, "paired devices may each have a
    /// different master password"). The receiving device calls this with the
    /// VMK recovered from [`crate::sync::pairing::open_vmk`] and whatever
    /// local master password the user chooses for *this* device -- the two
    /// need not match.
    pub fn create_with_vmk(
        path: &Path,
        password: &Zeroizing<String>,
        params: KdfParams,
        vmk: VaultMasterKey,
    ) -> Result<Self> {
        let store = Store::open(path)?;
        if store.is_initialised()? {
            return Err(Error::VaultExists);
        }

        let salt = kdf::generate_salt()?;
        let kek = kdf::derive_kek(password, &salt, params)?;
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
                local_device_id: 0,
                last_hlc: Hlc::ZERO,
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
        self.state = Some(UnlockedState {
            keys,
            index,
            local_device_id: 0,
            last_hlc: Hlc::ZERO,
        });
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

    /// Set the numeric id this vault stamps its own writes with, for HLC
    /// tie-breaking against synced peers. Called once by the IPC layer right
    /// after unlock, derived from the installation's `DeviceIdentity`
    /// fingerprint (`storage::Hlc::device_id_from_fingerprint_bytes`) -- the
    /// vault itself has no dependency on device identity or the network
    /// (`sync` depends on `storage` and `vault`, never the reverse).
    pub fn set_local_device_id(&mut self, id: u64) -> Result<()> {
        self.state_mut()?.local_device_id = id;
        Ok(())
    }

    /// Advance and return this vault's HLC for a new local write.
    fn tick_hlc(&mut self) -> Result<Hlc> {
        let state = self.state_mut()?;
        let next = state.last_hlc.tick(state.local_device_id, now_ms());
        state.last_hlc = next;
        Ok(next)
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

        let hlc = self.tick_hlc()?;
        let stored = seal_record(&record, &self.state()?.keys, hlc)?;
        self.store.insert(&stored)?;

        let summary = record.summary();
        self.state_mut()?.index.insert(0, record);
        Ok(summary)
    }

    /// Insert a fully-formed record exactly as given -- id, timestamps, and
    /// all -- bypassing the "assign a new id, stamp now" path
    /// [`Vault::create_secret`] uses.
    ///
    /// The only caller is backup restore ([`crate::backup::restore`] feeding
    /// into this). Reconstructing prior state should reproduce it faithfully
    /// rather than treating every restored record as newly created at the
    /// moment of restore. It does, however, get a fresh local HLC tick:
    /// `SecretRecord` carries no HLC of its own (that lives only in
    /// `StoredRecord`, one layer down), so from the sync system's point of
    /// view a restored record is indistinguishable from a freshly written
    /// one -- consistent with backups being data-only (see `crate::backup`).
    pub fn import_record(&mut self, record: SecretRecord) -> Result<()> {
        if record.name.trim().is_empty() {
            return Err(Error::InvalidInput("a secret needs a name"));
        }
        model::validate_payload(&record.payload)?;

        let hlc = self.tick_hlc()?;
        let stored = seal_record(&record, &self.state()?.keys, hlc)?;
        self.store.insert(&stored)?;
        self.state_mut()?.index.insert(0, record);
        Ok(())
    }

    /// Every record, fully decrypted.
    ///
    /// This is one of exactly two places in Envryn that ever produce every
    /// secret value in the vault at once -- the other is
    /// [`crate::backup::create`], which is this method's only caller. It
    /// grants no capability an attacker with unlock access does not already
    /// have via repeated [`Vault::reveal`] calls; it exists so backup export
    /// can be one explicit, clearly-labelled action instead of many silent
    /// ones. See `src-tauri/src/ipc.rs` for why every other list-shaped IPC
    /// command deliberately returns summaries instead.
    pub fn export_all(&self) -> Result<Vec<SecretRecord>> {
        Ok(self.state()?.index.clone())
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

        let hlc = self.tick_hlc()?;
        let stored = seal_record(&record, &self.state()?.keys, hlc)?;
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
        let hlc = self.tick_hlc()?;
        self.store.soft_delete(id, hlc)?;
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

    /// Whether the platform slot is set up. Callable while locked -- this is
    /// metadata about the vault, not vault content, so the UI can decide
    /// whether to offer an "unlock with this Windows account" option before
    /// the user has typed anything.
    pub fn platform_protection_enabled(&self) -> Result<bool> {
        Ok(self.store.get_meta(meta_keys::PLATFORM_KEY_BLOB)?.is_some())
    }

    /// Enable the platform slot: unlock without the master password, using
    /// DPAPI to tie that ability to the current Windows user account.
    ///
    /// Requires the vault to already be unlocked *and* the current master
    /// password again. Re-confirming the password here mirrors
    /// [`Vault::change_password`]: enabling an alternate route into the vault
    /// is exactly the kind of action that deserves the same friction as
    /// changing the primary one, not less.
    ///
    /// DPAPI never sees the VMK. A fresh random key is generated, DPAPI
    /// protects *that*, and the VMK is wrapped under it through the same AEAD
    /// path every other slot uses (`crypto::keys`). If DPAPI's protection is
    /// ever weakened on some future Windows release, the VMK's own wrapping
    /// is still standing behind it.
    pub fn enable_platform_protection(
        &mut self,
        current_password: &Zeroizing<String>,
    ) -> Result<()> {
        self.state()?;

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

        let kek = kdf::derive_kek(current_password, &salt, params)?;
        let vmk = VaultMasterKey::unwrap_from(&kek, &wrapped, KeySlot::Password)?;

        let platform_kek = SymmetricKey::generate()?;
        let platform_blob = platform::dpapi_protect(platform_kek.as_slice())?;
        let wrapped_platform = vmk.wrap(&platform_kek, KeySlot::Platform)?;

        self.store
            .set_meta(meta_keys::PLATFORM_KEY_BLOB, &platform_blob)?;
        self.store
            .set_meta(&wrapped_key(KeySlot::Platform), wrapped_platform.as_bytes())?;
        Ok(())
    }

    /// Disable the platform slot. The password slot is untouched, so this can
    /// never be the operation that locks someone out (INV-007) -- there is
    /// nothing here that could fail in a way that also breaks the password.
    pub fn disable_platform_protection(&mut self) -> Result<()> {
        self.state()?;
        self.store.delete_meta(meta_keys::PLATFORM_KEY_BLOB)?;
        self.store.delete_meta(&wrapped_key(KeySlot::Platform))?;
        Ok(())
    }

    /// Recover the VMK to hand to a new peer during pairing.
    ///
    /// Requires the current master password again, exactly like
    /// [`Vault::enable_platform_protection`] does for the same reason:
    /// handing your VMK to another device is at least as sensitive as
    /// opening a new local unlock route, so it gets at least as much
    /// friction. The VMK is not otherwise held anywhere in `UnlockedState`
    /// for the session -- only derived subkeys are -- so this is also the
    /// only place a whole VMK exists in memory outside of unlock itself.
    ///
    /// The caller (`sync::pairing`) seals this under a key derived from the
    /// pairing session, and only after the human has confirmed the SAS
    /// matches -- this method itself has no opinion on that; it is purely
    /// "prove you know the password, then here is the key."
    pub fn export_vmk_for_pairing(
        &self,
        current_password: &Zeroizing<String>,
    ) -> Result<VaultMasterKey> {
        self.state()?;

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

        let kek = kdf::derive_kek(current_password, &salt, params)?;
        VaultMasterKey::unwrap_from(&kek, &wrapped, KeySlot::Password)
    }

    // --- trusted devices ------------------------------------------------------
    //
    // `fingerprint` here is always raw bytes, never `sync::Fingerprint` --
    // envryn-core's dependency graph runs storage -> vault -> sync, and a
    // typed dependency the other way would invert it. The Tauri shell and
    // `sync` itself convert to/from `sync::Fingerprint` at their own
    // boundary.

    /// Record a newly paired device. The caller has already completed
    /// pairing (SAS confirmed, VMK exchanged if this device was the
    /// receiver) -- this only records the relationship so future sync
    /// connections from `fingerprint` are accepted.
    pub fn add_trusted_device(
        &mut self,
        device_id: &str,
        fingerprint: &[u8],
        name: &str,
    ) -> Result<TrustedDevice> {
        let now = now_ms();
        let device = TrustedDevice {
            device_id: device_id.to_string(),
            fingerprint_hex: hex_encode(fingerprint),
            name: name.to_string(),
            paired_ms: now,
            last_sync_ms: None,
        };
        let sealed = seal_trusted_device(&device, &self.state()?.keys)?;
        self.store
            .insert_trusted_device(device_id, fingerprint, sealed.as_bytes(), now)?;
        Ok(device)
    }

    pub fn list_trusted_devices(&self) -> Result<Vec<TrustedDevice>> {
        let keys = &self.state()?.keys;
        let rows = self.store.list_trusted_devices()?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(open_trusted_device(&row.device_id, &row.sealed, keys)?);
        }
        Ok(out)
    }

    pub fn rename_trusted_device(&mut self, device_id: &str, name: &str) -> Result<TrustedDevice> {
        let mut device = self
            .list_trusted_devices()?
            .into_iter()
            .find(|d| d.device_id == device_id)
            .ok_or(Error::NotFound)?;
        device.name = name.to_string();
        let sealed = seal_trusted_device(&device, &self.state()?.keys)?;
        self.store
            .update_trusted_device_sealed(device_id, sealed.as_bytes())?;
        Ok(device)
    }

    /// Revoke a device. The very next sync attempt from its fingerprint fails
    /// at the TLS handshake itself once the caller rebuilds
    /// `sync::transport::TrustedFingerprints` from the (now smaller) result
    /// of `Vault::trusted_fingerprints` -- see INV-104.
    pub fn revoke_trusted_device(&mut self, device_id: &str) -> Result<()> {
        self.state()?;
        self.store.revoke_trusted_device(device_id)
    }

    /// Every trusted fingerprint, for building `sync::transport`'s live
    /// verifier set. Requires the vault to be unlocked, even though the
    /// fingerprints themselves are unencrypted columns -- sync is something
    /// the user starts from inside the unlocked app, not a background
    /// listener that runs against a locked vault.
    pub fn trusted_fingerprints(&self) -> Result<Vec<Vec<u8>>> {
        self.state()?;
        self.store.list_trusted_fingerprints()
    }

    /// Unlock using the platform slot instead of the master password.
    ///
    /// Fails closed exactly like [`Vault::unlock`]: a missing slot, a DPAPI
    /// blob that belongs to a different Windows user account, and a tampered
    /// wrapped VMK are all indistinguishable `AuthenticationFailed` results.
    pub fn unlock_with_platform(&mut self) -> Result<()> {
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

        let blob = self
            .store
            .get_meta(meta_keys::PLATFORM_KEY_BLOB)?
            .ok_or(Error::AuthenticationFailed)?;
        let recovered =
            platform::dpapi_unprotect(&blob).map_err(|_| Error::AuthenticationFailed)?;
        if recovered.len() != KEY_LEN {
            return Err(Error::AuthenticationFailed);
        }
        let mut kek_bytes = [0u8; KEY_LEN];
        kek_bytes.copy_from_slice(&recovered);
        let platform_kek = SymmetricKey::from_bytes(kek_bytes);

        let wrapped_bytes = self
            .store
            .get_meta(&wrapped_key(KeySlot::Platform))?
            .ok_or(Error::AuthenticationFailed)?;
        let wrapped = Sealed::from_bytes(wrapped_bytes).map_err(|_| Error::AuthenticationFailed)?;
        let vmk = VaultMasterKey::unwrap_from(&platform_kek, &wrapped, KeySlot::Platform)?;

        let keys = VaultKeys::derive_from(&vmk)?;
        let index = load_index(&self.store, &keys.record)?;
        self.state = Some(UnlockedState {
            keys,
            index,
            local_device_id: 0,
            last_hlc: Hlc::ZERO,
        });
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

fn seal_record(record: &SecretRecord, keys: &VaultKeys, hlc: Hlc) -> Result<StoredRecord> {
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
        hlc,
        deleted: false,
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

/// AAD binding a trusted-device blob to the device id it belongs to, for the
/// same reason `record_aad` binds a secret to its row: without it, a
/// database write could move one device's sealed name onto another device's
/// row undetected.
fn trusted_device_aad(device_id: &str) -> Vec<u8> {
    format!("envryn/v1/trusted-device/{device_id}").into_bytes()
}

fn seal_trusted_device(device: &TrustedDevice, keys: &VaultKeys) -> Result<Sealed> {
    let plaintext = Zeroizing::new(serde_json::to_vec(device)?);
    aead::seal(
        &keys.record,
        &plaintext,
        &trusted_device_aad(&device.device_id),
    )
}

fn open_trusted_device(
    device_id: &str,
    sealed_bytes: &[u8],
    keys: &VaultKeys,
) -> Result<TrustedDevice> {
    let sealed = Sealed::from_bytes(sealed_bytes.to_vec())?;
    let plaintext = aead::open(&keys.record, &sealed, &trusted_device_aad(device_id))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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
