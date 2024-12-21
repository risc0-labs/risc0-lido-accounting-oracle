use crate::error::{Error, Result};
use bitvec::prelude::*;
use serde::de::value;
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
        let mut value_indices = Vec::new();

        let nodes: Vec<_> = proof_indices
            .iter() // TODO: parallelize
            .enumerate()
            .map(|(i, index)| {
                let mut prover = Prover::from(*index);
                prover.compute_proof_cached_tree(container, &tree)?;
                let proof = prover.into_proof();
                if gindices.contains(&proof.index) {
                    value_indices.push(i);
                }
                Ok(proof.leaf)
            })
            .collect::<Result<Vec<_>>>()?;

        let descriptor = compact_multiproofs::compute_proof_descriptor(&gindices)?;

        Ok(Multiproof {
            nodes,
            descriptor,
            value_indices,
        })
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
    /// The merkle tree nodes corresponding to both leaves and internal proof nodes
    nodes: Vec<Node>,

    /// Indices into the nodes vector that correspond to the leaf values of interest
    value_indices: Vec<usize>,

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
    /// Note this will also iterate leaves that were not added explicitly
    /// but are still needed to reconstruct the root
    pub fn values(&self) -> LeafNodeIterator {
        LeafNodeIterator::new(&self.descriptor, &self.nodes, &self.value_indices)
    }

    /// Finds the node corresponding to a given gindex.
    /// Returns None if the gindex is not in the proof.
    ///
    /// Note this is a linear search, so it's not efficient for large proofs.
    /// If you are iterating over all leaves it is much more efficient to use the iterator instead
    pub fn get(&self, gindex: u64) -> Option<Node> {
        self.values()
            .find(|(g, _)| *g == gindex)
            .map(|(_, node)| node)
    }
}

pub struct LeafNodeIterator<'a> {
    descriptor: &'a BitVec<u8, Msb0>,
    nodes: &'a Vec<Node>,
    value_indices: &'a [usize],
    descriptor_index: usize,
    proof_index: usize,
    current_gindex: u64,
}

impl<'a> LeafNodeIterator<'a> {
    pub(crate) fn new(
        descriptor: &'a BitVec<u8, Msb0>,
        nodes: &'a Vec<Node>,
        value_indices: &'a [usize],
    ) -> LeafNodeIterator<'a> {
        LeafNodeIterator {
            descriptor,
            nodes,
            value_indices,
            descriptor_index: 0,
            proof_index: 0,
            current_gindex: 1,
        }
    }
}

impl<'a> Iterator for LeafNodeIterator<'a> {
    /// Returns (node vec index, gindex, proof_node)
    type Item = (u64, Node);

    fn next(&mut self) -> Option<Self::Item> {
        while self.descriptor_index < self.descriptor.len() {
            let gindex = self.current_gindex;

            if self.descriptor[self.descriptor_index] {
                // Check if it's a leaf
                // let is_leaf = self.descriptor_index == self.descriptor.len() - 1
                // || self.descriptor[self.descriptor_index + 1];
                let is_value = self.value_indices.contains(&self.proof_index);
                let result = (gindex, self.nodes[self.proof_index]);
                self.proof_index += 1;
                self.descriptor_index += 1;

                if is_value {
                    return Some(result);
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
        let beacon_state = BeaconState::default();

        let builder = MultiproofBuilder::new();
        let multiproof = builder
            .with_path::<BeaconState>(&["validators".into()])
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(&beacon_state.hash_tree_root().unwrap())
            .unwrap();

        // Add a validator to the state
        let mut beacon_state = BeaconState::default();
        beacon_state.validators.push(Default::default());

        let gindex = BeaconState::generalized_index(&[
            "validators".into(),
            0.into(),
            "withdrawal_credentials".into(),
        ])
        .expect("Invalid path for state_roots");

        let multiproof = MultiproofBuilder::new()
            .with_gindex(gindex)
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(&beacon_state.hash_tree_root().unwrap())
            .unwrap();

        assert_eq!(
            multiproof.values().next(),
            Some((
                gindex as u64,
                super::Node::from_slice(
                    beacon_state.validators[0].withdrawal_credentials.as_slice()
                )
            ))
        );

        test_roundtrip_serialization(&multiproof);
    }

    #[test]
    fn test_proving_state_roots() {
        let beacon_state = BeaconState::default();

        let gindex = BeaconState::generalized_index(&["state_roots".into(), 10.into()])
            .expect("Invalid path for state_roots");

        let multiproof = MultiproofBuilder::new()
            .with_gindex(gindex)
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(&beacon_state.hash_tree_root().unwrap())
            .unwrap();

        assert_eq!(
            multiproof.values().next(),
            Some((gindex as u64, beacon_state.state_roots[10]))
        );

        test_roundtrip_serialization(&multiproof);
    }
}
