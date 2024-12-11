pub mod presets {
    pub mod mainnet {
        pub const SLOTS_PER_HISTORICAL_ROOT: u64 = 8192;
        pub const VALIDATOR_REGISTRY_LIMIT: u64 = 1099511627776;

        // these are hard-coded for now but it should be fairly simple to make them computed
        // based on chain spec so it is easier to support multiple chains

        pub fn state_roots_gindex(index: u64) -> u64 {
            311296 + index
        }

        // balances are packed 4 u64s into a single 256 bit leaf hence the fiddling here.
        // We can take advantage of this to shrink the proving work by a factor of 4!
        pub fn validator_balance_gindex(validator_index: u64) -> u64 {
            24189255811072 + (validator_index / 4)
        }

        pub fn validator_withdrawal_credentials_gindex(validator_index: u64) -> u64 {
            756463999909889 + validator_index * 8
        }

        pub fn validator_exit_epoch_gindex(validator_index: u64) -> u64 {
            756463999909894 + validator_index * 8
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ssz_rs::prelude::*;

    #[test]
    fn state_roots_gindices_match() -> anyhow::Result<()> {
        for index in 0_usize..presets::mainnet::SLOTS_PER_HISTORICAL_ROOT as usize {
            assert_eq!(
                ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
                    "state_roots".into(),
                    index.into(),
                ])? as u64,
                presets::mainnet::state_roots_gindex(index as u64)
            );
        }
        Ok(())
    }

    #[test]
    fn validator_balance_gindices_match() -> anyhow::Result<()> {
        for index in 0_usize..1_000_000 {
            assert_eq!(
                ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
                    "balances".into(),
                    index.into(),
                ])? as u64,
                presets::mainnet::validator_balance_gindex(index as u64)
            );
        }
        Ok(())
    }

    #[test]
    fn validator_withdrawal_credential_gindices_match() -> anyhow::Result<()> {
        for index in 0_usize..1_000_000 {
            assert_eq!(
                ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
                    "validators".into(),
                    index.into(),
                    "withdrawal_credentials".into(),
                ])? as u64,
                presets::mainnet::validator_withdrawal_credentials_gindex(index as u64)
            );
        }
        Ok(())
    }

    #[test]
    fn validator_exit_epoch_gindices_match() -> anyhow::Result<()> {
        for index in 0_usize..1_000_000 {
            assert_eq!(
                ethereum_consensus::phase0::presets::mainnet::BeaconState::generalized_index(&[
                    "validators".into(),
                    index.into(),
                    "exit_epoch".into(),
                ])? as u64,
                presets::mainnet::validator_exit_epoch_gindex(index as u64)
            );
        }
        Ok(())
    }
}
