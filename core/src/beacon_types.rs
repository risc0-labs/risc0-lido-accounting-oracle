//! Types used in the beacon chain.
//! These are stripped down types with one single purpose: to convert paths into g-indices
//! They are also restricted even further to only support the fields of interest for the lido-oracle
//!
//! Do not use these for any other purpose, instead prefer the fully specified types from ethereum_consensus or another crate
//!
use ssz_rs::prelude::*;

type Root = Node;
type Slot = u64;
type Epoch = u64;
type Gwei = u64;

type Bytes32 = Vector<u8, 32>;

pub mod presets {
    pub mod mainnet {
        pub const SLOTS_PER_HISTORICAL_ROOT: usize = 8192;
        pub const VALIDATOR_REGISTRY_LIMIT: usize = 1099511627776;

        pub type BeaconState =
            super::super::BeaconState<SLOTS_PER_HISTORICAL_ROOT, VALIDATOR_REGISTRY_LIMIT>;
    }
}

#[derive(Default, Debug, SimpleSerialize)]
pub struct BeaconState<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
> {
    genesis_time: u64,
    genesis_validators_root: Root,
    slot: Slot,
    fork: Node,
    latest_block_header: Node,
    block_roots: Node,
    state_roots: Vector<Root, SLOTS_PER_HISTORICAL_ROOT>,
    historical_roots: Node,
    eth1_data: Node,
    eth1_data_votes: Node,
    eth1_deposit_index: u64,
    validators: List<Validator, VALIDATOR_REGISTRY_LIMIT>,
    balances: List<Gwei, VALIDATOR_REGISTRY_LIMIT>,
    randao_mixes: Node,
    slashings: Node,
    previous_epoch_attestations: Node,
    current_epoch_attestations: Node,
    justification_bits: Node,
    previous_justified_checkpoint: Node,
    current_justified_checkpoint: Node,
    finalized_checkpoint: Node,
}

#[derive(Default, Debug, SimpleSerialize)]
pub struct Validator {
    public_key: Node,
    withdrawal_credentials: Bytes32,
    effective_balance: Gwei,
    slashed: bool,
    // Status epochs
    activation_eligibility_epoch: Epoch,
    activation_epoch: Epoch,
    exit_epoch: Epoch,
    withdrawable_epoch: Epoch,
}

pub fn validator_balance_gindex<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
>(
    validataor_index: usize,
) -> anyhow::Result<GeneralizedIndex> {
    let gindex =
        BeaconState::<SLOTS_PER_HISTORICAL_ROOT, VALIDATOR_REGISTRY_LIMIT>::generalized_index(&[
            "balances".into(),
            validataor_index.into(),
        ])?;
    Ok(gindex)
}

pub fn validator_withdrawal_credentials_gindex<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
>(
    validataor_index: usize,
) -> anyhow::Result<GeneralizedIndex> {
    let gindex =
        BeaconState::<SLOTS_PER_HISTORICAL_ROOT, VALIDATOR_REGISTRY_LIMIT>::generalized_index(&[
            "validators".into(),
            validataor_index.into(),
            "withdrawal_credentials".into(),
        ])?;
    Ok(gindex)
}

pub fn validator_exit_epoch_gindex<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
>(
    validataor_index: usize,
) -> anyhow::Result<GeneralizedIndex> {
    let gindex =
        BeaconState::<SLOTS_PER_HISTORICAL_ROOT, VALIDATOR_REGISTRY_LIMIT>::generalized_index(&[
            "validators".into(),
            validataor_index.into(),
            "exit_epoch".into(),
        ])?;
    Ok(gindex)
}

#[cfg(test)]
mod test {
    use super::*;

    type MainnetBeaconState = BeaconState<
        { ethereum_consensus::phase0::presets::mainnet::SLOTS_PER_HISTORICAL_ROOT },
        { ethereum_consensus::phase0::presets::mainnet::VALIDATOR_REGISTRY_LIMIT },
    >;

    #[test]
    fn ensure_same_gindices_as_ethereum_consensus_types() -> anyhow::Result<()> {
        let paths = vec![
            vec!["validators".into()],
            vec![
                "validators".into(),
                0.into(),
                "withdrawal_credentials".into(),
            ],
            vec!["balances".into(), 99.into()],
            vec!["state_roots".into(), 5.into()],
        ];
        for path in paths {
            assert_eq!(
                ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(
                    &path
                )?,
                MainnetBeaconState::generalized_index(&path)?
            );
        }
        Ok(())
    }
}
