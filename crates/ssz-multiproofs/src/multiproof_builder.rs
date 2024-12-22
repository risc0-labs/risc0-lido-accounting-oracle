use crate::error::{Error, Result};
use bitvec::prelude::*;
use sha2::{Digest, Sha256};
#[cfg(feature = "builder")]
use {
    indicatif::{ParallelProgressIterator, ProgressBar, ProgressIterator, ProgressStyle},
    rayon::prelude::*,
    ssz_rs::prelude::{GeneralizedIndex, GeneralizedIndexable, Path, Prove},
    ssz_rs::proofs::Prover,
    std::collections::BTreeSet,
    std::collections::HashSet,
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

        let proof_indices = compute_proof_indices(&gindices);

        let tree = container.compute_tree()?;

        // Provide a custom bar style
        let pb = ProgressBar::new(proof_indices.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] ({pos}/{len}, ETA {eta})",
            )
            .unwrap(),
        );

        let nodes: Vec<_> = proof_indices
            .par_iter()
            .progress_with(pb.clone())
            .map(|index| {
                let mut prover = Prover::from(*index);
                prover.compute_proof_cached_tree(container, &tree)?;
                let proof = prover.into_proof();
                Ok(proof.leaf)
            })
            .collect::<Result<Vec<_>>>()?;

        let value_mask = proof_indices
            .iter()
            .progress_with(pb)
            .map(|index| gindices.contains(index))
            .collect();

        let descriptor = compute_proof_descriptor(&gindices)?;

        Ok(Multiproof {
            nodes,
            descriptor,
            value_mask,
        })
    }
}

pub type Descriptor = BitVec<u8, Msb0>;

#[cfg(feature = "builder")]
pub fn compute_proof_indices(indices: &[GeneralizedIndex]) -> Vec<GeneralizedIndex> {
    let mut indices_set: HashSet<GeneralizedIndex> = HashSet::new();
    for &index in indices {
        let helper_indices = get_helper_indices(&[index]);
        for helper_index in helper_indices {
            indices_set.insert(helper_index);
        }
    }
    for &index in indices {
        let path_indices = get_path_indices(index);
        for path_index in path_indices {
            indices_set.remove(&path_index);
        }
        indices_set.insert(index);
    }
    let mut sorted_indices: Vec<GeneralizedIndex> = indices_set.into_iter().collect();
    sorted_indices.sort_by_key(|index| format!("{:b}", *index));
    sorted_indices
}

#[cfg(feature = "builder")]
pub fn compute_proof_descriptor(indices: &[GeneralizedIndex]) -> Result<Descriptor> {
    let indices = compute_proof_indices(indices);
    let mut descriptor = Descriptor::new();
    for index in indices {
        descriptor.extend(std::iter::repeat(false).take(index.trailing_zeros() as usize));
        descriptor.push(true);
    }
    Ok(descriptor)
}

#[cfg(feature = "builder")]
pub fn get_branch_indices(tree_index: GeneralizedIndex) -> Vec<GeneralizedIndex> {
    let mut focus = sibling(tree_index);
    let mut result = vec![focus];
    while focus > 1 {
        focus = sibling(parent(focus));
        result.push(focus);
    }
    result.truncate(result.len() - 1);
    result
}

#[cfg(feature = "builder")]
pub fn get_path_indices(tree_index: GeneralizedIndex) -> Vec<GeneralizedIndex> {
    let mut focus = tree_index;
    let mut result = vec![focus];
    while focus > 1 {
        focus = parent(focus);
        result.push(focus);
    }
    result.truncate(result.len() - 1);
    result
}

#[cfg(feature = "builder")]
pub fn get_helper_indices(indices: &[GeneralizedIndex]) -> Vec<GeneralizedIndex> {
    let mut all_helper_indices = HashSet::new();
    let mut all_path_indices = HashSet::new();

    for index in indices {
        all_helper_indices.extend(get_branch_indices(*index).iter());
        all_path_indices.extend(get_path_indices(*index).iter());
    }

    let mut all_branch_indices = all_helper_indices
        .difference(&all_path_indices)
        .cloned()
        .collect::<Vec<_>>();
    all_branch_indices.sort_by(|a: &GeneralizedIndex, b: &GeneralizedIndex| b.cmp(a));
    all_branch_indices
}

#[cfg(feature = "builder")]
pub const fn sibling(index: GeneralizedIndex) -> GeneralizedIndex {
    index ^ 1
}

#[cfg(feature = "builder")]
pub const fn parent(index: GeneralizedIndex) -> GeneralizedIndex {
    index / 2
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

    /// mask indicatign which nodes are values (1) or proof supporting nodes (0)
    value_mask: BitVec<u8, Msb0>,

    /// bitvector describing the shape of the proof
    descriptor: BitVec<u8, Msb0>,
}

impl Multiproof {
    /// Verify this multi-proof against a given root
    #[tracing::instrument(skip(self))]
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

    /// Creates an iterator the nodes in this proof along with their gindices
    pub fn nodes(&self) -> impl Iterator<Item = (u64, &Node)> {
        GIndexIterator::new(&self.descriptor).zip(self.nodes.iter())
    }

    /// Creates an iterator the values in this proof along with their gindices
    pub fn values(&self) -> impl Iterator<Item = (u64, &Node)> {
        self.nodes()
            .zip(self.value_mask.iter())
            .filter_map(|(node, is_value)| if *is_value { Some(node) } else { None })
    }

    /// Finds the node corresponding to a given gindex.
    /// Returns None if the gindex is not in the proof.
    ///
    /// Note this is a linear search, so it's not efficient for large proofs.
    /// If you are iterating over all leaves it is much more efficient to use the iterator instead
    pub fn get(&self, gindex: u64) -> Option<&Node> {
        self.values()
            .find(|(g, _)| *g == gindex)
            .map(|(_, node)| node)
    }
}

/// Given a descriptor iterate over the gindices it describes
struct GIndexIterator<'a> {
    descriptor: &'a Descriptor,
    descriptor_index: usize,
    current_gindex: u64,
    stack: Vec<u64>, // Stack to simulate the traversal
}

impl<'a> GIndexIterator<'a> {
    fn new(descriptor: &'a Descriptor) -> Self {
        GIndexIterator {
            descriptor,
            descriptor_index: 0,
            current_gindex: 1,
            stack: vec![1],
        }
    }
}

impl<'a> Iterator for GIndexIterator<'a> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        while self.descriptor_index < self.descriptor.len() {
            let bit = self.descriptor[self.descriptor_index];
            self.descriptor_index += 1;
            if !bit {
                self.stack.push(self.current_gindex);
                self.current_gindex *= 2;
            } else {
                let result = self.current_gindex;
                self.current_gindex = self.stack.pop()? * 2 + 1;
                return Some(result);
            }
        }
        None
    }
}

#[cfg(test)]
mod gtests {
    #[test]
    fn test_gindex_iterator() {
        use super::*;

        let descriptor = bitvec![u8, Msb0; 0,0,1,0,0,1,0,1,1,1,1];
        assert_eq!(
            GIndexIterator::new(&descriptor).collect::<Vec<u64>>(),
            vec![4, 20, 42, 43, 11, 3]
        );
    }
}

struct Pointer {
    bit_index: usize,
    node_index: usize,
}

fn calculate_compact_multi_merkle_root_inner(
    nodes: &[Node],
    descriptor: &Descriptor,
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

#[cfg(feature = "builder")]
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
                &super::Node::from_slice(
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
            Some((gindex as u64, &beacon_state.state_roots[10]))
        );

        test_roundtrip_serialization(&multiproof);
    }
}
