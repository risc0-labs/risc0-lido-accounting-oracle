#[cfg(feature = "builder")]
use {
    ssz_rs::multiproofs::get_helper_indices,
    ssz_rs::prelude::{GeneralizedIndex, GeneralizedIndexable, Path, Prove},
    std::collections::BTreeSet,
};

use crate::error::{Error, Result};
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

    // build the multi-proof for a given container
    // the resulting multi-proof will be sorted by descending gindex in both the leaves and proof nodes
    pub fn build<T: Prove + Sync>(self, container: &T) -> Result<Multiproof> {
        let gindices = self.gindices.into_iter().collect::<Vec<_>>();
        let (multiproof, _root) = container.multi_prove_gindices(&gindices)?;
        let leaves = multiproof.leaves;
        let proof = multiproof.branch;
        let proof_indices = get_helper_indices(&gindices);

        // Sort gindices and leaves descending by gindex
        let mut leaves: Vec<_> = gindices.into_iter().map(|i| i as u64).zip(leaves).collect();
        leaves.sort_by(|a, b| b.0.cmp(&a.0));

        // sort proof and proof_indices descending by gindex
        let mut proof: Vec<_> = proof_indices
            .into_iter()
            .map(|i| i as u64)
            .zip(proof)
            .collect();
        proof.sort_by(|a, b| b.0.cmp(&a.0));

        Ok(Multiproof { leaves, proof })
    }
}

/// An abstraction around a SSZ merkle multi-proof
///
/// This is serializable and deserializable an intended to be passed to the ZKVM for verification.
///
/// The way to consume a multiproof is via its IntoIterator implementation.
/// It will iterate over all gindices and leaf values that the proof guarantees inclusion for
/// in order of increasing gindex
///
#[derive(Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Multiproof {
    /// The proof nodes
    proof: Vec<(u64, Node)>,

    /// gindices of the leaves of the tree we want to prove
    leaves: Vec<(u64, Node)>,
}

impl Multiproof {
    /// Verify this multi-proof against a given root
    pub fn verify(&self, root: &Node) -> Result<()> {
        verify_merkle_multiproof(&self.leaves, &self.proof, root)
    }

    /// Returns an iterator over the leaves in order of descending gindex
    pub fn leaves(&self) -> impl Iterator<Item = &(u64, Node)> {
        self.leaves.iter()
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
            .verify(&beacon_state.hash_tree_root().unwrap())
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
            .verify(&beacon_state.hash_tree_root().unwrap())
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
            .verify(&beacon_state.hash_tree_root().unwrap())
            .unwrap();

        test_roundtrip_serialization(&multiproof);
    }
}
