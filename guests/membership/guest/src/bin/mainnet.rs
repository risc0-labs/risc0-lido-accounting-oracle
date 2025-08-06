#![no_main]

use lido_oracle_core::mainnet::WITHDRAWAL_CREDENTIALS;

risc0_zkvm::guest::entry!(main);

fn main() {
    validator_membership::entry(WITHDRAWAL_CREDENTIALS);
}
