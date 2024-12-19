// Copyright 2024 RISC Zero, Inc.
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

// This application demonstrates how to send an off-chain proof request
// to the Bonsai proving service and publish the received proofs directly
// to your deployed app contract.

use alloy_primitives::{Address, U256};
use anyhow::{Context, Result};
use apps::beacon_client::BeaconClient;
use clap::Parser;
use ethereum_consensus::types::mainnet::BeaconState;
use methods::VALIDATOR_MEMBERSHIP_ELF;
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_executor, serde::to_vec, ExecutorEnv, ProverOpts, VerifierContext};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use url::Url;

use lido_oracle_core::io::validator_membership::{Input, Journal};

/// Arguments of the publisher CLI.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Ethereum beacon node HTTP RPC endpoint.
    #[clap(long)]
    beacon_rpc_url: Url,

    /// slot at which to generate an oracle proof for
    #[clap(long)]
    slot: u64,

    #[clap(long)]
    max_validator_index: u64,

    /// The slot used previously if this is a continuation
    /// proof, otherwise None of this is the first proof
    #[clap(long)]
    prior_slot: Option<u64>,

    /// The validator index used previously if this is a continuation
    #[clap(long)]
    prior_max_validator_index: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    let beacon_client = BeaconClient::new_with_cache(args.beacon_rpc_url, "./beacon-cache")?;
    let beacon_state = beacon_client.get_state(args.slot).await?;

    let input = if let (Some(prior_slot), Some(prior_max_validator_index)) =
        (args.prior_slot, args.prior_max_validator_index)
    {
        tracing::info!("Building input for continuation proof");

        let prior_beacon_state = beacon_client.get_state(prior_slot).await?;
        Input::build_continuation(
            prior_beacon_state,
            prior_max_validator_index,
            beacon_state,
            args.max_validator_index,
        )?
    } else {
        tracing::info!("Building input for initial proof");
        Input::build_initial(beacon_state, args.max_validator_index)?
    };

    tracing::debug!("Input: {:?}", input);
    tracing::debug!("input size (bytes): {}", to_vec(&input)?.len() * 4);

    let env = ExecutorEnv::builder().write(&input)?.build()?;

    tracing::info!("Starting execution of the program");
    let session_info = default_executor().execute(env, VALIDATOR_MEMBERSHIP_ELF)?;
    tracing::debug!(
        "program execution returned: {:?}",
        session_info.journal.decode::<Journal>()?
    );
    tracing::info!("total cycles: {}", session_info.cycles());

    tracing::info!("Complete");

    Ok(())
}
