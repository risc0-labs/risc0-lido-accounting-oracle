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

mod beacon_client;

use anyhow::Result;
use balance_and_exits_builder::BALANCE_AND_EXITS_ELF;
use beacon_client::BeaconClient;
use clap::Parser;
use membership_builder::{VALIDATOR_MEMBERSHIP_ELF, VALIDATOR_MEMBERSHIP_ID};
use risc0_zkvm::{
    default_prover,
    guest::env,
    serde::{from_slice, to_vec},
    ExecutorEnv, ProverOpts, Receipt, VerifierContext,
};
use std::{io::Write, path::PathBuf};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use url::Url;

/// CLI for generating and submitting Lido oracle proofs
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Ethereum beacon node HTTP RPC endpoint.
    #[clap(long, env)]
    beacon_rpc_url: Url,

    /// slot at which to base the proofs
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
    Membership {
        /// The top validator index the membership proof will be extended to.
        /// If not included it will proceed up to the total number of validators
        /// in the beacon state at the given slot
        #[clap(long)]
        max_validator_index: Option<u64>,

        #[clap(long = "out", short)]
        out_path: Option<PathBuf>,

        #[clap(subcommand)]
        command: MembershipCommand,
    },
    /// Produce the final oracle proof to go on-chain
    Aggregate,
}

/// Membership specific subcommands of the publisher CLI.
#[derive(Parser, Debug)]
enum MembershipCommand {
    /// Generate a new membership proof from scratch
    Initialize,
    /// Update an existing membership proof
    Update { in_path: PathBuf },
}

#[tokio::main]
async fn main() -> Result<()> {
    let indicatif_layer = IndicatifLayer::new();
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .init();

    let args = Args::parse();

    match args.command {
        Command::Membership {
            max_validator_index,
            out_path,
            command: MembershipCommand::Initialize,
        } => {
            build_membership_proof(
                args.beacon_rpc_url,
                args.slot,
                max_validator_index,
                None,
                out_path,
            )
            .await?
        }
        Command::Membership {
            max_validator_index,
            out_path,
            command: MembershipCommand::Update { in_path },
        } => {
            build_membership_proof(
                args.beacon_rpc_url,
                args.slot,
                max_validator_index,
                Some(in_path),
                out_path,
            )
            .await?
        }
        Command::Aggregate => build_aggregate_proof(args).await?,
    }

    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct MembershipProof {
    slot: u64,
    max_validator_index: u64,
    receipt: Receipt,
}

impl MembershipProof {
    pub fn new(slot: u64, max_validator_index: u64, receipt: Receipt) -> Self {
        Self {
            slot,
            max_validator_index,
            receipt,
        }
    }
}

#[tracing::instrument(skip(beacon_rpc_url))]
async fn build_membership_proof(
    beacon_rpc_url: Url,
    slot: u64,
    max_validator_index: Option<u64>,
    in_path: Option<PathBuf>,
    out_path: Option<PathBuf>,
) -> Result<()> {
    use guest_io::validator_membership::{Input, Journal};

    let beacon_client = BeaconClient::new_with_cache(beacon_rpc_url, "./beacon-cache")?;
    let beacon_state = beacon_client.get_beacon_state(slot).await?;

    tracing::info!("Total validators: {}", beacon_state.validators().len());

    let max_validator_index = max_validator_index.unwrap_or(beacon_state.validators().len() as u64);

    let mut env_builder = ExecutorEnv::builder();

    let env = if let Some(in_path) = in_path {
        tracing::info!("Reading input data from file: {:?}", in_path);
        let input_data = std::fs::read(in_path)?;
        let prior_proof: MembershipProof = bincode::deserialize(&input_data)?;

        let prior_beacon_state = beacon_client.get_beacon_state(prior_proof.slot).await?;
        let input = Input::build_continuation(
            &prior_beacon_state,
            prior_proof.max_validator_index,
            &beacon_state,
            max_validator_index,
            VALIDATOR_MEMBERSHIP_ID,
        )?;
        env_builder
            .add_assumption(prior_proof.receipt)
            .write(&input)?
            .build()?
    } else {
        let input =
            Input::build_initial(&beacon_state, max_validator_index, VALIDATOR_MEMBERSHIP_ID)?;
        env_builder.write(&input)?.build()?
    };

    let session_info = default_prover().prove(env, VALIDATOR_MEMBERSHIP_ELF)?;
    tracing::debug!(
        "program execution returned: {:?}",
        session_info.receipt.journal.decode::<Journal>()?
    );
    tracing::info!("total cycles: {}", session_info.stats.total_cycles);

    let proof = MembershipProof::new(slot, max_validator_index, session_info.receipt);
    let serialized_proof = bincode::serialize(&proof)?;

    if let Some(out_path) = out_path {
        std::fs::write(out_path, &serialized_proof)?;
    } else {
        std::io::stdout().write(&serialized_proof)?;
    }

    Ok(())
}

#[tracing::instrument(skip(args))]
async fn build_aggregate_proof(args: Args) -> Result<()> {
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

        let beacon_state = beacon_client.get_beacon_state(args.slot).await?;
        let input = Input::build(&beacon_block_header.message, &beacon_state)?;

        // serialize input and write it to file
        let serialized_input = to_vec(&input)?;
        let mut file = File::create(format!("input_data_slot_{}.bin", args.slot))?;
        file.write_all(&bytemuck::cast_slice(&serialized_input))?;
        input
    };

    let env = ExecutorEnv::builder().write(&input)?.build()?;

    let session_info = default_prover().prove(env, BALANCE_AND_EXITS_ELF)?;
    tracing::debug!(
        "program execution returned: {:?}",
        session_info.receipt.journal.decode::<Journal>()?
    );
    tracing::info!("total cycles: {}", session_info.stats.total_cycles);

    Ok(())
}
