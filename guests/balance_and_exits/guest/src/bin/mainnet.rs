#![no_main]

use guest_io::mainnet::WITHDRAWAL_VAULT_ADDRESS;
use risc0_steel::ethereum::ETH_MAINNET_CHAIN_SPEC;

risc0_zkvm::guest::entry!(main);

fn main() {
    balance_and_exits::entry(
        &ETH_MAINNET_CHAIN_SPEC,
        WITHDRAWAL_VAULT_ADDRESS,
        membership_builder::MAINNET_ID,
    );
}
