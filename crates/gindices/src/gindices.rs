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
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ssz_rs::prelude::*;

    #[test]
    fn block_state_root() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::phase0::presets::mainnet::BeaconBlock::generalized_index(&[
                "state_root".into(),
            ])? as u64,
            presets::mainnet::beacon_block::state_root()
        );
        Ok(())
    }

    #[test]
    fn block_slot() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::phase0::presets::mainnet::BeaconBlock::generalized_index(&[
                "slot".into(),
            ])? as u64,
            presets::mainnet::beacon_block::slot()
        );
        Ok(())
    }

    #[test]
    fn slot() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
                "slot".into(),
            ])? as u64,
            presets::mainnet::beacon_state::slot()
        );
        Ok(())
    }

    #[test]
    fn validator_count() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
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
                ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
                    "state_roots".into(),
                    index.into(),
                ])? as u64,
                presets::mainnet::beacon_state::state_roots(index as u64)
            );
        }
        Ok(())
    }

    #[test]
    fn validator_balance() -> anyhow::Result<()> {
        for index in 0_usize..1_000_000 {
            assert_eq!(
                ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
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
                ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
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
                ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
                    "validators".into(),
                    index.into(),
                    "exit_epoch".into(),
                ])? as u64,
                presets::mainnet::beacon_state::validator_exit_epoch(index as u64)
            );
        }
        Ok(())
    }
}
