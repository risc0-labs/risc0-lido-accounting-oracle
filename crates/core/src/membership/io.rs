use crate::error::Result;
use alloy_primitives::B256;
use bitvec::prelude::*;
use risc0_zkvm::serde::to_vec;
use risc0_zkvm::sha::Digest;
use ssz_multiproofs::Multiproof;
#[cfg(feature = "builder")]
use {
    crate::error::Error,
    crate::io::build_with_versioned_state,
    crate::InputWithReceipt,
    beacon_state::mainnet::BeaconState,
    ethereum_consensus::phase0::presets::mainnet::HistoricalBatch,
    gindices::presets::mainnet::{
        beacon_state::post_electra as beacon_state_gindices,
        beacon_state::SLOTS_PER_HISTORICAL_ROOT, historical_batch as historical_batch_gindices,
    },
    risc0_zkvm::Receipt,
    ssz_multiproofs::MultiproofBuilder,
    ssz_rs::prelude::*,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Input<'a> {
    /// The Program ID of this program. Need to accept it as input rather than hard-code otherwise it creates a cyclic hash reference
    /// This MUST be written to the journal and checked by the verifier! See https://github.com/risc0/risc0-ethereum/blob/main/contracts/src/RiscZeroSetVerifier.sol#L114
    pub self_program_id: Digest,

    /// The state root of the state used in the current proof
    pub state_root: B256,

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
    /// Build an initial proof that proves the membership status of all validators in the beacons state
    pub fn build_initial<D: Into<Digest>>(
        beacon_state: BeaconState,
        self_program_id: D,
    ) -> Result<Self> {
        let state_root = beacon_state.hash_tree_root()?;

        let proof_builder = MultiproofBuilder::new().with_gindices(
            (0..=beacon_state.validators().len()).map(|i| {
                beacon_state_gindices::validator_withdrawal_credentials(i as u64)
                    .try_into()
                    .unwrap()
            }),
        );

        let multiproof = build_with_versioned_state(proof_builder, &beacon_state)?;

        Ok(Self {
            self_program_id: self_program_id.into(),
            state_root,
            proof_type: ProofType::Initial,
            multiproof,
            hist_summary_multiproof: None,
        })
    }

    pub fn build_continuation<D: Into<Digest>>(
        withdrawal_credentials: B256,
        prior_beacon_state: &BeaconState,
        beacon_state: &BeaconState,
        historical_batch: Option<HistoricalBatch>,
        self_program_id: D,
    ) -> Result<Self> {
        let state_root = beacon_state.hash_tree_root()?;
        let slot = beacon_state.slot();
        let prior_slot = prior_beacon_state.slot();

        let mut proof_builder = MultiproofBuilder::new().with_gindices(
            (prior_beacon_state.validators().len()..beacon_state.validators().len()).map(|i| {
                beacon_state_gindices::validator_withdrawal_credentials(i as u64)
                    .try_into()
                    .unwrap()
            }),
        );

        let prior_membership = prior_beacon_state
            .validators()
            .iter()
            .map(|v| v.withdrawal_credentials.as_slice() == withdrawal_credentials.as_slice())
            .collect::<BitVec<u32, Lsb0>>();

        let (cont_type, hist_summary_multiproof) = if slot == prior_slot {
            (ContinuationType::SameSlot, None)
        } else if slot <= prior_slot + SLOTS_PER_HISTORICAL_ROOT {
            proof_builder = proof_builder
                .with_gindex(beacon_state_gindices::state_roots(prior_slot).try_into()?);
            (ContinuationType::ShortRange, None)
        } else if let Some(historical_batch) = historical_batch {
            proof_builder = proof_builder
                .with_gindex(beacon_state_gindices::historical_summaries(prior_slot).try_into()?);
            let hist_summary_multiproof = MultiproofBuilder::new()
                .with_gindex(historical_batch_gindices::state_roots(prior_slot).try_into()?)
                .build(&historical_batch)?;
            (ContinuationType::LongRange, Some(hist_summary_multiproof))
        } else {
            return Err(Error::MissingHistoricalBatch);
        };

        let multiproof = build_with_versioned_state(proof_builder, beacon_state)?;

        Ok(Self {
            self_program_id: self_program_id.into(),
            state_root,
            proof_type: ProofType::Continuation {
                prior_state_root: prior_beacon_state.hash_tree_root()?,
                prior_slot,
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
    pub membership: BitVec<u32, Lsb0>,
}

impl Journal {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(bytemuck::cast_slice(&to_vec(self)?).to_vec())
    }
}
