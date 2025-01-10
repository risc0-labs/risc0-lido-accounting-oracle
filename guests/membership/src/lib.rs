// Copyright 2023 RISC Zero, Inc.
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

//! Generated crate containing the image ID and ELF binary of the build guest.
include!(concat!(env!("OUT_DIR"), "/methods.rs"));

#[cfg(test)]
mod tests {
    use ethereum_consensus::capella::presets::mainnet::{
        BeaconState, HistoricalBatch, HistoricalSummary, Validator,
    };
    use ethereum_consensus::ssz::prelude::*;
    use gindices::presets::mainnet::beacon_state::SLOTS_PER_HISTORICAL_ROOT;
    use guest_io::{validator_membership, WITHDRAWAL_CREDENTIALS};
    use risc0_zkvm::{default_executor, ExecutorEnv, ExitCode};

    struct TestStateBuilder {
        inner: BeaconState,
    }

    impl TestStateBuilder {
        pub fn new(slot: u64) -> Self {
            Self {
                inner: BeaconState {
                    slot,
                    ..Default::default()
                },
            }
        }

        pub fn with_validators(&mut self, n_empty_validators: usize) {
            for _ in 0..n_empty_validators {
                self.inner.validators.push(Default::default());
                self.inner.balances.push(99);
            }
        }

        pub fn with_lido_validators(&mut self, n_lido_validators: usize) {
            for _ in 0..n_lido_validators {
                self.inner.validators.push(Validator {
                    withdrawal_credentials: WITHDRAWAL_CREDENTIALS.as_slice().try_into().unwrap(),
                    ..Default::default()
                });
                self.inner.balances.push(10);
            }
        }

        pub fn with_prior_state(
            &mut self,
            prior_state: &ethereum_consensus::types::mainnet::BeaconState,
        ) -> Option<HistoricalBatch> {
            let slot = self.inner.slot;
            let prior_slot = prior_state.slot();
            assert!(slot > prior_slot, "prior_state.slot must be less than slot");
            let index: usize = (prior_slot % SLOTS_PER_HISTORICAL_ROOT).try_into().unwrap();

            // if a short range add the state root to the state_roots list
            if slot <= prior_slot + SLOTS_PER_HISTORICAL_ROOT {
                self.inner.state_roots[index] = prior_state.hash_tree_root().unwrap();
                None
            } else {
                // if a long range build a HistoricalSummary containing the state root and add this so the historical_summaries list
                let mut batch = HistoricalBatch::default();
                batch.state_roots[index] = prior_state.hash_tree_root().unwrap();
                let summary = HistoricalSummary {
                    block_summary_root: batch.block_roots.hash_tree_root().unwrap(),
                    state_summary_root: batch.state_roots.hash_tree_root().unwrap(),
                };
                self.inner.historical_summaries.extend(
                    std::iter::repeat(summary)
                        .take(((slot - prior_slot) / SLOTS_PER_HISTORICAL_ROOT) as usize + 1),
                );
                Some(batch)
            }
        }

        fn build(self) -> ethereum_consensus::types::mainnet::BeaconState {
            ethereum_consensus::types::mainnet::BeaconState::Capella(self.inner)
        }
    }

    #[test]
    fn test_initial_proof() -> anyhow::Result<()> {
        let n_validators = 1001;
        let max_validator_index = n_validators - 1;

        let mut b = TestStateBuilder::new(10);
        b.with_validators(n_validators);
        let s = b.build();

        let input = validator_membership::Input::build_initial(
            &s,
            max_validator_index as u64,
            super::VALIDATOR_MEMBERSHIP_ID,
        )?;

        let env = ExecutorEnv::builder().write(&input)?.build()?;

        println!("Starting execution of the program");
        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;
        println!(
            "program execution returned: {:?}",
            session_info
                .journal
                .decode::<validator_membership::Journal>()?
        );
        println!("total cycles: {}", session_info.cycles());
        Ok(())
    }

    #[test]
    fn test_continuation_same_slot() -> anyhow::Result<()> {
        let n_validators = 1001;
        let max_validator_index = n_validators - 1;

        let mut b = TestStateBuilder::new(10);
        b.with_validators(n_validators);
        let s1 = b.build();

        let input =
            validator_membership::Input::build_initial(&s1, 500, super::VALIDATOR_MEMBERSHIP_ID)?;
        let env = ExecutorEnv::builder().write(&input)?.build()?;
        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;

        let input = validator_membership::Input::build_continuation(
            &s1,
            500,
            &s1,
            max_validator_index as u64,
            &None,
            super::VALIDATOR_MEMBERSHIP_ID,
        )?;
        let env = ExecutorEnv::builder()
            .add_assumption(session_info.receipt_claim.unwrap())
            .write(&input)?
            .build()?;
        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;

        assert_eq!(session_info.exit_code, ExitCode::Halted(0));

        Ok(())
    }

    #[test]
    fn test_continuation_short_range() -> anyhow::Result<()> {
        let n_validators = 1001;
        let max_validator_index = n_validators - 1;

        let mut b = TestStateBuilder::new(10);
        b.with_validators(n_validators);
        let s1 = b.build();

        let mut b = TestStateBuilder::new(20);
        b.with_validators(n_validators + 10);
        b.with_prior_state(&s1);
        let s2 = b.build();

        let input =
            validator_membership::Input::build_initial(&s1, 500, super::VALIDATOR_MEMBERSHIP_ID)?;
        let env = ExecutorEnv::builder().write(&input)?.build()?;
        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;

        let input = validator_membership::Input::build_continuation(
            &s1.clone(),
            500,
            &s2,
            max_validator_index as u64,
            &None,
            super::VALIDATOR_MEMBERSHIP_ID,
        )?;
        let env = ExecutorEnv::builder()
            .add_assumption(session_info.receipt_claim.unwrap())
            .write(&input)?
            .build()?;

        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;
        assert_eq!(session_info.exit_code, ExitCode::Halted(0));
        Ok(())
    }

    #[test]
    fn test_continuation_long_range() -> anyhow::Result<()> {
        let n_validators = 1001;
        let max_validator_index = n_validators - 1;

        let mut b = TestStateBuilder::new(10);
        b.with_validators(n_validators);
        let s1 = b.build();

        let mut b = TestStateBuilder::new(20 + SLOTS_PER_HISTORICAL_ROOT + 1);
        b.with_validators(n_validators + 10);
        let hist_batch = b.with_prior_state(&s1);
        let s2 = b.build();

        let input =
            validator_membership::Input::build_initial(&s1, 500, super::VALIDATOR_MEMBERSHIP_ID)?;
        let env = ExecutorEnv::builder().write(&input)?.build()?;
        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;

        let input = validator_membership::Input::build_continuation(
            &s1,
            500,
            &s2,
            max_validator_index as u64,
            &hist_batch,
            super::VALIDATOR_MEMBERSHIP_ID,
        )?;
        let env = ExecutorEnv::builder()
            .add_assumption(session_info.receipt_claim.unwrap())
            .write(&input)?
            .build()?;

        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;
        assert_eq!(session_info.exit_code, ExitCode::Halted(0));
        Ok(())
    }
}
