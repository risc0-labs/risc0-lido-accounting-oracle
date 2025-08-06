#![no_main]

use lido_oracle_core::mainnet::{WITHDRAWAL_CREDENTIALS, WITHDRAWAL_VAULT_ADDRESS};
use risc0_steel::ethereum::ETH_MAINNET_CHAIN_SPEC;

risc0_zkvm::guest::entry!(main);

fn main() {
    oracle::entry(
        &ETH_MAINNET_CHAIN_SPEC,
        &WITHDRAWAL_CREDENTIALS,
        WITHDRAWAL_VAULT_ADDRESS,
    );
}
