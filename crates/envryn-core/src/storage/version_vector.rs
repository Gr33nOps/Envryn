//! Per-record causal history, distinct from the scalar [`super::Hlc`].
//!
//! An `Hlc` gives every write a total order -- useful for picking a
//! deterministic winner, but a total order cannot tell you whether two writes
//! were *causally related* (one built on the other) or *concurrent* (neither
//! side had seen the other's edit). Two devices editing the same record while
//! offline from each other produce concurrent writes; a plain HLC comparison
//! silently declares one of them the winner and the other vanishes.
//!
//! A `VersionVector` tracks, per device, the newest `Hlc` that device has
//! contributed to *this specific record*. Comparing two vectors distinguishes
//! "B has seen everything A has" (a fast-forward -- no conflict) from "A and
//! B each know something the other doesn't" (a genuine conflict -- see
//! `storage::Store::upsert_from_sync`, which is the only place this
//! distinction matters).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Hlc;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionVector(BTreeMap<u64, Hlc>);

impl VersionVector {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// A vector with exactly one contribution -- the natural starting point
    /// for a record's first write.
    pub fn single(hlc: Hlc) -> Self {
        let mut m = BTreeMap::new();
        m.insert(hlc.device_id, hlc);
        Self(m)
    }

    /// Record a new local tick from `hlc.device_id`, leaving every other
    /// device's prior entry untouched. A local edit is built on whatever this
    /// vector already represented, so it inherits that knowledge and adds its
    /// own.
    pub fn advanced_by(&self, hlc: Hlc) -> Self {
        let mut m = self.0.clone();
        m.insert(hlc.device_id, hlc);
        Self(m)
    }

    /// True if `self` has observed everything `other` has: every device
    /// entry `other` carries is present in `self` with an equal-or-newer
    /// `Hlc`. Equal vectors dominate each other (reflexive), which is the
    /// right call for "nothing new arrived."
    pub fn dominates(&self, other: &Self) -> bool {
        other
            .0
            .iter()
            .all(|(device, hlc)| self.0.get(device).is_some_and(|mine| mine >= hlc))
    }

    /// Component-wise max: the vector that has observed everything both
    /// inputs have. The natural result of reconciling two branches, whether
    /// one dominated the other or they were genuinely concurrent.
    pub fn merged_with(&self, other: &Self) -> Self {
        let mut m = self.0.clone();
        for (device, hlc) in &other.0 {
            m.entry(*device)
                .and_modify(|existing| {
                    if hlc > existing {
                        *existing = *hlc;
                    }
                })
                .or_insert(*hlc);
        }
        Self(m)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.0).unwrap_or_else(|_| "{}".to_string())
    }

    /// Malformed or missing storage is treated as an empty vector rather than
    /// an error -- the worst case is a false "no conflict" fast-forward the
    /// very first time a pre-v3 row is touched, not a refusal to sync.
    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).map(Self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hlc(wall_ms: i64, counter: u32, device_id: u64) -> Hlc {
        Hlc {
            wall_ms,
            counter,
            device_id,
        }
    }

    #[test]
    fn a_vector_dominates_itself() {
        let v = VersionVector::single(hlc(100, 0, 1));
        assert!(v.dominates(&v));
    }

    #[test]
    fn advancing_by_the_same_device_supersedes_its_own_entry() {
        let v1 = VersionVector::single(hlc(100, 0, 1));
        let v2 = v1.advanced_by(hlc(200, 0, 1));
        assert!(v2.dominates(&v1));
        assert!(!v1.dominates(&v2));
    }

    #[test]
    fn advancing_by_a_different_device_does_not_lose_the_first() {
        let v1 = VersionVector::single(hlc(100, 0, 1));
        let v2 = v1.advanced_by(hlc(50, 0, 2));
        assert!(v2.dominates(&v1));
    }

    #[test]
    fn independent_edits_from_two_devices_are_incomparable() {
        let base = VersionVector::single(hlc(100, 0, 1));
        let branch_a = base.advanced_by(hlc(200, 0, 1));
        let branch_b = base.advanced_by(hlc(150, 0, 2));
        assert!(!branch_a.dominates(&branch_b));
        assert!(!branch_b.dominates(&branch_a));
    }

    #[test]
    fn merging_two_branches_dominates_both() {
        let base = VersionVector::single(hlc(100, 0, 1));
        let branch_a = base.advanced_by(hlc(200, 0, 1));
        let branch_b = base.advanced_by(hlc(150, 0, 2));
        let merged = branch_a.merged_with(&branch_b);
        assert!(merged.dominates(&branch_a));
        assert!(merged.dominates(&branch_b));
    }

    #[test]
    fn json_round_trips() {
        let v = VersionVector::single(hlc(100, 5, 42)).advanced_by(hlc(90, 1, 7));
        let json = v.to_json();
        assert_eq!(VersionVector::from_json(&json), v);
    }

    #[test]
    fn malformed_json_becomes_an_empty_vector_rather_than_an_error() {
        assert_eq!(VersionVector::from_json("not json"), VersionVector::new());
        assert_eq!(VersionVector::from_json(""), VersionVector::new());
    }
}
