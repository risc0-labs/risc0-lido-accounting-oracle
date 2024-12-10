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
    use alloy_primitives::U256;
    use bitvec::prelude::*;
    use lido_oracle_core::{Input, MultiproofBuilder};
    use risc0_zkvm::{default_executor, sha::Digest, ExecutorEnv};

    #[test]
    fn test_sending_multiproof() {
        let state = ethereum_consensus::phase0::presets::mainnet::BeaconState::default();

        let block_root_proof = MultiproofBuilder::new()
            .with_path(&["block_roots".into(), 0.into()])
            .unwrap()
            .build(&state)
            .unwrap();

        let input = Input {
            self_program_id: Digest::ZERO,
            prior_state_root: U256::ZERO,
            prior_max_validator_index: 0,
            withdrawal_credentials: U256::ZERO,
            // prior_membership: BitVec::new(),
            current_state_root: U256::ZERO,
            multiproof: block_root_proof,
        };

        let env = ExecutorEnv::builder()
            .write(&input)
            .unwrap()
            .build()
            .unwrap();

        // NOTE: Use the executor to run tests without proving.
        let session_info = default_executor()
            .execute(env, super::VALIDATOR_MEMBERSHIP_ELF)
            .unwrap();
    }
}
