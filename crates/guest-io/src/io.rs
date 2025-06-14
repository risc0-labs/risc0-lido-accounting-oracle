// Copyright 2025 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::error::Result;
use alloy_primitives::B256;
use alloy_sol_types::sol;
use bitvec::prelude::*;
use risc0_steel::ethereum::EthEvmInput;
use risc0_zkvm::{sha::Digest, Receipt};
use ssz_multiproofs::Multiproof;
#[cfg(feature = "builder")]
use {
    crate::error::Error,
    beacon_state::mainnet::BeaconState,
    ethereum_consensus::phase0::{presets::mainnet::HistoricalBatch, BeaconBlockHeader},
    gindices::presets::mainnet::{
        beacon_block as beacon_block_gindices, beacon_state::post_electra as beacon_state_gindices,
        beacon_state::SLOTS_PER_HISTORICAL_ROOT, historical_batch as historical_batch_gindices,
    },
    ssz_multiproofs::MultiproofBuilder,
    ssz_rs::prelude::*,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct InputWithReceipt<T> {
    pub input: T,
    pub receipt: Option<Receipt>,
}

pub mod validator_membership {

    use risc0_zkvm::serde::to_vec;

    use super::*;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Input<'a> {
        /// The Program ID of this program. Need to accept it as input rather than hard-code otherwise it creates a cyclic hash reference
        /// This MUST be written to the journal and checked by the verifier! See https://github.com/risc0/risc0-ethereum/blob/main/contracts/src/RiscZeroSetVerifier.sol#L114
        pub self_program_id: Digest,

        /// The state root of the state used in the current proof
        pub state_root: B256,

        /// the top validator index the membership proof will be extended to
        pub max_validator_index: u64,

        /// If this the first proof in the sequence, or a continuation that consumes an existing proof
        pub proof_type: ProofType,

        /// Merkle SSZ proof rooted in the beacon state
        #[serde(borrow)]
        pub multiproof: Multiproof<'a>,

        /// Merkle SSZ proof rooted in an intermediate beacon state
        pub hist_summary_multiproof: Option<Multiproof<'a>>,
    }

    #[cfg(feature = "builder")]
    impl<'a> Input<'a> {
        #[tracing::instrument(skip(beacon_state, max_validator_index, self_program_id))]
        pub fn build_initial<D: Into<Digest>>(
            beacon_state: BeaconState,
            max_validator_index: u64,
            self_program_id: D,
        ) -> Result<Self> {
            let state_root = beacon_state.hash_tree_root()?;

            let proof_builder =
                MultiproofBuilder::new().with_gindices((0..=max_validator_index).map(|i| {
                    beacon_state_gindices::validator_withdrawal_credentials(i)
                        .try_into()
                        .unwrap()
                }));

            let multiproof = build_with_versioned_state(proof_builder, &beacon_state)?;

            Ok(Self {
                self_program_id: self_program_id.into(),
                state_root,
                max_validator_index,
                proof_type: ProofType::Initial,
                multiproof,
                hist_summary_multiproof: None,
            })
        }

        #[tracing::instrument(skip(
            prior_beacon_state,
            prior_max_validator_index,
            beacon_state,
            max_validator_index,
            self_program_id
        ))]
        pub fn build_continuation<D: Into<Digest>>(
            prior_beacon_state: &BeaconState,
            prior_max_validator_index: u64,
            beacon_state: &BeaconState,
            max_validator_index: u64,
            historical_batch: Option<HistoricalBatch>,
            self_program_id: D,
        ) -> Result<Self> {
            let state_root = beacon_state.hash_tree_root()?;
            let slot = beacon_state.slot();
            let prior_slot = prior_beacon_state.slot();

            let mut proof_builder = MultiproofBuilder::new().with_gindices(
                (prior_max_validator_index + 1..=max_validator_index).map(|i| {
                    beacon_state_gindices::validator_withdrawal_credentials(i)
                        .try_into()
                        .unwrap()
                }),
            );

            let prior_membership = prior_beacon_state
                .validators()
                .iter()
                .take((prior_max_validator_index + 1) as usize)
                .map(|v| {
                    v.withdrawal_credentials.as_slice() == crate::WITHDRAWAL_CREDENTIALS.as_slice()
                })
                .collect::<BitVec<u32, Lsb0>>();

            let (cont_type, hist_summary_multiproof) = if slot == prior_slot {
                (ContinuationType::SameSlot, None)
            } else if slot <= prior_slot + SLOTS_PER_HISTORICAL_ROOT {
                proof_builder = proof_builder
                    .with_gindex(beacon_state_gindices::state_roots(prior_slot).try_into()?);
                (ContinuationType::ShortRange, None)
            } else if let Some(historical_batch) = historical_batch {
                proof_builder = proof_builder.with_gindex(
                    beacon_state_gindices::historical_summaries(prior_slot).try_into()?,
                );
                let hist_summary_multiproof = MultiproofBuilder::new()
                    .with_gindex(historical_batch_gindices::state_roots(prior_slot).try_into()?)
                    .build(&historical_batch, Option::<(_, usize)>::None)?;
                (ContinuationType::LongRange, Some(hist_summary_multiproof))
            } else {
                return Err(Error::MissingHistoricalBatch);
            };

            let multiproof = build_with_versioned_state(proof_builder, beacon_state)?;

            Ok(Self {
                self_program_id: self_program_id.into(),
                state_root,
                max_validator_index,
                proof_type: ProofType::Continuation {
                    prior_state_root: prior_beacon_state.hash_tree_root()?,
                    prior_slot,
                    prior_max_validator_index,
                    prior_membership,
                    cont_type,
                },
                multiproof,
                hist_summary_multiproof,
            })
        }

        pub fn without_receipt(self) -> InputWithReceipt<Self> {
            InputWithReceipt {
                input: self,
                receipt: None,
            }
        }

        pub fn with_receipt(self, receipt: Receipt) -> InputWithReceipt<Self> {
            InputWithReceipt {
                input: self,
                receipt: Some(receipt),
            }
        }
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub enum ProofType {
        Initial,
        Continuation {
            prior_state_root: B256,
            prior_slot: u64,
            prior_max_validator_index: u64,
            prior_membership: BitVec<u32, Lsb0>,
            cont_type: ContinuationType,
        },
    }

    /// Continuations proofs are slightly different depending on how far back the prior proof is.
    /// There are three possibilities here. Either
    /// 1. They are in the same slot
    ///     Just prove the prior state root is the same as the current state root
    /// 2. prior_slot < slot <= prior_slot + SLOTS_PER_HISTORICAL_ROOT
    ///    Prove the prior state root is in the state_roots list of the current state at (prior_slot % SLOTS_PER_HISTORICAL_ROOT)
    /// 3. slot > prior_slot + SLOTS_PER_HISTORICAL_ROOT
    ///     This requires doing an extra step. In this case prove an entry in the historical_summaries list of the current state
    ///     and then prove the prior state root is in the state_roots list of the historical summary.
    ///    The element in the historical_summaries list is at index (prior_slot - CAPELLA_FORK_SLOT) / SLOTS_PER_HISTORICAL_ROOT
    ///    and the index in the state_roots list is (prior_slot % SLOTS_PER_HISTORICAL_ROOT).
    ///    This also requires fetching the state at slot ( (prior_slot / SLOTS_PER_HISTORICAL_ROOT + 1) * SLOTS_PER_HISTORICAL_ROOT )
    ///    to retrieve its state_roots list and build a merkle proof into it
    ///
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub enum ContinuationType {
        SameSlot,
        ShortRange,
        LongRange,
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Journal {
        pub self_program_id: Digest,
        pub state_root: B256,
        pub max_validator_index: u64,
        pub membership: BitVec<u32, Lsb0>,
    }

    impl Journal {
        pub fn to_bytes(&self) -> Result<Vec<u8>> {
            Ok(bytemuck::cast_slice(&to_vec(self)?).to_vec())
        }
    }
}

