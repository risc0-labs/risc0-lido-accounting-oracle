// Copyright 2025 RISC Zero, Inc.
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

use oracle_builder::MAINNET_ID;

use alloy_primitives::utils::parse_ether;
use ethereum_consensus::phase0::presets::mainnet::BeaconBlockHeader;
use ethereum_consensus::ssz::prelude::*;
use gindices::presets::mainnet::beacon_state::CAPELLA_FORK_SLOT;
use lido_oracle_core::{
    generate_oracle_report,
    input::Input,
    mainnet::{WITHDRAWAL_CREDENTIALS, WITHDRAWAL_VAULT_ADDRESS},
    ANVIL_CHAIN_SPEC,
};
use test_utils::TestStateBuilder;

use alloy::providers::{ext::AnvilApi, Provider, ProviderBuilder};

/// Returns an Anvil provider the WITHDRAWAL_VAULT_ADDRESS balance set to 33 ether
async fn test_provider() -> impl Provider + Clone {
    let provider = ProviderBuilder::new()
        .connect_anvil_with_wallet_and_config(|anvil| anvil.args(["--hardfork", "cancun"]))
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

#[tokio::test]
async fn test_simple_initial() -> anyhow::Result<()> {
    let n_validators = 10;
    let n_lido_validators = 1;

    let provider = test_provider().await;

    let mut b = TestStateBuilder::new(CAPELLA_FORK_SLOT);
    b.with_validators(n_validators);
    b.with_lido_validators(n_lido_validators);
    let s = b.build();

    let mut block_header = BeaconBlockHeader::default();
    block_header.slot = s.slot();
    block_header.state_root = s.hash_tree_root().unwrap();

    let input = Input::build_initial(
        &ANVIL_CHAIN_SPEC,
        MAINNET_ID,
        &block_header,
        &s,
        &WITHDRAWAL_CREDENTIALS,
        WITHDRAWAL_VAULT_ADDRESS,
        provider.clone(),
    )
    .await?;

    let journal = generate_oracle_report(
        &input,
        &ANVIL_CHAIN_SPEC,
        &WITHDRAWAL_CREDENTIALS,
        WITHDRAWAL_VAULT_ADDRESS,
    )?;

    assert_eq!(
        journal.withdrawalVaultBalanceWei,
        parse_ether("33").unwrap()
    );
    assert_eq!(journal.clBalanceGwei, U256::from(10 * n_lido_validators));

    Ok(())
}
