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

            pub mod post_electra {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/gen_post_electra.rs"));

                pub fn state_roots(slot: u64) -> u64 {
                    // note this only holds if slot < state.slot <= slot + SLOTS_PER_HISTORICAL_ROOT
                    // otherwise the state_root is not available in the state_roots list
                    let index = slot % SLOTS_PER_HISTORICAL_ROOT;
                    state_roots_base() + index
                }

                pub fn historical_summaries(slot: u64) -> u64 {
                    assert!(
                        slot >= CAPELLA_FORK_SLOT,
                        "Historical summaries are only available from Capella fork onwards"
                    );
                    let index = (slot - CAPELLA_FORK_SLOT) / SLOTS_PER_HISTORICAL_ROOT;
                    historical_summaries_base() + index
                }

                pub fn validator_balance(validator_index: u64) -> u64 {
                    validator_balance_base() + (validator_index / 4)
                }

                pub fn validator_withdrawal_credentials(validator_index: u64) -> u64 {
                    validator_withdrawal_credentials_base() + validator_index * 8
                }

                pub fn validator_exit_epoch(validator_index: u64) -> u64 {
                    validator_exit_epoch_base() + validator_index * 8
                }
            }

            pub mod pre_electra {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/gen_pre_electra.rs"));

                pub fn state_roots(slot: u64) -> u64 {
                    // note this only holds if slot < state.slot <= slot + SLOTS_PER_HISTORICAL_ROOT
                    // otherwise the state_root is not available in the state_roots list
                    let index = slot % SLOTS_PER_HISTORICAL_ROOT;
                    state_roots_base() + index
                }

                pub fn historical_summaries(slot: u64) -> u64 {
                    assert!(
                        slot >= CAPELLA_FORK_SLOT,
                        "Historical summaries are only available from Capella fork onwards"
                    );
                    let index = (slot - CAPELLA_FORK_SLOT) / SLOTS_PER_HISTORICAL_ROOT;
                    historical_summaries_base() + index
                }

                pub fn validator_balance(validator_index: u64) -> u64 {
                    validator_balance_base() + (validator_index / 4)
                }

                pub fn validator_withdrawal_credentials(validator_index: u64) -> u64 {
                    validator_withdrawal_credentials_base() + validator_index * 8
                }

                pub fn validator_exit_epoch(validator_index: u64) -> u64 {
                    validator_exit_epoch_base() + validator_index * 8
                }
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
    // use beacon_state::mainnet::ElectraBeaconState as BeaconState;
    use presets::mainnet::beacon_state::SLOTS_PER_HISTORICAL_ROOT;
    use ssz_rs::prelude::*;

    #[test]
    fn block_state_root() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::electra::presets::mainnet::BeaconBlock::generalized_index(&[
                "state_root".into(),
            ])? as u64,
            presets::mainnet::beacon_block::state_root()
        );
        Ok(())
    }

    #[test]
    fn block_slot() -> anyhow::Result<()> {
        assert_eq!(
            ethereum_consensus::electra::presets::mainnet::BeaconBlock::generalized_index(&[
                "slot".into(),
            ])? as u64,
            presets::mainnet::beacon_block::slot()
        );
        Ok(())
    }

    mod pre_electra {
        use super::*;
        use ethereum_consensus::capella::presets::mainnet::BeaconState;

        #[test]
        fn slot() -> anyhow::Result<()> {
            assert_eq!(
                BeaconState::generalized_index(&["slot".into(),])? as u64,
                presets::mainnet::beacon_state::pre_electra::slot()
            );
            Ok(())
        }

        #[test]
        fn validator_count() -> anyhow::Result<()> {
            assert_eq!(
                BeaconState::generalized_index(&["validators".into(), PathElement::Length,])?
                    as u64,
                presets::mainnet::beacon_state::pre_electra::validator_count()
            );
            Ok(())
        }

        #[test]
        fn state_roots() -> anyhow::Result<()> {
            for index in 0_usize..presets::mainnet::beacon_state::SLOTS_PER_HISTORICAL_ROOT as usize
            {
                assert_eq!(
                    BeaconState::generalized_index(&["state_roots".into(), index.into(),])? as u64,
                    presets::mainnet::beacon_state::pre_electra::state_roots(index as u64)
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
                    BeaconState::generalized_index(&[
                        "historical_summaries".into(),
                        (index as usize).into(),
                    ])? as u64,
                    presets::mainnet::beacon_state::pre_electra::historical_summaries(slot as u64)
                );
            }
            Ok(())
        }

        #[test]
        fn validator_balance() -> anyhow::Result<()> {
            for index in 0_usize..1_000_000 {
                assert_eq!(
                    BeaconState::generalized_index(&["balances".into(), index.into(),])? as u64,
                    presets::mainnet::beacon_state::pre_electra::validator_balance(index as u64)
                );
            }
            Ok(())
        }

        #[test]
        fn validator_withdrawal_credential() -> anyhow::Result<()> {
            for index in 0_usize..1_000_000 {
                assert_eq!(
                    BeaconState::generalized_index(&[
                        "validators".into(),
                        index.into(),
                        "withdrawal_credentials".into(),
                    ])? as u64,
                    presets::mainnet::beacon_state::pre_electra::validator_withdrawal_credentials(
                        index as u64
                    )
                );
            }
            Ok(())
        }

        #[test]
        fn validator_exit_epoch() -> anyhow::Result<()> {
            for index in 0_usize..1_000_000 {
                assert_eq!(
                    BeaconState::generalized_index(&[
                        "validators".into(),
                        index.into(),
                        "exit_epoch".into(),
                    ])? as u64,
                    presets::mainnet::beacon_state::pre_electra::validator_exit_epoch(index as u64)
                );
            }
            Ok(())
        }
    }

    mod post_electra {
        use super::*;
        use ethereum_consensus::electra::presets::mainnet::BeaconState;

        #[test]
        fn slot() -> anyhow::Result<()> {
            assert_eq!(
                BeaconState::generalized_index(&["slot".into(),])? as u64,
                presets::mainnet::beacon_state::post_electra::slot()
            );
            Ok(())
        }

        #[test]
        fn validator_count() -> anyhow::Result<()> {
            assert_eq!(
                BeaconState::generalized_index(&["validators".into(), PathElement::Length,])?
                    as u64,
                presets::mainnet::beacon_state::post_electra::validator_count()
            );
            Ok(())
        }

        #[test]
        fn state_roots() -> anyhow::Result<()> {
            for index in 0_usize..presets::mainnet::beacon_state::SLOTS_PER_HISTORICAL_ROOT as usize
            {
                assert_eq!(
                    BeaconState::generalized_index(&["state_roots".into(), index.into(),])? as u64,
                    presets::mainnet::beacon_state::post_electra::state_roots(index as u64)
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
                    BeaconState::generalized_index(&[
                        "historical_summaries".into(),
                        (index as usize).into(),
                    ])? as u64,
                    presets::mainnet::beacon_state::post_electra::historical_summaries(slot as u64)
                );
            }
            Ok(())
        }

        #[test]
        fn validator_balance() -> anyhow::Result<()> {
            for index in 0_usize..1_000_000 {
                assert_eq!(
                    BeaconState::generalized_index(&["balances".into(), index.into(),])? as u64,
                    presets::mainnet::beacon_state::post_electra::validator_balance(index as u64)
                );
            }
            Ok(())
        }

        #[test]
        fn validator_withdrawal_credential() -> anyhow::Result<()> {
            for index in 0_usize..1_000_000 {
                assert_eq!(
                    BeaconState::generalized_index(&[
                        "validators".into(),
                        index.into(),
                        "withdrawal_credentials".into(),
                    ])? as u64,
                    presets::mainnet::beacon_state::post_electra::validator_withdrawal_credentials(
                        index as u64
                    )
                );
            }
            Ok(())
        }

        #[test]
        fn validator_exit_epoch() -> anyhow::Result<()> {
            for index in 0_usize..1_000_000 {
                assert_eq!(
                    BeaconState::generalized_index(&[
                        "validators".into(),
                        index.into(),
                        "exit_epoch".into(),
                    ])? as u64,
                    presets::mainnet::beacon_state::post_electra::validator_exit_epoch(
                        index as u64
                    )
                );
            }
            Ok(())
        }
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
