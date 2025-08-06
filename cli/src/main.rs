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
use beacon_client::BeaconClient;
use clap::Parser;
use ethereum_consensus::phase0::mainnet::{HistoricalBatch, SLOTS_PER_HISTORICAL_ROOT};
use lido_oracle_core::{
    input::Input as OracleInput,
    mainnet::{WITHDRAWAL_CREDENTIALS, WITHDRAWAL_VAULT_ADDRESS},
    ETH_SEPOLIA_CHAIN_SPEC,
};
use oracle_builder::{MAINNET_ELF as BALANCE_AND_EXITS_ELF, MAINNET_ID as BALANCE_AND_EXITS_ID};
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
    /// An aggregation (oracle) proof that can be submitted on-chain
    ContinuationFrom {
        prior_proof_path: PathBuf,

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
            command:
                ProveCommand::ContinuationFrom {
                    prior_proof_path,
                    eth_rpc_url,
                },
            beacon_rpc_url,
        } => {
            let input = build_input(beacon_rpc_url, args.slot, eth_rpc_url).await?;
            let membership_proof: MembershipProof = bincode::deserialize(&read(prior_proof_path)?)?;
            let proof = build_proof(input, membership_proof, args.slot).await?;
            write(out_path, &bincode::serialize(&proof)?)?;
        }
        Command::Submit {
            eth_wallet_private_key,
            eth_rpc_url,
            contract,
            test_contract,
            proof_path,
        } => {
            submit_proof(
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
    receipt: Receipt,
}

impl MembershipProof {
    pub fn new(slot: u64, receipt: Receipt) -> Self {
        Self { slot, receipt }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AggregateProof {
    slot: u64,
    receipt: Receipt,
}

#[tracing::instrument(skip(beacon_rpc_url, eth_rpc_url))]
async fn build_input<'a>(
    beacon_rpc_url: Url,
    slot: u64,
    eth_rpc_url: Url,
) -> Result<OracleInput<'a>> {
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

    let input = OracleInput::build(
        WITHDRAWAL_CREDENTIALS,
        &beacon_block_header.message,
        &beacon_state,
        evm_input,
    )?;

    Ok(input)
}

#[tracing::instrument(skip(input, membership_proof))]
async fn build_proof<'a>(
    input: OracleInput<'a>,
    membership_proof: MembershipProof,
    slot: u64,
) -> Result<AggregateProof> {
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

async fn submit_proof(
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
