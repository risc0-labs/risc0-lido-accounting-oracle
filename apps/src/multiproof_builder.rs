use ethereum_consensus::ssz::prelude::{
    proofs::{Proof, ProofAndWitness, Prover},
    GeneralizedIndex, Node, Path, SimpleSerialize,
};

#[derive(Debug)]
pub struct MultiproofBuilder<T> {
    gindices: Vec<GeneralizedIndex>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: SimpleSerialize> MultiproofBuilder<T> {
    pub fn new() -> Self {
        Self {
            gindices: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_path(mut self, path: Path) -> anyhow::Result<Self> {
        self.gindices.push(T::generalized_index(path)?);
        Ok(self)
    }

    // build the multi-proof for a given
    pub fn build(self, container: &T) -> anyhow::Result<Multiproof> {
        let proofs_and_witnesses = self
            .gindices
            .iter()
            .map(|gindex| {
                let mut prover = Prover::from(*gindex);
                prover.compute_proof(container)?;
                Ok(ProofAndWitness::from(prover).0)
            })
            .collect::<anyhow::Result<Vec<Proof>>>()?;

        Ok(Multiproof {
            proofs: proofs_and_witnesses,
        })
    }
}

/// An abstraction around a SSZ merkle multi-proof
/// Currently this does naive multi-proofs (e.g. no sharing of internal tree nodes)
/// just to get the ball rolling. This can be replaced with proper multi-proofs without changing the API.
// #[derive(serde::Serialize, serde::Deserialize)]
pub struct Multiproof {
    proofs: Vec<Proof>,
}

impl Multiproof {
    /// Verify this multi-proof against a given root
    pub fn verify(&self, root: Node) -> anyhow::Result<()> {
        self.proofs.iter().try_for_each(|proof| {
            proof.verify(root)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethereum_consensus::phase0::presets::mainnet::{
        BeaconState, Validator, VALIDATOR_REGISTRY_LIMIT,
    };
    use ethereum_consensus::ssz::prelude::*;

    #[test]
    fn test_multiproof_builder() {
        let mut beacon_state = BeaconState::default();

        let multiproof = MultiproofBuilder::<BeaconState>::new()
            .with_path(&["validators".into()])
            .unwrap()
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(beacon_state.hash_tree_root().unwrap())
            .unwrap();

        // Add a validator to the state
        beacon_state.validators.push(Default::default());

        let multiproof = MultiproofBuilder::<BeaconState>::new()
            .with_path(&[
                "validators".into(),
                1.into(),
                "withdrawal_credentials".into(),
            ])
            .unwrap()
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(beacon_state.hash_tree_root().unwrap())
            .unwrap();

        let multiproof = MultiproofBuilder::<List<Validator, { VALIDATOR_REGISTRY_LIMIT }>>::new()
            .with_path(&[PathElement::Length])
            .unwrap()
            .build(&beacon_state.validators)
            .unwrap();

        multiproof
            .verify(beacon_state.validators.hash_tree_root().unwrap())
            .unwrap();
    }
}
