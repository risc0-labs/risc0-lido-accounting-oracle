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
use ethereum_consensus::{ssz::prelude::Path, types::mainnet::BeaconState};
use methods::{BALANCE_AND_EXITS_ELF, VALIDATOR_MEMBERSHIP_ELF};
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{
    default_executor,
    serde::{from_slice, to_vec},
    ExecutorEnv, ProverOpts, VerifierContext,
};
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use url::Url;

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
    input_data: Option<PathBuf>,

    #[clap(subcommand)]
    command: Command,
}

/// Subcommands of the publisher CLI.
#[derive(Parser, Debug)]
enum Command {
    /// Generate or update a membership proof
    Update {
        #[clap(long)]
        max_validator_index: u64,

        /// The slot used previously if this is a continuation
        /// proof, otherwise None if this is the first proof
        #[clap(long)]
        prior_slot: Option<u64>,

        /// The validator index used previously if this is a continuation
        #[clap(long)]
        prior_max_validator_index: Option<u64>,
    },
    Finalize,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    match args.command {
        Command::Update {
            max_validator_index,
            prior_slot,
            prior_max_validator_index,
        } => {
            membership(
                args,
                max_validator_index,
                prior_slot,
                prior_max_validator_index,
            )
            .await?
        }
        Command::Finalize => balance_and_exits(args).await?,
    }

    Ok(())
}

async fn membership(
    args: Args,
    max_validator_index: u64,
    prior_slot: Option<u64>,
    prior_max_validator_index: Option<u64>,
) -> Result<()> {
    use guest_io::validator_membership::{Input, Journal};

    let beacon_client = BeaconClient::new_with_cache(args.beacon_rpc_url, "./beacon-cache")?;

    tracing::info!(
        "Fetching (or retrieving cached) beacon state at slot {}",
        args.slot
    );
    let beacon_state = beacon_client.get_state(args.slot).await?;
    tracing::info!("Fetched beacon state");

    let input = if let (Some(prior_slot), Some(prior_max_validator_index)) =
        (prior_slot, prior_max_validator_index)
    {
        tracing::info!("Building input for continuation proof");

        let prior_beacon_state = beacon_client.get_state(prior_slot).await?;
        Input::build_continuation(
            &prior_beacon_state,
            prior_max_validator_index,
            &beacon_state,
            max_validator_index,
        )?
    } else {
        tracing::info!("Building input for initial proof");
        Input::build_initial(&beacon_state, max_validator_index)?
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

async fn balance_and_exits(args: Args) -> Result<()> {
    use guest_io::balance_and_exits::{Input, Journal};
    use std::fs::File;
    use std::io::Write;

    let input = if let Some(input_data) = args.input_data {
        tracing::info!("Reading input data from file: {:?}", input_data);
        let input_data = std::fs::read(input_data)?;
        let input: Input = from_slice(&input_data)?;
        input
    } else {
        let beacon_client = BeaconClient::new_with_cache(args.beacon_rpc_url, "./beacon-cache")?;
        let beacon_block_header = beacon_client.get_block_header(args.slot).await?;

        tracing::info!(
            "Fetching (or retrieving cached) beacon state at slot {}",
            args.slot
        );
        let beacon_state = beacon_client.get_state(args.slot).await?;

        tracing::info!("Fetched beacon state");

        tracing::info!("Building input");
        let input = Input::build(&beacon_block_header.message, &beacon_state)?;

        // serialize input and write it to file
        let serialized_input = to_vec(&input)?;
        let mut file = File::create(format!("input_data_slot_{}.bin", args.slot))?;
        file.write_all(&bytemuck::cast_slice(&serialized_input))?;
        input
    };

    let env = ExecutorEnv::builder().write(&input)?.build()?;

    tracing::info!("Starting execution of the program");
    let session_info = default_executor().execute(env, BALANCE_AND_EXITS_ELF)?;
    tracing::debug!(
        "program execution returned: {:?}",
        session_info.journal.decode::<Journal>()?
    );
    tracing::info!("total cycles: {}", session_info.cycles());

    tracing::info!("Complete");

    Ok(())
}
