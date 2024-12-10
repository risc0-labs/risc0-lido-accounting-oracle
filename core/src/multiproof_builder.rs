use std::collections::BTreeMap;

use ssz_rs::prelude::{
    proofs::{Proof, ProofAndWitness, Prover},
    GeneralizedIndex, Node, Path, SimpleSerialize,
};

use serde::ser::SerializeSeq;

#[derive(Debug)]
pub struct MultiproofBuilder {
    gindices: Vec<GeneralizedIndex>,
}

impl MultiproofBuilder {
    pub fn new() -> Self {
        Self {
            gindices: Vec::new(),
        }
    }

    pub fn with_path<T: SimpleSerialize>(mut self, path: Path) -> anyhow::Result<Self> {
        self.gindices.push(T::generalized_index(path)?);
        Ok(self)
    }

    pub fn with_gindex(mut self, gindex: GeneralizedIndex) -> Self {
        self.gindices.push(gindex);
        self
    }

    pub fn with_gindices<'a, I>(mut self, gindices: I) -> Self
    where
        I: IntoIterator<Item = &'a GeneralizedIndex>,
    {
        self.gindices.extend(gindices);
        self
    }

    // build the multi-proof for a given
    pub fn build<T: SimpleSerialize>(self, container: &T) -> anyhow::Result<Multiproof> {
        let proofs = self
            .gindices
            .iter()
            .map(|gindex| {
                let mut prover = Prover::from(*gindex);
                prover.compute_proof(container)?;
                Ok(ProofAndWitness::from(prover).0)
            })
            .collect::<anyhow::Result<Vec<Proof>>>()?;
        Ok(Multiproof {
            proofs,
            values: None,
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
    /// A lookup table for the leaf values by gindex
    /// This duplicates the leaf values in the proofs but is useful for quick lookups
    /// we might be able to do better with a more efficient data structure
    /// This is not serialized
    values: Option<BTreeMap<GeneralizedIndex, Node>>,
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
    pub fn get(&self, gindex: GeneralizedIndex) -> anyhow::Result<Option<&Node>> {
        if let Some(values) = &self.values {
            Ok(values.get(&gindex))
        } else {
            Err(anyhow::anyhow!(
                "Values lookup not built. Call build_values_lookup() first"
            ))
        }
    }

    pub fn build_values_lookup(&mut self) {
        let values = self
            .proofs
            .iter()
            .map(|proof| (proof.index, proof.leaf))
            .collect();
        self.values = Some(values);
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
                Ok(Multiproof {
                    proofs,
                    values: None,
                })
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

        let multiproof = MultiproofBuilder::new()
            .with_path::<BeaconState>(&["validators".into()])
            .unwrap()
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

        let multiproof = MultiproofBuilder::new()
            .with_path::<BeaconState>(&["state_roots".into(), 10.into()])
            .unwrap()
            .build(&beacon_state)
            .unwrap();

        multiproof
            .verify(beacon_state.hash_tree_root().unwrap())
            .unwrap();

        test_roundtrip_serialization(&multiproof);
    }
}
