use alloy_primitives::B256;
use alloy_sol_types::sol;
use bitvec::prelude::*;
use risc0_steel::ethereum::EthEvmInput;
use risc0_steel::Commitment;
use ssz_multiproofs::Multiproof;

#[cfg(feature = "builder")]
use {
    crate::io::build_with_versioned_state,
    crate::{InputWithReceipt, Result},
    beacon_state::mainnet::BeaconState,
    ethereum_consensus::phase0::BeaconBlockHeader,
    gindices::presets::mainnet::{
        beacon_block as beacon_block_gindices, beacon_state::post_electra as beacon_state_gindices,
    },
    risc0_zkvm::Receipt,
    ssz_multiproofs::MultiproofBuilder,
    ssz_rs::prelude::*,
};

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
        withdrawal_credentials: B256,
        block_header: &BeaconBlockHeader,
        beacon_state: &BeaconState,
        evm_input: EthEvmInput,
    ) -> Result<Self> {
        let block_root = block_header.hash_tree_root()?;

        let membership = beacon_state
            .validators()
            .iter()
            .map(|v| v.withdrawal_credentials.as_slice() == withdrawal_credentials.as_slice())
            .collect::<BitVec<u32, Lsb0>>();

        tracing::info!("{} Lido validators detected", membership.count_ones());

        let block_multiproof = MultiproofBuilder::new()
            .with_gindex(beacon_block_gindices::slot().try_into()?)
            .with_gindex(beacon_block_gindices::state_root().try_into()?)
            .build(block_header)?;

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

        let state_multiproof = build_with_versioned_state(state_multiproof_builder, beacon_state)?;

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
