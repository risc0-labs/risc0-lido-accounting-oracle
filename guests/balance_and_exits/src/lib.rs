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
    use ethereum_consensus::deneb::Validator;
    use ethereum_consensus::phase0::presets::mainnet::{BeaconBlockHeader, BeaconState};
    use ethereum_consensus::ssz::prelude::*;
    use guest_io::WITHDRAWAL_CREDENTIALS;
    use guest_io::{balance_and_exits, validator_membership};
    use risc0_zkvm::{default_executor, default_prover, ExecutorEnv};

    #[test]
    fn test_balance_and_exits() -> anyhow::Result<()> {
        let n_empty_validators = 100;
        let n_lido_validators = 10;

        let mut block_header = BeaconBlockHeader::default();
        let mut beacon_state = BeaconState::default();

        for _ in 0..n_empty_validators {
            beacon_state.validators.push(Default::default());
            beacon_state.balances.push(99);
        }
        for _ in 0..n_lido_validators {
            beacon_state.validators.push(Validator {
                withdrawal_credentials: WITHDRAWAL_CREDENTIALS.as_slice().try_into().unwrap(),
                ..Default::default()
            });
            beacon_state.balances.push(10);
        }
        block_header.state_root = beacon_state.hash_tree_root()?.into();

        // build a membership proof
        let input = validator_membership::Input::build_initial(
            &ethereum_consensus::types::mainnet::BeaconState::Phase0(beacon_state.clone()),
            (beacon_state.validators.len() - 1) as u64,
            membership_builder::VALIDATOR_MEMBERSHIP_ID,
        )?;
        let env = ExecutorEnv::builder().write(&input)?.build()?;
        let membership_proof =
            default_prover().prove(env, membership_builder::VALIDATOR_MEMBERSHIP_ELF)?;

        let input = balance_and_exits::Input::build(
            &block_header,
            &ethereum_consensus::types::mainnet::BeaconState::Phase0(beacon_state.clone()),
        )
        .unwrap();

        let env = ExecutorEnv::builder()
            .add_assumption(membership_proof.receipt)
            .write(&input)?
            .build()?;

        println!("Starting execution of the program");
        let session_info = default_executor().execute(env, super::BALANCE_AND_EXITS_ELF)?;
        println!(
            "program execution returned: {:?}",
            session_info
                .journal
                .decode::<balance_and_exits::Journal>()?
        );
        println!("total cycles: {}", session_info.cycles());
        Ok(())
    }
}