pub mod balance_and_exits {
    use risc0_steel::Commitment;

    use super::*;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Input<'a> {
        /// Block that the proof is rooted in
        pub block_root: B256,

        /// Bitfield indicating which validators are members of the Lido set
        pub membership: BitVec<u32, Lsb0>,

        /// Merkle SSZ proof rooted in the beacon block
        #[serde(borrow)]
        pub block_multiproof: Multiproof<'a>,

        /// Merkle SSZ proof rooted in the beacon state
        #[serde(borrow)]
        pub state_multiproof: Multiproof<'a>,

        pub evm_input: EthEvmInput,
    }

    #[cfg(feature = "builder")]
    impl Input<'_> {
        #[tracing::instrument(skip(block_header, beacon_state, evm_input))]
        pub fn build(
            block_header: &BeaconBlockHeader,
            beacon_state: &BeaconState,
            evm_input: EthEvmInput,
        ) -> Result<Self> {
            let block_root = block_header.hash_tree_root()?;

            let membership = beacon_state
                .validators()
                .iter()
                .map(|v| {
                    v.withdrawal_credentials.as_slice() == crate::WITHDRAWAL_CREDENTIALS.as_slice()
                })
                .collect::<BitVec<u32, Lsb0>>();

            tracing::info!("{} Lido validators detected", membership.count_ones());

            let block_multiproof = MultiproofBuilder::new()
                .with_gindex(beacon_block_gindices::slot().try_into()?)
                .with_gindex(beacon_block_gindices::state_root().try_into()?)
                .build(block_header, Option::<(_, usize)>::None)?;

            let state_multiproof_builder = MultiproofBuilder::new()
                .with_gindex(beacon_state_gindices::validator_count().try_into()?)
                .with_gindices(membership.iter_ones().map(|i| {
                    beacon_state_gindices::validator_balance(i as u64)
                        .try_into()
                        .unwrap()
                }))
                .with_gindices(membership.iter_ones().map(|i| {
                    beacon_state_gindices::validator_exit_epoch(i as u64)
                        .try_into()
                        .unwrap()
                }));

            let state_multiproof =
                build_with_versioned_state(state_multiproof_builder, beacon_state)?;

            Ok(Self {
                block_root,
                membership,
                block_multiproof,
                state_multiproof,
                evm_input,
            })
        }

        pub fn with_receipt(self, receipt: Receipt) -> InputWithReceipt<Self> {
            InputWithReceipt {
                input: self,
                receipt: Some(receipt),
            }
        }
    }

    sol! {
        struct Journal {
            uint256 clBalanceGwei;
            uint256 withdrawalVaultBalanceWei;
            uint256 totalDepositedValidators;
            uint256 totalExitedValidators;
            bytes32 blockRoot;
            Commitment commitment;
        }
    }
}

