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
use ethereum_consensus::phase0::mainnet::{HistoricalBatch, SLOTS_PER_HISTORICAL_ROOT};
use membership_builder::{VALIDATOR_MEMBERSHIP_ELF, VALIDATOR_MEMBERSHIP_ID};
use risc0_zkvm::{
    default_prover,
    serde::{from_slice, to_vec},
    ExecutorEnv, Receipt,
};
use std::{
    fs::{read, File},
    io::Write,
    path::PathBuf,
};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
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
        out_path: PathBuf,

        #[clap(subcommand)]
        command: MembershipCommand,
    },
    /// Produce the final oracle proof to go on-chain
    Aggregate {
        #[clap(long = "out", short)]
        out_path: PathBuf,

        membership_proof_path: PathBuf,
    },
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
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
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
        Command::Aggregate {
            out_path,
            membership_proof_path,
        } => {
            build_aggregate_proof(
                args.beacon_rpc_url,
                args.slot,
                membership_proof_path,
                args.input_data,
                out_path,
            )
            .await?
        }
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
    out_path: PathBuf,
) -> Result<()> {
    use guest_io::validator_membership::{Input, Journal};

    let beacon_client = BeaconClient::new_with_cache(beacon_rpc_url, "./beacon-cache")?;
    let beacon_state = beacon_client.get_beacon_state(slot).await?;

    tracing::info!("Total validators: {}", beacon_state.validators().len());

    let max_validator_index =
        max_validator_index.unwrap_or((beacon_state.validators().len() - 1) as u64);

    let mut env_builder = ExecutorEnv::builder();

    let env = if let Some(in_path) = in_path {
        tracing::info!("Reading prior proof from file: {:?}", in_path);
        let prior_proof: MembershipProof = bincode::deserialize(&read(in_path)?)?;

        let hist_summary =
            if beacon_state.slot() > prior_proof.slot + (SLOTS_PER_HISTORICAL_ROOT as u64) {
                // this is a long range continuation and we need to provide an intermediate historical summary
                tracing::info!("Long range continuation detected");
                let inter_slot = prior_proof.slot / (SLOTS_PER_HISTORICAL_ROOT as u64)
                    + (SLOTS_PER_HISTORICAL_ROOT as u64);
                tracing::info!("Fetching intermediate state at slot: {}", inter_slot);
                let inter_state = beacon_client.get_beacon_state(inter_slot).await?;
                Some(HistoricalBatch {
                    block_roots: inter_state.block_roots().clone(),
                    state_roots: inter_state.state_roots().clone(),
                })
            } else {
                None
            };

        let prior_beacon_state = beacon_client.get_beacon_state(prior_proof.slot).await?;
        let input = Input::build_continuation(
            &prior_beacon_state,
            prior_proof.max_validator_index,
            &beacon_state,
            max_validator_index,
            &hist_summary,
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

    std::fs::write(out_path, &serialized_proof)?;

    Ok(())
}

#[tracing::instrument(skip(beacon_rpc_url))]
async fn build_aggregate_proof(
    beacon_rpc_url: Url,
    slot: u64,
    membership_proof_path: PathBuf,
    input_path: Option<PathBuf>,
    out_path: PathBuf,
) -> Result<()> {
    use guest_io::balance_and_exits::Input;

    let input = if let Some(input_data) = input_path {
        tracing::info!("Reading input data from file: {:?}", input_data);
        let input_data = std::fs::read(input_data)?;
        let input: Input = from_slice(&input_data)?;
        input
    } else {
        let beacon_client = BeaconClient::new_with_cache(beacon_rpc_url, "./beacon-cache")?;
        let beacon_block_header = beacon_client.get_block_header(slot).await?;

        let beacon_state = beacon_client.get_beacon_state(slot).await?;
        let input = Input::build(&beacon_block_header.message, &beacon_state)?;

        // serialize input and write it to file
        let serialized_input = to_vec(&input)?;
        let mut file = File::create(format!("input_data_slot_{}.bin", slot))?;
        file.write_all(&bytemuck::cast_slice(&serialized_input))?;
        input
    };

    let membership_proof: MembershipProof = bincode::deserialize(&read(membership_proof_path)?)?;

    let env = ExecutorEnv::builder()
        .add_assumption(membership_proof.receipt)
        .write(&input)?
        .build()?;

    let session_info = default_prover().prove(env, BALANCE_AND_EXITS_ELF)?;
    tracing::debug!(
        "program execution returned: {:?}",
        session_info.receipt.journal
    );
    tracing::info!("total cycles: {}", session_info.stats.total_cycles);

    Ok(())
}
