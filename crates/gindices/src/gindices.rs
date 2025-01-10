// these are hard-coded for now but it should be fairly simple to make them computed
// based on chain spec so it is easier to support multiple chains
pub mod presets {
    pub mod mainnet {

        pub mod beacon_block {
            pub fn slot() -> u64 {
                8
            }

            pub fn state_root() -> u64 {
                11
            }
        }

        pub mod beacon_state {
            pub const SLOTS_PER_HISTORICAL_ROOT: u64 = 8192;
            pub const VALIDATOR_REGISTRY_LIMIT: u64 = 1099511627776;
            pub const CAPELLA_FORK_SLOT: u64 = 6209536;

            pub fn slot() -> u64 {
                34
            }

            pub fn validator_count() -> u64 {
                87
            }

            pub fn state_roots(slot: u64) -> u64 {
                // note this only holds if slot < state.slot <= slot + SLOTS_PER_HISTORICAL_ROOT
                // otherwise the state_root is not available in the state_roots list
                let index = slot % SLOTS_PER_HISTORICAL_ROOT;
                311296 + index
            }

            // Only present in Capella and later
            // The root of a historical summary is the same as a historical batch
            // so this can be used to verify historical batch proofs
            pub fn historical_summaries(slot: u64) -> u64 {
                assert!(
                    slot >= CAPELLA_FORK_SLOT,
                    "Historical summaries are only available from Capella fork onwards"
                );
                let index = (slot - CAPELLA_FORK_SLOT) / SLOTS_PER_HISTORICAL_ROOT;
                1979711488 + index
            }

            // balances are packed 4 u64s into a single 256 bit leaf hence the fiddling here.
            // We can take advantage of this to shrink the proving work where adjacent balances are required
            pub fn validator_balance(validator_index: u64) -> u64 {
                24189255811072 + (validator_index / 4)
            }

            pub fn validator_withdrawal_credentials(validator_index: u64) -> u64 {
                756463999909889 + validator_index * 8
            }

            pub fn validator_exit_epoch(validator_index: u64) -> u64 {
                756463999909894 + validator_index * 8
            }
        }

        pub mod historical_batch {
            pub fn state_roots(slot: u64) -> u64 {
                // note this only holds if slot < state.slot <= slot + SLOTS_PER_HISTORICAL_ROOT
                // otherwise the state_root is not available in the state_roots list
                let index = slot % super::beacon_state::SLOTS_PER_HISTORICAL_ROOT;
                24576 + index
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use presets::mainnet::beacon_state::SLOTS_PER_HISTORICAL_ROOT;
    use ssz_rs::prelude::*;

    #[test]
    fn block_state_root() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::capella::presets::mainnet::BeaconBlock::generalized_index(&[
                "state_root".into(),
            ])? as u64,
            presets::mainnet::beacon_block::state_root()
        );
        Ok(())
    }

    #[test]
    fn block_slot() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::capella::presets::mainnet::BeaconBlock::generalized_index(&[
                "slot".into(),
            ])? as u64,
            presets::mainnet::beacon_block::slot()
        );
        Ok(())
    }

    #[test]
    fn slot() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::capella::presets::mainnet::BeaconState::generalized_index(&[
                "slot".into(),
            ])? as u64,
            presets::mainnet::beacon_state::slot()
        );
        Ok(())
    }

    #[test]
    fn validator_count() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::capella::presets::mainnet::BeaconState::generalized_index(&[
                "validators".into(),
                PathElement::Length,
            ])? as u64,
            presets::mainnet::beacon_state::validator_count()
        );
        Ok(())
    }

    #[test]
    fn state_roots() -> anyhow::Result<()> {
        for index in 0_usize..presets::mainnet::beacon_state::SLOTS_PER_HISTORICAL_ROOT as usize {
            assert_eq!(
                ethereum_consensus::capella::presets::mainnet::BeaconState::generalized_index(&[
                    "state_roots".into(),
                    index.into(),
                ])? as u64,
                presets::mainnet::beacon_state::state_roots(index as u64)
            );
        }
        Ok(())
    }

    #[test]
    fn historical_summaries() -> anyhow::Result<()> {
        for index in 0_u64..10 {
            let slot = presets::mainnet::beacon_state::CAPELLA_FORK_SLOT
                + (index * SLOTS_PER_HISTORICAL_ROOT);
            assert_eq!(
                ethereum_consensus::capella::presets::mainnet::BeaconState::generalized_index(&[
                    "historical_summaries".into(),
                    (index as usize).into(),
                ])? as u64,
                presets::mainnet::beacon_state::historical_summaries(slot as u64)
            );
        }
        Ok(())
    }

    #[test]
    fn validator_balance() -> anyhow::Result<()> {
        for index in 0_usize..1_000_000 {
            assert_eq!(
                ethereum_consensus::capella::presets::mainnet::BeaconState::generalized_index(&[
                    "balances".into(),
                    index.into(),
                ])? as u64,
                presets::mainnet::beacon_state::validator_balance(index as u64)
            );
        }
        Ok(())
    }

    #[test]
    fn validator_withdrawal_credential() -> anyhow::Result<()> {
        for index in 0_usize..1_000_000 {
            assert_eq!(
                ethereum_consensus::capella::presets::mainnet::BeaconState::generalized_index(&[
                    "validators".into(),
                    index.into(),
                    "withdrawal_credentials".into(),
                ])? as u64,
                presets::mainnet::beacon_state::validator_withdrawal_credentials(index as u64)
            );
        }
        Ok(())
    }

    #[test]
    fn validator_exit_epoch() -> anyhow::Result<()> {
        for index in 0_usize..1_000_000 {
            assert_eq!(
                ethereum_consensus::capella::presets::mainnet::BeaconState::generalized_index(&[
                    "validators".into(),
                    index.into(),
                    "exit_epoch".into(),
                ])? as u64,
                presets::mainnet::beacon_state::validator_exit_epoch(index as u64)
            );
        }
        Ok(())
    }

    #[test]
    fn historical_batch_state_root() -> anyhow::Result<()> {
        for index in 0_usize..presets::mainnet::beacon_state::SLOTS_PER_HISTORICAL_ROOT as usize {
            assert_eq!(
                ethereum_consensus::capella::presets::mainnet::HistoricalBatch::generalized_index(
                    &["state_roots".into(), index.into(),]
                )? as u64,
                presets::mainnet::historical_batch::state_roots(index as u64)
            );
        }
        Ok(())
    }
}
