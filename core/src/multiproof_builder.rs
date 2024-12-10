use ssz_rs::prelude::{
    proofs::{Proof, ProofAndWitness, Prover},
    GeneralizedIndex, Node, Path, SimpleSerialize,
};

use serde::ser::SerializeSeq;

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
///
/// This is serializable and deserializable an intended to be passed to the ZKVM for verification
///
#[derive(Debug, PartialEq)]
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

    /// Get the leaf value at a given path with respect to the SSZ type T
    /// If this multiproof has been verified the returned leaf value can be trusted
    /// Note this is currently not an efficient way to get leaf values since it iterates over all the proofs
    pub fn get<T: SimpleSerialize>(&self, path: Path) -> Option<Node> {
        let gindex = T::generalized_index(path).ok()?;
        self.proofs.iter().find_map(|proof| {
            if proof.index == gindex {
                Some(proof.leaf)
            } else {
                None
            }
        })
    }
}

impl serde::Serialize for Multiproof {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.proofs.len()))?;
        for proof in &self.proofs {
            seq.serialize_element(&(proof.leaf, proof.branch.clone(), proof.index))?;
        }
        seq.end()
    }
}

impl<'de> serde::Deserialize<'de> for Multiproof {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MultiproofVisitor;

        impl<'de> serde::de::Visitor<'de> for MultiproofVisitor {
            type Value = Multiproof;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a sequence of serialized proofs")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut proofs = Vec::new();
                while let Some((leaf, branch, index)) =
                    seq.next_element::<(Node, Vec<Node>, GeneralizedIndex)>()?
                {
                    proofs.push(Proof {
                        leaf,
                        branch,
                        index,
                    });
                }
                Ok(Multiproof { proofs })
            }
        }

        deserializer.deserialize_seq(MultiproofVisitor)
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
                0.into(),
                "withdrawal_credentials".into(),
            ])
            .unwrap()
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

        let multiproof = MultiproofBuilder::<BeaconState>::new()
            .with_path(&["state_roots".into(), 10.into()])
            .unwrap()
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(beacon_state.hash_tree_root().unwrap())
            .unwrap();

        test_roundtrip_serialization(&multiproof);
    }
}
