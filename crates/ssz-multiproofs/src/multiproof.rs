use bitvec::prelude::*;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::{Descriptor, Node};

/// An abstraction around a SSZ merkle multi-proof
///
/// This is serializable and deserializable an intended to be passed to the ZKVM for verification.
///
/// The most efficient way to consume a multiproof is via its IntoIterator implementation.
/// It will iterate over (gindex, value) tuples for all gindices added when building.
/// Note this will iterate over the values/gindices in depth-first left-to-right order as they appear in the SSZ merkle tree.
/// This will NOT be the order they were added or increasing order of gindex, it will depend on the shape of the data structure.
///
#[derive(Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Multiproof {
    /// The merkle tree nodes corresponding to both leaves and internal proof nodes
    pub(crate) nodes: Vec<Node>,

    /// mask indicating which nodes are values (1) or proof supporting nodes (0)
    pub(crate) value_mask: BitVec<u8, Msb0>,

    /// bitvector describing the shape of the proof. See https://github.com/ethereum/consensus-specs/pull/3148
    pub(crate) descriptor: BitVec<u8, Msb0>,

    /// hint for the depth of the stack needed to verify this proof, useful for preallocation and computing this can be done outside the ZKVM
    pub(crate) max_stack_depth: usize,
}

impl Multiproof {
    /// Verify this multi-proof against a given root
    #[tracing::instrument(skip(self))]
    pub fn verify(&self, root: &Node) -> Result<()> {
        if self.calculate_root()? == *root {
            Ok(())
        } else {
            Err(Error::RootMismatch)
        }
    }

    /// Calculate the root of this multi-proof
    pub fn calculate_root(&self) -> Result<Node> {
        calculate_compact_multi_merkle_root(&self.nodes, &self.descriptor, self.max_stack_depth)
    }

    /// Creates an iterator the nodes in this proof along with their gindices
    pub fn nodes(&self) -> impl Iterator<Item = (u64, &Node)> {
        GIndexIterator::new(&self.descriptor).zip(self.nodes.iter())
    }

    /// Creates an iterator the values in this proof along with their gindices
    pub fn values(&self) -> ValueIterator<impl Iterator<Item = (u64, &Node)>> {
        ValueIterator::new(
            self.nodes()
                .zip(self.value_mask.iter())
                .filter_map(|(node, is_value)| if *is_value { Some(node) } else { None }),
        )
    }

    /// Finds the node corresponding to a given gindex.
    /// Returns None if the gindex is not in the proof.
    ///
    /// Note this is a linear search, so it's not efficient for large proofs.
    /// If there are a lot of values and you want to use them all it is much more efficient to use the iterator instead
    pub fn get(&self, gindex: u64) -> Option<&Node> {
        self.values()
            .find(|(g, _)| *g == gindex)
            .map(|(_, node)| node)
    }
}

/// An iterator over the values in a multiproof along with their gindices
pub struct ValueIterator<'a, I>
where
    I: Iterator<Item = (u64, &'a Node)>,
{
    inner: I,
}

impl<'a, I> ValueIterator<'a, I>
where
    I: Iterator<Item = (u64, &'a Node)>,
{
    fn new(inner: I) -> Self {
        ValueIterator { inner }
    }

    pub fn next_assert_gindex(&mut self, gindex: u64) -> Result<&'a Node> {
        let (g, node) = self.inner.next().ok_or(Error::MissingValue)?;
        if g == gindex {
            Ok(node)
        } else {
            Err(Error::GIndexMismatch {
                expected: gindex,
                actual: g,
            })
        }
    }
}

impl<'a, I> Iterator for ValueIterator<'a, I>
where
    I: Iterator<Item = (u64, &'a Node)>,
{
    type Item = (u64, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Given a descriptor, iterate over the gindices it describes
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

enum TreeNode<'a> {
    Leaf(&'a Node),
    Computed(Node),
    Internal,
}

impl<'a> TreeNode<'a> {
    fn has_value(&self) -> bool {
        matches!(self, TreeNode::Leaf(_)) || matches!(self, TreeNode::Computed(_))
    }

    fn is_internal(&self) -> bool {
        matches!(self, TreeNode::Internal)
    }
}

/// Compute the root of a compact multi-proof given the nodes and descriptor
/// This is the hot path so any optimizations belong here.
fn calculate_compact_multi_merkle_root(
    nodes: &[Node],
    descriptor: &Descriptor,
    stack_depth_hint: usize,
) -> Result<Node> {
    let mut stack = Vec::with_capacity(stack_depth_hint);
    let mut node_index = 0;
    let mut hasher = Sha256::new();
    for bit in descriptor.iter() {
        if *bit {
            stack.push(TreeNode::Leaf(&nodes[node_index]));
            node_index += 1;

            // reduce any leaf pairs on the stack until we can progress no further
            while stack.len() > 2
                && stack[stack.len() - 1].has_value()
                && stack[stack.len() - 2].has_value()
                && stack[stack.len() - 3].is_internal()
            {
                let right = stack.pop().unwrap();
                let left = stack.pop().unwrap();

                match left {
                    TreeNode::Leaf(node) => hasher.update(node),
                    TreeNode::Computed(node) => hasher.update(node),
                    _ => panic!("Expected leaf"),
                }
                match right {
                    TreeNode::Leaf(node) => hasher.update(node),
                    TreeNode::Computed(node) => hasher.update(node),
                    _ => panic!("Expected leaf"),
                }

                stack.pop(); // pop the internal node and replace with the hashed children
                stack.push(TreeNode::Computed(Node::from_slice(
                    &hasher.finalize_reset(),
                )));
            }
        } else {
            stack.push(TreeNode::Internal);
        }
    }
    assert_eq!(stack.len(), 1);
    Ok(match stack.pop().unwrap() {
        TreeNode::Leaf(_) => panic!("root must be computed"),
        TreeNode::Computed(node) => node,
        _ => panic!("Expected leaf"),
    })
}

#[cfg(feature = "builder")]
/// Compute ahead of time what the maximum stack depth will be required to verify a proof with the given descriptor
pub(crate) fn calculate_max_stack_depth(descriptor: &Descriptor) -> usize {
    let mut stack = Vec::new();
    let mut max_stack_depth = 0;
    for bit in descriptor.iter() {
        if *bit {
            stack.push(TreeNode::Computed(Node::default()));
            while stack.len() > 2
                && stack[stack.len() - 1].has_value()
                && stack[stack.len() - 2].has_value()
                && stack[stack.len() - 3].is_internal()
            {
                stack.pop();
                stack.pop();
                stack.pop();
                stack.push(TreeNode::Computed(Node::default()));
                max_stack_depth = max_stack_depth.max(stack.len());
            }
        } else {
            stack.push(TreeNode::Internal);
            max_stack_depth = max_stack_depth.max(stack.len());
        }
    }
    assert_eq!(stack.len(), 1);
    max_stack_depth
}

#[cfg(test)]
mod tests {
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
