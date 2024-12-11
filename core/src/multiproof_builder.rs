use std::collections::BTreeMap;
#[cfg(feature = "builder")]
use {
    ssz_rs::prelude::{GeneralizedIndex, GeneralizedIndexable, Path, Prove},
    std::collections::BTreeSet,
};

use crate::error::Result;
use crate::verify_merkle_multiproof;

pub type Node = alloy_primitives::B256;

#[cfg(feature = "builder")]
#[derive(Debug)]
pub struct MultiproofBuilder {
    gindices: BTreeSet<GeneralizedIndex>,
}

#[cfg(feature = "builder")]
impl MultiproofBuilder {
    pub fn new() -> Self {
        Self {
            gindices: BTreeSet::new(),
        }
    }

    pub fn with_path<T: GeneralizedIndexable>(mut self, path: Path) -> Self {
        self.gindices
            .insert(T::generalized_index(path).expect("Path is not valid for this type"));
        self
    }

    pub fn with_gindex(mut self, gindex: GeneralizedIndex) -> Self {
        self.gindices.insert(gindex);
        self
    }

    pub fn with_gindices<'a, I>(mut self, gindices: I) -> Self
    where
        I: IntoIterator<Item = GeneralizedIndex>,
    {
        self.gindices.extend(gindices);
        self
    }

    // build the multi-proof for a given
    pub fn build<T: Prove>(self, container: &T) -> Result<Multiproof> {
        let gindices = self.gindices.into_iter().collect::<Vec<_>>();
        let (multiproof, _root) = container.multi_prove_gindices(&gindices)?;
        Ok(Multiproof {
            branch: multiproof.branch,
            indexed_leaves: multiproof
                .indices
                .into_iter()
                .map(|i| i as u64)
                .zip(multiproof.leaves.into_iter())
                .collect(),
        })
    }
}

/// An abstraction around a SSZ merkle multi-proof
/// Currently this does naive multi-proofs (e.g. no sharing of internal tree nodes)
/// just to get the ball rolling. This can be replaced with proper multi-proofs without changing the API.
///
/// This is serializable and deserializable an intended to be passed to the ZKVM for verification
///
#[derive(Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Multiproof {
    branch: Vec<Node>,
    indexed_leaves: BTreeMap<u64, Node>,
}

impl Multiproof {
    /// Verify this multi-proof against a given root
    pub fn verify(&self, root: Node) -> Result<()> {
        verify_merkle_multiproof(&self.indexed_leaves, &self.branch, root)
    }

    /// Get the leaf value at a given path with respect to the SSZ type T
    /// If this multiproof has been verified the returned leaf value can be trusted
    pub fn get(&self, gindex: u64) -> Option<&Node> {
        self.indexed_leaves.get(&gindex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethereum_consensus::phase0::presets::mainnet::BeaconState;
    use risc0_zkvm::serde::{from_slice, to_vec};
    use ssz_rs::prelude::*;

    fn test_roundtrip_serialization(multiproof: &Multiproof) {
        let serialized = to_vec(multiproof).unwrap();
        let deserialized: Multiproof = from_slice(&serialized).unwrap();
        assert_eq!(multiproof, &deserialized);
    }

    #[test]
    fn test_proving_validator_fields() {
        let mut beacon_state = BeaconState::default();

        let builder = MultiproofBuilder::new();
        let multiproof = builder
            .with_path::<BeaconState>(&["validators".into()])
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(beacon_state.hash_tree_root().unwrap())
            .unwrap();

        // Add a validator to the state
        beacon_state.validators.push(Default::default());

        let multiproof = MultiproofBuilder::new()
            .with_path::<BeaconState>(&[
                "validators".into(),
                0.into(),
                "withdrawal_credentials".into(),
            ])
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(beacon_state.hash_tree_root().unwrap())
            .unwrap();

        test_roundtrip_serialization(&multiproof);
    }

    #[test]
    fn test_proving_state_roots() {
        let beacon_state = BeaconState::default();

        let multiproof = MultiproofBuilder::new()
            .with_path::<BeaconState>(&["state_roots".into(), 10.into()])
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(beacon_state.hash_tree_root().unwrap())
            .unwrap();

        test_roundtrip_serialization(&multiproof);
    }
}
