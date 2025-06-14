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

mod beacon_client;

use alloy::{
    dyn_abi::SolType, network::EthereumWallet, primitives::Address, providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
};
use anyhow::{Context, Result};
use balance_and_exits_builder::{BALANCE_AND_EXITS_ELF, BALANCE_AND_EXITS_ID};
use beacon_client::BeaconClient;
use clap::Parser;
use ethereum_consensus::phase0::mainnet::{HistoricalBatch, SLOTS_PER_HISTORICAL_ROOT};
use guest_io::{ETH_SEPOLIA_CHAIN_SPEC, WITHDRAWAL_CREDENTIALS, WITHDRAWAL_VAULT_ADDRESS};
use membership_builder::{VALIDATOR_MEMBERSHIP_ELF, VALIDATOR_MEMBERSHIP_ID};
use risc0_ethereum_contracts::encode_seal;
use risc0_steel::{ethereum::EthEvmEnv, Account};
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, Receipt, VerifierContext};
use std::{
    fs::{read, write},
    path::PathBuf,
};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use url::Url;

alloy::sol!(
    struct Report {
        uint256 clBalanceGwei;
        uint256 withdrawalVaultBalanceWei;
        uint256 totalDepositedValidators;
        uint256 totalExitedValidators;
    }

    struct Commitment {
        uint256 id;
        bytes32 digest;
        bytes32 configID;
    }

    /// @title Receiver of oracle reports and proof data
    #[sol(rpc, all_derives)]
    interface IOracleProofReceiver {
        function update(uint256 refSlot, Report calldata r, bytes calldata seal, Commitment calldata commitment) external;
    }
);

alloy::sol!(
    #[sol(rpc, all_derives)]
    "../contracts/src/ITestVerifier.sol"
);

/// CLI for generating and submitting Lido oracle proofs
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// slot at which to base the proofs
    #[clap(long)]
    slot: u64,

    /// The top validator index proofs  will be extended to.
    /// If not included it will proceed up to the total number of validators
    /// in the beacon state at the given slot.
    /// This does nothing for aggregation proofs which must be run for all validators.
    #[clap(long)]
    max_validator_index: Option<u64>,

    #[clap(subcommand)]
    command: Command,
}

/// Subcommands of the publisher CLI.
#[derive(Parser, Debug)]
enum Command {
    /// Generate a proof from a given input
    Prove {
        /// Ethereum beacon node HTTP RPC endpoint.
        #[clap(long, env)]
        beacon_rpc_url: Url,

        #[clap(long = "out", short)]
        out_path: PathBuf,

        #[clap(subcommand)]
        command: ProveCommand,
    },
    /// Submit an aggregation proof to the oracle contract
    Submit {
        /// Eth key to sign with
        #[clap(long, env)]
        eth_wallet_private_key: PrivateKeySigner,

        /// Ethereum Node endpoint.
        #[clap(long, env)]
        eth_rpc_url: Url,

        /// SecondOpinionOracle contract address
        #[clap(long, env)]
        contract: Option<Address>,

        /// TestVerifier contract address
        #[clap(long, env)]
        test_contract: Option<Address>,

        #[clap(long = "proof", short)]
        proof_path: PathBuf,
    },
}

