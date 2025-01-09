use crate::multiproof::calculate_max_stack_depth;
use crate::{Descriptor, Multiproof, Result};
use rayon::prelude::*;
use ssz_rs::prelude::{GeneralizedIndex, GeneralizedIndexable, Path, Prove};
use ssz_rs::proofs::Prover;
use std::collections::BTreeSet;
use std::collections::HashSet;
#[cfg(feature = "progress-bar")]
use {
    indicatif::{ParallelProgressIterator, ProgressBar, ProgressIterator, ProgressStyle},
    tracing_indicatif::suspend_tracing_indicatif as suspend_tracing,
};
#[cfg(not(feature = "progress-bar"))]
fn suspend_tracing<F: FnOnce() -> R, R>(f: F) -> R {
    f()
}

/// The only way to create a multiproof is via this builder.
///
/// The usage process is as follows:
/// - A number of gindices/paths are be registered with the builder
/// - Calling `build` with a SSZ container (type that implements `Prove`) results in a multiproof containing the data at those gindices/paths.
///     This will error if any of the gindices/paths are invalid for the container.
#[derive(Debug)]
pub struct MultiproofBuilder {
    gindices: BTreeSet<GeneralizedIndex>,
}

impl Default for MultiproofBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiproofBuilder {
    pub fn new() -> Self {
        Self {
            gindices: BTreeSet::new(),
        }
    }

    /// Register a path in the container for data to be included in the proof
    /// Paths and gindices are equivalent, but paths are more human-readable
    /// This will be used to retrieve the corresponding 32 byte word from the container at build time
    pub fn with_path<T: GeneralizedIndexable>(mut self, path: Path) -> Self {
        self.gindices
            .insert(T::generalized_index(path).expect("Path is not valid for this type"));
        self
    }

    /// Register a single gindex to be included in the proof.
    /// This will be used to retrieve the corresponding 32 byte word from the container at build time
    pub fn with_gindex(mut self, gindex: GeneralizedIndex) -> Self {
        self.gindices.insert(gindex);
        self
    }

    /// Register an iterator of gindices to be included in the proof
    pub fn with_gindices<I>(mut self, gindices: I) -> Self
    where
        I: IntoIterator<Item = GeneralizedIndex>,
    {
        self.gindices.extend(gindices);
        self
    }

    /// Build the multi-proof for a given container
    #[tracing::instrument(skip(self, container))]
    pub fn build<T: Prove + Sync>(self, container: &T) -> Result<Multiproof> {
        let gindices = self.gindices.into_iter().collect::<Vec<_>>();

        let proof_indices = compute_proof_indices(&gindices);

        let tree = container.compute_tree()?;

        let nodes: Vec<_> = suspend_tracing(|| {
            #[cfg(feature = "progress-bar")]
            {
                proof_indices
                    .par_iter()
                    .progress_with(new_progress_bar(
                        "Computing proof nodes",
                        proof_indices.len(),
                    ))
                    .map(|index| {
                        let mut prover = Prover::from(*index);
                        prover.compute_proof_cached_tree(container, &tree)?;
                        let proof = prover.into_proof();
                        Ok(proof.leaf)
                    })
                    .collect::<Result<Vec<_>>>()
            }
            #[cfg(not(feature = "progress-bar"))]
            {
                proof_indices
                    .par_iter()
                    .map(|index| {
                        let mut prover = Prover::from(*index);
                        prover.compute_proof_cached_tree(container, &tree)?;
                        let proof = prover.into_proof();
                        Ok(proof.leaf)
                    })
                    .collect::<Result<Vec<_>>>()
            }
        })?;

        let value_mask = suspend_tracing(|| {
            #[cfg(feature = "progress-bar")]
            {
                proof_indices
                    .iter()
                    .progress_with(new_progress_bar(
                        "Computing value mask",
                        proof_indices.len(),
                    ))
                    .map(|index| gindices.contains(index))
                    .collect()
            }
            #[cfg(not(feature = "progress-bar"))]
            {
                proof_indices
                    .iter()
                    .map(|index| gindices.contains(index))
                    .collect()
            }
        });

        let descriptor = compute_proof_descriptor(&gindices)?;
        let max_stack_depth = calculate_max_stack_depth(&descriptor);

        Ok(Multiproof {
            nodes,
            descriptor,
            value_mask,
            max_stack_depth,
        })
    }
}

fn compute_proof_indices(indices: &[GeneralizedIndex]) -> Vec<GeneralizedIndex> {
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

fn compute_proof_descriptor(indices: &[GeneralizedIndex]) -> Result<Descriptor> {
    let indices = compute_proof_indices(indices);
    let mut descriptor = Descriptor::new();
    for index in indices {
        descriptor.extend(std::iter::repeat(false).take(index.trailing_zeros() as usize));
        descriptor.push(true);
    }
    Ok(descriptor)
}

fn get_branch_indices(tree_index: GeneralizedIndex) -> Vec<GeneralizedIndex> {
    let mut focus = sibling(tree_index);
    let mut result = vec![focus];
    while focus > 1 {
        focus = sibling(parent(focus));
        result.push(focus);
    }
    result.truncate(result.len() - 1);
    result
}

fn get_path_indices(tree_index: GeneralizedIndex) -> Vec<GeneralizedIndex> {
    let mut focus = tree_index;
    let mut result = vec![focus];
    while focus > 1 {
        focus = parent(focus);
        result.push(focus);
    }
    result.truncate(result.len() - 1);
    result
}

fn get_helper_indices(indices: &[GeneralizedIndex]) -> Vec<GeneralizedIndex> {
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

const fn sibling(index: GeneralizedIndex) -> GeneralizedIndex {
    index ^ 1
}

const fn parent(index: GeneralizedIndex) -> GeneralizedIndex {
    index / 2
}

#[cfg(feature = "progress-bar")]
fn new_progress_bar(msg: &'static str, len: usize) -> ProgressBar {
    let pb_style = ProgressStyle::with_template(
        "{msg} {spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] ({pos}/{len}, ETA {eta})",
    )
    .unwrap();
    let pb = ProgressBar::new(len as u64);
    pb.set_message(msg);
    pb.set_style(pb_style);
    pb
}
