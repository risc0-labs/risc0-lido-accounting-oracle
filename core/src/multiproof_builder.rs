use crate::error::{Error, Result};
use crate::verify_merkle_multiproof;
use bitvec::prelude::*;
use sha2::{Digest, Sha256};
#[cfg(feature = "builder")]
use {
    ssz_rs::compact_multiproofs,
    ssz_rs::prelude::{GeneralizedIndex, GeneralizedIndexable, Path, Prove},
    ssz_rs::proofs::Prover,
    std::collections::BTreeSet,
};

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

        let proof_indices = compact_multiproofs::compute_proof_indices(&gindices);

        let tree = container.compute_tree()?;

        let nodes: Vec<_> = proof_indices
            .iter() // TODO: parallelize
            .enumerate()
            .map(|(i, index)| {
                let mut prover = Prover::from(*index);
                prover.compute_proof_cached_tree(container, &tree)?;
                if i % 1000 == 0 {
                    tracing::debug!("computed proof for node: {}/{}", i, proof_indices.len());
                }
                Ok(prover.into_proof().leaf)
            })
            .collect::<Result<Vec<_>>>()?;

        let descriptor = compact_multiproofs::compute_proof_descriptor(&gindices)?;

        Ok(Multiproof { nodes, descriptor })
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
    /// The proof adn leaf nodes
    nodes: Vec<Node>,

    /// bitvector describing the shape of the proof
    descriptor: BitVec<u8, Msb0>,
}

impl Multiproof {
    /// Verify this multi-proof against a given root
    pub fn verify(&self, root: &Node) -> Result<()> {
        if self.calculate_root()? == *root {
            Ok(())
        } else {
            Err(Error::InvalidProof)
        }
    }

    pub fn calculate_root(&self) -> Result<Node> {
        let mut ptr = Pointer {
            bit_index: 0,
            node_index: 0,
        };
        let root =
            calculate_compact_multi_merkle_root_inner(&self.nodes, &self.descriptor, &mut ptr)?;
        if ptr.bit_index != self.descriptor.len() || ptr.node_index != self.nodes.len() {
            Err(Error::InvalidProof)
        } else {
            Ok(root)
        }
    }

    /// Creates an iterator over the leaves of the proof.
    pub fn leaves(&self) -> MerkleProofIterator {
        MerkleProofIterator {
            descriptor: &self.descriptor,
            nodes: &self.nodes,
            descriptor_index: 0,
            proof_index: 0,
            current_gindex: 1,
        }
    }
}

pub struct MerkleProofIterator<'a> {
    descriptor: &'a BitVec<u8, Msb0>,
    nodes: &'a Vec<Node>,
    descriptor_index: usize,
    proof_index: usize,
    current_gindex: u64,
}

impl<'a> Iterator for MerkleProofIterator<'a> {
    /// Returns (gindex, proof_node)
    type Item = (u64, Node);

    fn next(&mut self) -> Option<Self::Item> {
        while self.descriptor_index < self.descriptor.len() {
            let gindex = self.current_gindex;

            if self.descriptor[self.descriptor_index] {
                // Check if it's a leaf
                let is_leaf = self.descriptor_index == self.descriptor.len() - 1
                    || self.descriptor[self.descriptor_index + 1];
                let result = self.nodes[self.proof_index];
                self.proof_index += 1;
                self.descriptor_index += 1;

                if is_leaf {
                    return Some((gindex, result));
                }
            } else {
                // Move to the left or right child
                self.descriptor_index += 1;
            }

            // Update gindex based on traversal
            if self.descriptor[self.descriptor_index - 1] == false {
                // If it's a `0` bit, we're traversing deeper:
                // Left child = gindex * 2, Right child = gindex * 2 + 1
                self.current_gindex *= 2;
            } else {
                // If it's a `1` bit, go back up and adjust accordingly
                while self.current_gindex % 2 == 1 {
                    self.current_gindex /= 2;
                }
                self.current_gindex += 1;
            }
        }

        None
    }
}

struct Pointer {
    bit_index: usize,
    node_index: usize,
}

fn calculate_compact_multi_merkle_root_inner(
    nodes: &[Node],
    descriptor: &BitVec<u8, Msb0>,
    ptr: &mut Pointer,
) -> Result<Node> {
    let bit = descriptor[ptr.bit_index];
    ptr.bit_index += 1;
    if bit {
        let node = nodes[ptr.node_index];
        ptr.node_index += 1;
        Ok(node)
    } else {
        let left = calculate_compact_multi_merkle_root_inner(nodes, descriptor, ptr)?;
        let right = calculate_compact_multi_merkle_root_inner(nodes, descriptor, ptr)?;
        Ok(hash_pair(&left, &right))
    }
}

fn hash_pair(left: &Node, right: &Node) -> Node {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    Node::from_slice(hasher.finalize().as_slice())
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
