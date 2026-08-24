//! Password-based key derivation.
//!
//! Argon2id, with parameters stored alongside the vault so they can be raised
//! on a future release without invalidating existing vaults.
//!
//! See docs/CRYPTOGRAPHY.md section 2.

use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto::keys::{SymmetricKey, KEY_LEN};
use crate::error::{Error, Result};

pub const SALT_LEN: usize = 16;

/// Argon2id cost parameters.
///
/// Persisted with the vault. A vault created on a phone at 32 MiB must still
/// open on that phone after the desktop default is raised to 128 MiB, so the
/// parameters travel with the data rather than living in the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub memory_kib: u32,
    /// Time cost (passes).
    pub iterations: u32,
    /// Parallelism (lanes).
    pub parallelism: u32,
}

impl KdfParams {
    /// Desktop default: 64 MiB, 3 passes, 4 lanes.
    pub const DESKTOP: Self = Self {
        memory_kib: 64 * 1024,
        iterations: 3,
        parallelism: 4,
    };

    /// Mobile default: 32 MiB. Low-end Android devices will have the process
    /// killed by the OS rather than granted 64 MiB of scratch space, and a
    /// vault that cannot open is worse than one with a lower cost factor.
    pub const MOBILE: Self = Self {
        memory_kib: 32 * 1024,
        iterations: 3,
        parallelism: 4,
    };

    /// Floor below which Envryn refuses to derive.
    ///
    /// Guards against a tampered database claiming `memory_kib = 8`, which
    /// would make an offline attack on the wrapped VMK trivial. The parameters
    /// are stored unauthenticated by necessity -- they are needed *before* any
    /// key exists to authenticate them with -- so they are range-checked here.
    pub const MINIMUM: Self = Self {
        memory_kib: 19 * 1024,
        iterations: 2,
        parallelism: 1,
    };

    pub fn platform_default() -> Self {
        if cfg!(target_os = "android") {
            Self::MOBILE
        } else {
            Self::DESKTOP
        }
    }

    fn validate(&self) -> Result<()> {
        if self.memory_kib < Self::MINIMUM.memory_kib
            || self.iterations < Self::MINIMUM.iterations
            || self.parallelism < Self::MINIMUM.parallelism
        {
            return Err(Error::InvalidInput(
                "KDF parameters below the permitted floor",
            ));
        }
        // Upper bounds: a hostile file must not be able to make unlock
        // allocate 16 GiB and take the process down with it.
        if self.memory_kib > 1024 * 1024 || self.iterations > 32 || self.parallelism > 16 {
            return Err(Error::InvalidInput(
                "KDF parameters above the permitted ceiling",
            ));
        }
        Ok(())
    }

    fn to_argon2_params(self) -> Result<Params> {
        Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(KEY_LEN),
        )
        .map_err(|_| Error::InvalidInput("invalid Argon2 parameters"))
    }
}

impl Default for KdfParams {
    fn default() -> Self {
        Self::platform_default()
    }
}

/// A random per-vault salt.
pub fn generate_salt() -> Result<[u8; SALT_LEN]> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt).map_err(|_| Error::Internal("CSPRNG unavailable"))?;
    Ok(salt)
}

/// Derive the key-encryption key from a master password.
///
/// The password arrives as `Zeroizing` so the caller cannot hold a plain
/// `String` copy past this call without it being visible in the type.
pub fn derive_kek(
    password: &Zeroizing<String>,
    salt: &[u8; SALT_LEN],
    params: KdfParams,
) -> Result<SymmetricKey> {
    params.validate()?;

    let argon = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        params.to_argon2_params()?,
    );

    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|_| Error::Internal("Argon2id derivation failed"))?;

    Ok(SymmetricKey::from_bytes(out))
}

/// Pick parameters that take roughly `target_ms` on this machine.
///
/// Runs at vault creation only. Starts from the platform default and lowers
/// memory if the device is too slow -- never below `MINIMUM`, because a fast
/// unlock on a weak device is not worth a cheap offline attack.
pub fn calibrate(target_ms: u128) -> KdfParams {
    use std::time::Instant;

    let password = Zeroizing::new("calibration-probe".to_string());
    let salt = [0u8; SALT_LEN];
    let mut params = KdfParams::platform_default();

    for _ in 0..4 {
        let start = Instant::now();
        if derive_kek(&password, &salt, params).is_err() {
            return KdfParams::MINIMUM;
        }
        let elapsed = start.elapsed().as_millis();

        if elapsed <= target_ms {
            return params;
        }
        let halved = params.memory_kib / 2;
        if halved < KdfParams::MINIMUM.memory_kib {
            return KdfParams::MINIMUM;
        }
        params.memory_kib = halved;
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fast parameters, for tests only. Real derivation at 64 MiB would make
    /// the suite take minutes.
    const TEST: KdfParams = KdfParams {
        memory_kib: 19 * 1024,
        iterations: 2,
        parallelism: 1,
    };

    fn pw(s: &str) -> Zeroizing<String> {
        Zeroizing::new(s.to_string())
    }

    #[test]
    fn derivation_is_deterministic() {
        let salt = [1u8; SALT_LEN];
        let a = derive_kek(&pw("correct horse battery staple"), &salt, TEST).unwrap();
        let b = derive_kek(&pw("correct horse battery staple"), &salt, TEST).unwrap();
        assert_eq!(a.as_slice(), b.as_slice());
    }

    #[test]
    fn different_passwords_differ() {
        let salt = [1u8; SALT_LEN];
        let a = derive_kek(&pw("password-one"), &salt, TEST).unwrap();
        let b = derive_kek(&pw("password-two"), &salt, TEST).unwrap();
        assert_ne!(a.as_slice(), b.as_slice());
    }

    /// The salt is what stops one rainbow table covering every Envryn vault.
    #[test]
    fn different_salts_differ() {
        let a = derive_kek(&pw("same"), &[1u8; SALT_LEN], TEST).unwrap();
        let b = derive_kek(&pw("same"), &[2u8; SALT_LEN], TEST).unwrap();
        assert_ne!(a.as_slice(), b.as_slice());
    }

    #[test]
    fn params_below_floor_are_rejected() {
        let weak = KdfParams {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        assert!(derive_kek(&pw("x"), &[0u8; SALT_LEN], weak).is_err());
    }

    /// A tampered vault file must not be able to exhaust memory at unlock.
    #[test]
    fn params_above_ceiling_are_rejected() {
        let absurd = KdfParams {
            memory_kib: 16 * 1024 * 1024,
            iterations: 3,
            parallelism: 4,
        };
        assert!(derive_kek(&pw("x"), &[0u8; SALT_LEN], absurd).is_err());
    }

    #[test]
    fn salts_are_random() {
        assert_ne!(generate_salt().unwrap(), generate_salt().unwrap());
    }

    #[test]
    fn defaults_satisfy_the_floor() {
        for p in [KdfParams::DESKTOP, KdfParams::MOBILE, KdfParams::MINIMUM] {
            assert!(p.validate().is_ok(), "{p:?} should be permitted");
        }
    }

    #[test]
    fn empty_password_still_derives() {
        // Length policy belongs at the UI/vault layer, not here. The KDF must
        // not silently succeed-with-a-constant on empty input.
        let a = derive_kek(&pw(""), &[0u8; SALT_LEN], TEST).unwrap();
        let b = derive_kek(&pw("x"), &[0u8; SALT_LEN], TEST).unwrap();
        assert_ne!(a.as_slice(), b.as_slice());
    }
}
