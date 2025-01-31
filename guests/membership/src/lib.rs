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
    use gindices::presets::mainnet::beacon_state::{CAPELLA_FORK_SLOT, SLOTS_PER_HISTORICAL_ROOT};
    use guest_io::validator_membership;
    use risc0_zkvm::{default_executor, ExecutorEnv, ExitCode};
    use test_utils::TestStateBuilder;

    #[test]
    fn test_initial_proof() -> anyhow::Result<()> {
        let n_validators = 1001;
        let n_lido_validators = 10;
        let max_validator_index = n_validators + n_lido_validators - 1;

        let mut b = TestStateBuilder::new(CAPELLA_FORK_SLOT);
        b.with_validators(n_validators);
        b.with_lido_validators(n_lido_validators);
        let s = b.build();

        let input = validator_membership::Input::build_initial(
            &s,
            max_validator_index as u64,
            super::VALIDATOR_MEMBERSHIP_ID,
        )?;
        let input_bytes = bincode::serialize(&input).unwrap();
        println!("input length: {}", input_bytes.len());
        let env = ExecutorEnv::builder().write_frame(&input_bytes).build()?;

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

        let mut b = TestStateBuilder::new(CAPELLA_FORK_SLOT);
        b.with_validators(n_validators);
        let s1 = b.build();

        let input =
            validator_membership::Input::build_initial(&s1, 500, super::VALIDATOR_MEMBERSHIP_ID)?;
        let env = ExecutorEnv::builder()
            .write_frame(&bincode::serialize(&input).unwrap())
            .build()?;
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
            .write_frame(&bincode::serialize(&input).unwrap())
            .build()?;
        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;

        assert_eq!(session_info.exit_code, ExitCode::Halted(0));

        Ok(())
    }

    #[test]
    fn test_continuation_short_range() -> anyhow::Result<()> {
        let n_validators = 1001;
        let max_validator_index = n_validators - 1;

        let mut b = TestStateBuilder::new(CAPELLA_FORK_SLOT);
        b.with_validators(n_validators);
        let s1 = b.build();

        let mut b = TestStateBuilder::new(CAPELLA_FORK_SLOT + 20);
        b.with_validators(n_validators + 10);
        b.with_prior_state(&s1);
        let s2 = b.build();

        let input =
            validator_membership::Input::build_initial(&s1, 500, super::VALIDATOR_MEMBERSHIP_ID)?;
        let env = ExecutorEnv::builder()
            .write_frame(&bincode::serialize(&input).unwrap())
            .build()?;
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
            .write_frame(&bincode::serialize(&input).unwrap())
            .build()?;

        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;
        assert_eq!(session_info.exit_code, ExitCode::Halted(0));
        Ok(())
    }

    #[test]
    fn test_continuation_long_range() -> anyhow::Result<()> {
        let n_validators = 1001;
        let max_validator_index = n_validators - 1;

        let mut b = TestStateBuilder::new(CAPELLA_FORK_SLOT);
        b.with_validators(n_validators);
        let s1 = b.build();

        let mut b = TestStateBuilder::new(CAPELLA_FORK_SLOT + SLOTS_PER_HISTORICAL_ROOT + 1);
        b.with_validators(n_validators + 10);
        let hist_batch = b.with_prior_state(&s1);
        let s2 = b.build();

        let input =
            validator_membership::Input::build_initial(&s1, 500, super::VALIDATOR_MEMBERSHIP_ID)?;
        let env = ExecutorEnv::builder()
            .write_frame(&bincode::serialize(&input).unwrap())
            .build()?;
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
            .write_frame(&bincode::serialize(&input).unwrap())
            .build()?;

        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;
        assert_eq!(session_info.exit_code, ExitCode::Halted(0));
        Ok(())
    }
}