#[cfg(feature = "builder")]
fn build_with_versioned_state(
    builder: MultiproofBuilder,
    beacon_state: &BeaconState,
) -> Result<Multiproof<'static>> {
    use beacon_state::mainnet::ElectraBeaconState;

    match beacon_state {
        BeaconState::Phase0(b) => Ok(builder.build(
            b,
            Some((
                BeaconState::generalized_index(&["validators".into()]).unwrap(),
                beacon_state.validators().clone(),
            )),
        )?),
        BeaconState::Altair(b) => Ok(builder.build(
            b,
            Some((
                BeaconState::generalized_index(&["validators".into()]).unwrap(),
                beacon_state.validators().clone(),
            )),
        )?),
        BeaconState::Bellatrix(b) => Ok(builder.build(
            b,
            Some((
                BeaconState::generalized_index(&["validators".into()]).unwrap(),
                beacon_state.validators().clone(),
            )),
        )?),
        BeaconState::Capella(b) => Ok(builder.build(
            b,
            Some((
                BeaconState::generalized_index(&["validators".into()]).unwrap(),
                beacon_state.validators().clone(),
            )),
        )?),
        BeaconState::Deneb(b) => Ok(builder.build(
            b,
            Some((
                BeaconState::generalized_index(&["validators".into()]).unwrap(),
                beacon_state.validators().clone(),
            )),
        )?),
        BeaconState::Electra(b) => Ok(builder.build(
            b,
            Some((
                ElectraBeaconState::generalized_index(&["validators".into()]).unwrap(),
                beacon_state.validators().clone(),
            )),
        )?),
    }
}
