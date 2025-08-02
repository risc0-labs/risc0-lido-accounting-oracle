#![no_main]

use guest_io::sepolia::WITHDRAWAL_CREDENTIALS;

risc0_zkvm::guest::entry!(main);

fn main() {
    validator_membership::entry(WITHDRAWAL_CREDENTIALS);
}