#[derive(Parser, Debug)]
enum ProveCommand {
    /// An initial membership proof
    Initial,
    /// A continuation from a prior membership proof
    ContinuationFrom { prior_path: PathBuf },
    /// An aggregation (oracle) proof that can be submitted on-chain
    Aggregation {
        membership_proof_path: PathBuf,

        // Ethereum execution node HTTP RPC endpoint.
        #[clap(long, env)]
        eth_rpc_url: Url,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    match args.command {
        Command::Prove {
            out_path,
            command: ProveCommand::Initial,
            beacon_rpc_url,
        } => {
            let input =
                build_membership_input(beacon_rpc_url, args.slot, args.max_validator_index, None)
                    .await?;
            let proof =
                build_membership_proof(input, None, args.slot, args.max_validator_index).await?;
            write(out_path, &bincode::serialize(&proof)?)?;
        }
        Command::Prove {
            out_path,
            command: ProveCommand::ContinuationFrom { prior_path },
            beacon_rpc_url,
        } => {
            let prior_proof: MembershipProof = bincode::deserialize(&read(prior_path)?)?;
            let input = build_membership_input(
                beacon_rpc_url,
                args.slot,
                args.max_validator_index,
                Some(prior_proof.slot),
            )
            .await?;
            let proof = build_membership_proof(
                input,
                Some(prior_proof),
                args.slot,
                args.max_validator_index,
            )
            .await?;
            write(out_path, &bincode::serialize(&proof)?)?;
        }
        Command::Prove {
            out_path,
            command:
                ProveCommand::Aggregation {
                    membership_proof_path,
                    eth_rpc_url,
                },
            beacon_rpc_url,
        } => {
            let input = build_aggregate_input(beacon_rpc_url, args.slot, eth_rpc_url).await?;
            let membership_proof: MembershipProof =
                bincode::deserialize(&read(membership_proof_path)?)?;
            let proof = build_aggregate_proof(input, membership_proof, args.slot).await?;
            write(out_path, &bincode::serialize(&proof)?)?;
        }
        Command::Submit {
            eth_wallet_private_key,
            eth_rpc_url,
            contract,
            test_contract,
            proof_path,
        } => {
            submit_aggregate_proof(
                eth_wallet_private_key,
                eth_rpc_url,
                contract,
                test_contract,
                proof_path,
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
async fn build_membership_input<'a>(
    beacon_rpc_url: Url,
    slot: u64,
    max_validator_index: Option<u64>,
    prior_slot: Option<u64>,
) -> Result<guest_io::validator_membership::Input<'a>> {
    use guest_io::validator_membership::Input;

    let beacon_client = BeaconClient::new_with_cache(beacon_rpc_url, "./beacon-cache")?;

    tracing::info!("Retrieving beacon state...");
    let beacon_state = beacon_client.get_beacon_state(slot).await?;

    tracing::info!("Total validators: {}", beacon_state.validators().len());
    tracing::info!(
        "Lido validators: {}",
        beacon_state
            .validators()
            .iter()
            .filter(
                |validator| validator.withdrawal_credentials.as_slice() == WITHDRAWAL_CREDENTIALS
            )
            .count()
    );

    let max_validator_index =
        max_validator_index.unwrap_or((beacon_state.validators().len() - 1) as u64);

    let input = if let Some(prior_slot) = prior_slot {
        let hist_summary = if beacon_state.slot() > prior_slot + (SLOTS_PER_HISTORICAL_ROOT as u64)
        {
            // this is a long range continuation and we need to provide an intermediate historical summary
            tracing::info!("Long range continuation detected");
            let inter_slot = (prior_slot / (SLOTS_PER_HISTORICAL_ROOT as u64) + 1)
                * (SLOTS_PER_HISTORICAL_ROOT as u64);
            tracing::info!("Fetching intermediate state at slot: {}", inter_slot);
            let inter_state = beacon_client.get_beacon_state(inter_slot).await?;
            Some(HistoricalBatch {
                block_roots: inter_state.block_roots().clone(),
                state_roots: inter_state.state_roots().clone(),
            })
        } else {
            None
        };

        tracing::info!("Retrieving intermediate beacon state...");
        let prior_beacon_state = beacon_client.get_beacon_state(prior_slot).await?;
        let prior_max_validator_index = (prior_beacon_state.validators().len() - 1) as u64;

        tracing::info!("Building input. This may take a few minutes...");
        Input::build_continuation(
            &prior_beacon_state,
            prior_max_validator_index,
            &beacon_state,
            max_validator_index,
            hist_summary,
            VALIDATOR_MEMBERSHIP_ID,
        )?
    } else {
        tracing::info!("Building input. This may take a few minutes...");

        Input::build_initial(beacon_state, max_validator_index, VALIDATOR_MEMBERSHIP_ID)?
    };
    Ok(input)
}

#[tracing::instrument(skip(input, prior_proof))]
async fn build_membership_proof<'a>(
    input: guest_io::validator_membership::Input<'a>,
    prior_proof: Option<MembershipProof>,
    slot: u64,
    max_validator_index: Option<u64>,
) -> Result<MembershipProof> {
    let mut env_builder = ExecutorEnv::builder();

    let input = if let Some(prior_proof) = prior_proof {
        input.with_receipt(prior_proof.receipt)
    } else {
        input.without_receipt()
    };

    let env = env_builder
        .write_frame(&bincode::serialize(&input)?)
        .build()?;

    tracing::info!("Generating membership proof...");
    let session_info = default_prover().prove_with_ctx(
        env,
        &VerifierContext::default(),
        VALIDATOR_MEMBERSHIP_ELF,
        &ProverOpts::succinct(),
    )?;
    tracing::info!("total cycles: {}", session_info.stats.total_cycles);

    let proof = MembershipProof::new(slot, input.input.max_validator_index, session_info.receipt);

    Ok(proof)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AggregateProof {
    slot: u64,
    receipt: Receipt,
}

#[tracing::instrument(skip(beacon_rpc_url, eth_rpc_url))]
async fn build_aggregate_input<'a>(
    beacon_rpc_url: Url,
    slot: u64,
    eth_rpc_url: Url,
) -> Result<guest_io::balance_and_exits::Input<'a>> {
    let beacon_client = BeaconClient::new_with_cache(beacon_rpc_url.clone(), "./beacon-cache")?;
    let beacon_block_header = beacon_client.get_block_header(slot).await?;

    let beacon_state = beacon_client.get_beacon_state(slot).await?;

    let block_hash = beacon_client.get_eth1_block_hash_at_slot(slot).await?;

    let mut env = EthEvmEnv::builder()
        .chain_spec(&ETH_SEPOLIA_CHAIN_SPEC)
        .rpc(eth_rpc_url)
        .beacon_api(beacon_rpc_url)
        .block_hash(block_hash)
        .build()
        .await?;

    let _preflight_info = {
        let account = Account::preflight(WITHDRAWAL_VAULT_ADDRESS, &mut env);
        account.bytecode(true).info().await.unwrap()
    };

    let evm_input = env.into_input().await?;

    let input = guest_io::balance_and_exits::Input::build(
        &beacon_block_header.message,
        &beacon_state,
        evm_input,
    )?;

    Ok(input)
}

#[tracing::instrument(skip(input, membership_proof))]
async fn build_aggregate_proof<'a>(
    input: guest_io::balance_and_exits::Input<'a>,
    membership_proof: MembershipProof,
    slot: u64,
) -> Result<AggregateProof> {
    let input = input.with_receipt(membership_proof.receipt);

    let env = ExecutorEnv::builder()
        .write_frame(&bincode::serialize(&input)?)
        .build()?;

    tracing::info!("Generating aggregate proof...");
    let session_info = default_prover().prove_with_ctx(
        env,
        &VerifierContext::default(),
        BALANCE_AND_EXITS_ELF,
        &ProverOpts::groth16(),
    )?;
    tracing::info!("total cycles: {}", session_info.stats.total_cycles);

    Ok(AggregateProof {
        slot,
        receipt: session_info.receipt,
    })
}

