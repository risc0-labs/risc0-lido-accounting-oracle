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
    use alloy_primitives::utils::parse_ether;
    use ethereum_consensus::phase0::presets::mainnet::BeaconBlockHeader;
    use ethereum_consensus::ssz::prelude::*;
    use gindices::presets::mainnet::beacon_state::CAPELLA_FORK_SLOT;
    use guest_io::{
        balance_and_exits, validator_membership, ANVIL_CHAIN_SPEC, WITHDRAWAL_VAULT_ADDRESS,
    };
    use risc0_steel::{ethereum::EthEvmEnv, Account};
    use risc0_zkvm::{default_executor, default_prover, ExecutorEnv};
    use test_utils::TestStateBuilder;

    use alloy::providers::{ext::AnvilApi, Provider, ProviderBuilder};

    /// Returns an Anvil provider the WITHDRAWAL_VAULT_ADDRESS balance set to 33 ether
    async fn test_provider() -> impl Provider + Clone {
        let provider = ProviderBuilder::new()
            .on_anvil_with_wallet_and_config(|anvil| anvil.args(["--hardfork", "cancun"]))
            .unwrap();
        let node_info = provider.anvil_node_info().await.unwrap();
        println!("Anvil started: {:?}", node_info);
        provider
            .anvil_set_balance(WITHDRAWAL_VAULT_ADDRESS, parse_ether("33").unwrap())
            .await
            .unwrap();
        // mine a block so the new balance is in the state
        provider.anvil_mine(Some(1), Some(1)).await.unwrap();
        provider
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_balance_and_exits() -> anyhow::Result<()> {
        let n_validators = 10;
        let n_lido_validators = 1;
        let max_validator_index = n_validators + n_lido_validators - 1;

        let mut b = TestStateBuilder::new(CAPELLA_FORK_SLOT);
        b.with_validators(n_validators);
        b.with_lido_validators(n_lido_validators);
        let s = b.build();

        let mut block_header = BeaconBlockHeader::default();
        block_header.slot = s.slot();
        block_header.state_root = s.hash_tree_root().unwrap();

        // build a membership proof
        let input = validator_membership::Input::build_initial(
            s.clone(),
            max_validator_index as u64,
            membership_builder::VALIDATOR_MEMBERSHIP_ID,
        )?
        .without_receipt();
        let env = ExecutorEnv::builder()
            .write_frame(&bincode::serialize(&input).unwrap())
            .build()?;
        let membership_proof = tokio::task::block_in_place(|| {
            default_prover().prove(env, membership_builder::VALIDATOR_MEMBERSHIP_ELF)
        })?;

        // build the Steel input for reading the balance
        let provider = test_provider().await;
        let mut env = EthEvmEnv::builder()
            .provider(provider.clone())
            .build()
            .await
            .unwrap()
            .with_chain_spec(&ANVIL_CHAIN_SPEC);
        let preflight_info = {
            let account = Account::preflight(WITHDRAWAL_VAULT_ADDRESS, &mut env);
            account.bytecode(true).info().await.unwrap()
        };
        assert_eq!(preflight_info.balance, parse_ether("33").unwrap());

        // Sanity check converting it back to an env as in the guest gives the same account info
        let input = env.into_input().await.unwrap();
        let env = input.clone().into_env().with_chain_spec(&ANVIL_CHAIN_SPEC);
        let info = {
            let account = Account::new(WITHDRAWAL_VAULT_ADDRESS, &env);
            account.bytecode(true).info()
        };
        assert_eq!(info, preflight_info, "mismatch in preflight and execution");

        let zkvm_input = balance_and_exits::Input::build(&block_header, &s.clone(), input)?
            .with_receipt(membership_proof.receipt);
        let env = ExecutorEnv::builder()
            .write_frame(&bincode::serialize(&zkvm_input).unwrap())
            .build()?;

        println!("Starting execution of the program");
        let session_info = default_executor().execute(env, super::BALANCE_AND_EXITS_ELF)?;
        println!("program execution returned: {:?}", session_info.journal);
        println!("total cycles: {}", session_info.cycles());
        Ok(())
    }
}
