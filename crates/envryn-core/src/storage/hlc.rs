//! Hybrid logical clock.
//!
//! Wall-clock timestamps alone are unusable for ordering sync writes: a
//! phone and a desktop disagree by seconds even when both are "correct," and
//! a clock that jumps backwards (NTP correction, a user changing the time)
//! would silently lose an edit that should have won. An HLC combines a wall
//! clock with a logical counter so that causally-related events always order
//! correctly, and unrelated events order consistently even under modest
//! clock skew. See docs/CRYPTOGRAPHY.md's sync design and INV-109/INV-110.

use serde::{Deserialize, Serialize};

/// `(wall_ms, counter, device_id)`. The device id is the final, deterministic
/// tiebreak: two devices can legitimately produce the same `(wall_ms,
/// counter)` pair, and ties must resolve the same way on every peer, or
/// different devices could disagree about which of two writes "won."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Hlc {
    pub wall_ms: i64,
    pub counter: u32,
    pub device_id: u64,
}

impl Hlc {
    /// A zero clock, older than any real tick. Used as the default for a
    /// record that has never synced.
    pub const ZERO: Hlc = Hlc {
        wall_ms: 0,
        counter: 0,
        device_id: 0,
    };

    /// A stable numeric id derived from a device's fingerprint, used only for
    /// HLC tie-breaking -- not a security property, just needs to be
    /// deterministic and (for practical purposes) distinct per device. Takes
    /// the raw fingerprint bytes rather than `sync::Fingerprint` so this
    /// module -- storage, one layer below sync -- has no dependency on it.
    pub fn device_id_from_fingerprint_bytes(fingerprint: &[u8]) -> u64 {
        let head: [u8; 8] = fingerprint
            .get(..8)
            .and_then(|b| b.try_into().ok())
            .unwrap_or([0; 8]);
        u64::from_be_bytes(head)
    }

    /// Advance the clock for a new local write.
    ///
    /// If real time has moved past the clock's last tick, jump to it and
    /// reset the counter. Otherwise (the common case for several writes in
    /// the same millisecond, or the wall clock having gone backwards) keep
    /// the same `wall_ms` and increment the counter -- this is what makes
    /// the clock monotonic even when `SystemTime` is not.
    pub fn tick(&self, device_id: u64, now_wall_ms: i64) -> Hlc {
        if now_wall_ms > self.wall_ms {
            Hlc {
                wall_ms: now_wall_ms,
                counter: 0,
                device_id,
            }
        } else {
            Hlc {
                wall_ms: self.wall_ms,
                counter: self.counter.saturating_add(1),
                device_id,
            }
        }
    }

    /// Merge with a clock received from a peer, producing a value that is
    /// provably newer than both -- the core HLC receive rule. Every
    /// subsequent local write should tick from this merged value, so the
    /// local clock never falls behind a peer's.
    pub fn merge(&self, remote: Hlc, device_id: u64, now_wall_ms: i64) -> Hlc {
        let max_wall = self.wall_ms.max(remote.wall_ms).max(now_wall_ms);
        let counter = if max_wall == self.wall_ms && max_wall == remote.wall_ms {
            self.counter.max(remote.counter).saturating_add(1)
        } else if max_wall == self.wall_ms {
            self.counter.saturating_add(1)
        } else if max_wall == remote.wall_ms {
            remote.counter.saturating_add(1)
        } else {
            0
        };
        Hlc {
            wall_ms: max_wall,
            counter,
            device_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_advances_with_real_time() {
        let a = Hlc::ZERO.tick(1, 100);
        assert_eq!(
            a,
            Hlc {
                wall_ms: 100,
                counter: 0,
                device_id: 1
            }
        );
        let b = a.tick(1, 200);
        assert_eq!(
            b,
            Hlc {
                wall_ms: 200,
                counter: 0,
                device_id: 1
            }
        );
    }

    #[test]
    fn tick_increments_counter_within_the_same_millisecond() {
        let a = Hlc::ZERO.tick(1, 100);
        let b = a.tick(1, 100);
        assert!(b > a);
        assert_eq!(b.counter, 1);
    }

    /// The whole point of an HLC: a wall clock going backwards must not
    /// produce a clock that orders before the previous tick.
    #[test]
    fn tick_is_monotonic_even_if_wall_clock_goes_backwards() {
        let a = Hlc::ZERO.tick(1, 1_000_000);
        let b = a.tick(1, 500_000); // wall clock jumped backwards
        assert!(b > a);
        assert_eq!(b.wall_ms, a.wall_ms);
        assert_eq!(b.counter, a.counter + 1);
    }

    #[test]
    fn merge_produces_a_clock_newer_than_both_inputs() {
        let local = Hlc::ZERO.tick(1, 100);
        let remote = Hlc {
            wall_ms: 150,
            counter: 3,
            device_id: 2,
        };
        let merged = local.merge(remote, 1, 90);
        assert!(merged > local);
        assert!(merged > remote);
    }

    #[test]
    fn ties_break_deterministically_on_device_id() {
        let a = Hlc {
            wall_ms: 100,
            counter: 5,
            device_id: 1,
        };
        let b = Hlc {
            wall_ms: 100,
            counter: 5,
            device_id: 2,
        };
        assert!(a < b);
        // The comparison must agree regardless of which side evaluates it.
        assert!(b > a);
    }

    #[test]
    fn ordering_is_wall_ms_then_counter_then_device() {
        let older = Hlc {
            wall_ms: 100,
            counter: 9,
            device_id: 9,
        };
        let newer_wall = Hlc {
            wall_ms: 101,
            counter: 0,
            device_id: 0,
        };
        assert!(older < newer_wall);

        let lower_counter = Hlc {
            wall_ms: 100,
            counter: 1,
            device_id: 9,
        };
        let higher_counter = Hlc {
            wall_ms: 100,
            counter: 2,
            device_id: 0,
        };
        assert!(lower_counter < higher_counter);
    }
}