async fn submit_aggregate_proof(
    eth_wallet_private_key: PrivateKeySigner,
    eth_rpc_url: Url,
    contract: Option<Address>,
    test_contract: Option<Address>,
    in_path: PathBuf,
) -> Result<()> {
    let wallet = EthereumWallet::from(eth_wallet_private_key);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(eth_rpc_url);

    let proof: AggregateProof = bincode::deserialize(&read(in_path)?)?;
    tracing::info!("verifying locally for sanity check");
    proof.receipt.verify(BALANCE_AND_EXITS_ID)?;
    tracing::info!("Local verification passed :)");

    let seal = encode_seal(&proof.receipt).context("encoding seal")?;

    if let Some(test_contract) = test_contract {
        let contract = ITestVerifier::new(test_contract, provider.clone());
        let block_root = proof.receipt.journal.bytes[..32].try_into()?;
        let report = TestReport::abi_decode(&proof.receipt.journal.bytes[32..])?;
        let call_builder = contract.verify(block_root, report, seal.clone().into());
        let pending_tx = call_builder.send().await?;
        tracing::info!(
            "test_verifier: Submitted proof with tx hash: {}",
            pending_tx.tx_hash()
        );
        let tx_receipt = pending_tx.get_receipt().await?;
        tracing::info!("Test_verifier: Tx included with receipt {:?}", tx_receipt);
    }

    if let Some(contract) = contract {
        let contract = IOracleProofReceiver::new(contract, provider.clone());
        // skip the first 32 bytes of the journal as that is the beacon block hash which is not part of the report
        let report = Report::abi_decode(&proof.receipt.journal.bytes[32..])?;
        let commitment = Commitment::abi_decode(&proof.receipt.journal.bytes[32 + 32..])?;
        let call_builder = contract.update(
            proof.slot.try_into()?,
            report,
            seal.clone().into(),
            commitment,
        );
        let pending_tx = call_builder.send().await?;
        tracing::info!("Submitted proof with tx hash: {}", pending_tx.tx_hash());
        let tx_receipt = pending_tx.get_receipt().await?;
        tracing::info!("Tx included with receipt {:?}", tx_receipt);
    }

    if let (None, None) = (contract, test_contract) {
        eprintln!("No contract address provided, skipping submission");
    }

    Ok(())
}
