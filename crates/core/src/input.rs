#[cfg(feature = "builder")]
use alloy_primitives::Address;
use alloy_primitives::B256;
use bitvec::prelude::*;
#[cfg(feature = "builder")]
use risc0_steel::alloy::providers::Provider;
#[cfg(feature = "builder")]
use risc0_steel::ethereum::EthChainSpec;
use risc0_steel::ethereum::EthEvmInput;
use risc0_zkvm::{Digest, Receipt};
use ssz_multiproofs::Multiproof;

#[cfg(feature = "builder")]
use {
    crate::build_with_versioned_state,
    crate::Result,
    beacon_state::mainnet::BeaconState,
    ethereum_consensus::phase0::BeaconBlockHeader,
    gindices::presets::mainnet::{
        beacon_block as beacon_block_gindices, beacon_state::post_electra as beacon_state_gindices,
    },
    risc0_steel::Account,
    ssz_multiproofs::MultiproofBuilder,
    ssz_rs::prelude::*,
};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Input<'a> {
    /// The Program ID of this program. Need to accept it as input rather than hard-code otherwise it creates a cyclic hash reference
    /// This MUST be written to the journal and checked by the verifier! See https://github.com/risc0/risc0-ethereum/blob/main/contracts/src/RiscZeroSetVerifier.sol#L114
    pub self_program_id: Digest,

    /// Block that the proof is rooted in
    pub block_root: B256,

    /// Merkle SSZ proof rooted in the beacon block
    #[serde(borrow)]
    pub block_multiproof: Multiproof<'a>,

    /// Merkle SSZ proof rooted in the beacon state
    #[serde(borrow)]
    pub state_multiproof: Multiproof<'a>,

    /// Steel EvmInput, used for reading the withdrawal vault balance
    pub evm_input: EthEvmInput,

    /// If this proof is a continuation, the membership status of the validators
    #[serde(borrow)]
    pub proof_type: ProofType<'a>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ProofType<'a> {
    Initial,
    Continuation {
        #[serde(borrow)]
        cont_type: ContinuationType<'a>,
        /// Journal to verify the previous proof
        prior_receipt: Receipt,
        /// The prior membership bitfield for the previous proof to be checked against the journal membershipCommitment
        prior_membership: BitVec<u32, Lsb0>,
        /// The slot of the prior proof
        prior_slot: u64,
        /// The state root of the prior proof
        prior_state_root: B256,
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
pub enum ContinuationType<'a> {
    SameSlot,
    ShortRange,
    LongRange {
        /// The historical summary multiproof to verify the historical summary root
        #[serde(borrow)]
        hist_summary_multiproof: Multiproof<'a>,
    },
}

#[cfg(feature = "builder")]
impl<'a> Input<'a> {
    /// Build an oracle proof for all validators in the beacon state
    pub async fn build_initial<D, P>(
        spec: &EthChainSpec,
        self_program_id: D,
        block_header: &BeaconBlockHeader,
        beacon_state: &BeaconState,
        withdrawal_credentials: &B256,
        withdrawal_vault_address: Address,
        provider: P,
    ) -> Result<Self>
    where
        D: Into<Digest>,
        P: Provider + 'static,
    {
        use risc0_steel::ethereum::EthEvmEnv;

        let block_root = block_header.hash_tree_root()?;

        let membership = beacon_state
            .validators()
            .iter()
            .map(|v| v.withdrawal_credentials.as_slice() == withdrawal_credentials.as_slice())
            .collect::<BitVec<u32, Lsb0>>();

        let block_multiproof = MultiproofBuilder::new()
            .with_gindex(beacon_block_gindices::slot().try_into()?)
            .with_gindex(beacon_block_gindices::state_root().try_into()?)
            .build(block_header)?;

        let state_multiproof_builder = MultiproofBuilder::new()
            .with_gindex(beacon_state_gindices::validator_count().try_into()?)
            .with_gindices((0..beacon_state.validators().len()).map(|i| {
                beacon_state_gindices::validator_withdrawal_credentials(i as u64)
                    .try_into()
                    .unwrap()
            }))
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

        let state_multiproof = build_with_versioned_state(state_multiproof_builder, &beacon_state)?;

        // build the Steel input for reading the balance
        let mut env = EthEvmEnv::builder()
            .provider(provider)
            .chain_spec(&spec)
            .build()
            .await
            .unwrap();
        let _preflight_info = {
            let account = Account::preflight(withdrawal_vault_address, &mut env);
            account.bytecode(true).info().await.unwrap()
        };
        let evm_input = env.into_input().await.unwrap();

        Ok(Self {
            self_program_id: self_program_id.into(),
            proof_type: ProofType::Initial,
            block_root,
            block_multiproof,
            state_multiproof,
            evm_input,
        })
    }
}
