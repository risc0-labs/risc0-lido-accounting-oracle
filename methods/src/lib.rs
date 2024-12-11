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
    use alloy_primitives::B256;
    use ethereum_consensus::phase0::presets::mainnet::BeaconState;
    use ethereum_consensus::ssz::prelude::*;
    use lido_oracle_core::{
        gindices::presets::mainnet::{state_roots_gindex, validator_withdrawal_credentials_gindex},
        Input, MultiproofBuilder, ProofType,
    };
    use risc0_zkvm::{default_executor, ExecutorEnv};

    #[test]
    fn test_initial_proof() -> anyhow::Result<()> {
        let prior_up_to_validator_index = 0;
        let up_to_validator_index = 1000;

        let mut beacon_state = BeaconState::default();

        // add empty validators to the state
        for _ in prior_up_to_validator_index..up_to_validator_index {
            beacon_state.validators.push(Default::default());
        }

        println!("Starting building multiproof");

        let multiproof = MultiproofBuilder::new()
            .with_gindex(state_roots_gindex(0).try_into()?)
            .with_gindices(
                (prior_up_to_validator_index..up_to_validator_index).map(|i| {
                    validator_withdrawal_credentials_gindex(i)
                        .try_into()
                        .unwrap()
                }),
            )
            .build(&beacon_state)
            .unwrap();

        println!("end building multiproof");

        let input = Input {
            self_program_id: crate::VALIDATOR_MEMBERSHIP_ID.into(),
            proof_type: ProofType::Initial,
            withdrawal_credentials: B256::ZERO,
            current_state_root: beacon_state.hash_tree_root().unwrap().into(),
            up_to_validator_index,
            multiproof,
        };

        let env = ExecutorEnv::builder().write(&input)?.build()?;

        println!("Starting execution of the program");
        // NOTE: Use the executor to run tests without proving.
        let session_info = default_executor().execute(env, super::VALIDATOR_MEMBERSHIP_ELF)?;
        println!("total cycles: {:?}", session_info.cycles());
        Ok(())
    }
}
