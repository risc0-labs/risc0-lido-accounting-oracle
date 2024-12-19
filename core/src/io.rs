use alloy_primitives::B256;
use bitvec::prelude::*;
use risc0_zkvm::sha::Digest;

#[cfg(feature = "builder")]
use {
    crate::error::Result, crate::gindices::presets::mainnet::beacon_state as beacon_state_gindices,
    ethereum_consensus::types::mainnet::BeaconState, ssz_rs::prelude::*,
};

pub mod validator_membership {
    use super::*;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Input {
        /// The Program ID of this program. Need to accept it as input rather than hard-code otherwise it creates a cyclic hash reference
        /// This MUST be written to the journal and checked by the verifier! See https://github.com/risc0/risc0-ethereum/blob/main/contracts/src/RiscZeroSetVerifier.sol#L114
        pub self_program_id: Digest,

        /// The state root of the state used in the current proof
        pub current_state_root: B256,

        /// the top validator index the membership proof will be extended to
        pub up_to_validator_index: u64,

        /// If this the first proof in the sequence, or a continuation that consumes an existing proof
        pub proof_type: ProofType,

        /// Merkle SSZ proof rooted in the beacon state
        pub multiproof: crate::Multiproof,
    }

    #[cfg(feature = "builder")]
    impl Input {
        pub fn build_initial(
            beacon_state: BeaconState,
            up_to_validator_index: u64,
        ) -> Result<Self> {
            let current_state_root = beacon_state.hash_tree_root()?;

            let proof_builder = crate::MultiproofBuilder::new()
                .with_gindex(beacon_state_gindices::state_roots(0).try_into()?)
                .with_gindices((0..up_to_validator_index).map(|i| {
                    beacon_state_gindices::validator_withdrawal_credentials(i)
                        .try_into()
                        .unwrap()
                }));

            let multiproof = match beacon_state {
                BeaconState::Phase0(bs) => proof_builder.build(&bs),
                BeaconState::Altair(bs) => proof_builder.build(&bs),
                BeaconState::Bellatrix(bs) => proof_builder.build(&bs),
                BeaconState::Capella(bs) => proof_builder.build(&bs),
                BeaconState::Deneb(bs) => proof_builder.build(&bs),
                _ => panic!("Unsupported beacon state type"),
            }?;

            Ok(Self {
                self_program_id: [0_u8; 32].into(),
                current_state_root,
                up_to_validator_index,
                proof_type: ProofType::Initial,
                multiproof,
            })
        }

        pub fn build_continuation(
            prior_beacon_state: BeaconState,
            prior_up_to_validator_index: u64,
            beacon_state: BeaconState,
            up_to_validator_index: u64,
        ) -> Result<Self> {
            let current_state_root = beacon_state.hash_tree_root()?;
            let prior_slot = prior_beacon_state.slot();

            let proof_builder = crate::MultiproofBuilder::new()
                .with_gindex(beacon_state_gindices::state_roots(prior_slot).try_into()?)
                .with_gindices(
                    (prior_up_to_validator_index..up_to_validator_index).map(|i| {
                        beacon_state_gindices::validator_withdrawal_credentials(i)
                            .try_into()
                            .unwrap()
                    }),
                );

            let multiproof = match beacon_state {
                BeaconState::Phase0(bs) => proof_builder.build(&bs),
                BeaconState::Altair(bs) => proof_builder.build(&bs),
                BeaconState::Bellatrix(bs) => proof_builder.build(&bs),
                BeaconState::Capella(bs) => proof_builder.build(&bs),
                BeaconState::Deneb(bs) => proof_builder.build(&bs),
                _ => panic!("Unsupported beacon state type"),
            }?;

            let prior_membership = prior_beacon_state
                .validators()
                .iter()
                .take(prior_up_to_validator_index as usize)
                .map(|v| {
                    v.withdrawal_credentials.as_slice() == crate::WITHDRAWAL_CREDENTIALS.as_slice()
                })
                .collect::<BitVec<u32, Lsb0>>();
            Ok(Self {
                self_program_id: [0_u8; 32].into(),
                current_state_root,
                up_to_validator_index,
                proof_type: ProofType::Continuation {
                    prior_state_root: prior_beacon_state.hash_tree_root()?.into(),
                    prior_slot,
                    prior_up_to_validator_index,
                    prior_membership,
                },
                multiproof,
            })
        }
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub enum ProofType {
        Initial,
        Continuation {
            prior_state_root: B256,
            prior_slot: u64,
            prior_up_to_validator_index: u64,
            prior_membership: BitVec<u32, Lsb0>,
        },
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Journal {
        pub self_program_id: Digest,
        pub state_root: B256,
        pub up_to_validator_index: u64,
        pub membership: BitVec<u32, Lsb0>,
    }
}

pub mod balance_and_exits {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Input {
        /// Block that the proof is rooted in
        pub block_root: B256,

        /// Bitfield indicating which validators are members of the Lido set
        pub membership: BitVec<u32, Lsb0>,

        /// Merkle SSZ proof rooted in the beacon block
        pub block_multiproof: crate::Multiproof,

        /// Merkle SSZ proof rooted in the beacon state
        pub state_multiproof: crate::Multiproof,
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Journal {
        pub block_root: B256,
        pub cl_balance: u64,
        pub num_validators: u64,
        pub num_exited_validators: u64,
    }
}
