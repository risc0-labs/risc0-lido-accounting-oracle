use alloy_primitives::B256;
use lido_oracle_core::{
    gindices::presets::mainnet::{beacon_block as beacon_block_gindices, beacon_state as beacon_state_gindices},
    io::balance_and_exits::{Input, Journal},
};
use risc0_zkvm::guest::env;

pub fn main() {
    let Input {
        block_root,
        membership,
        block_multiproof,
        state_multiproof: multiproof,
        ..
    } = env::read::<Input>();

    block_multiproof
        .verify(&block_root)
        .expect("Failed to verify block multiproof");

    let (state_root_gindex, state_root) = block_multiproof
        .values()
        .next()
        .expect("Missing state root in multiproof");
    assert_eq!(state_root_gindex, beacon_block_gindices::state_root());

    multiproof
        .verify(&state_root)
        .expect("Failed to verify state multiproof");
    let mut values = multiproof.values();

    let current_epoch = 0;
    let num_validators = membership.count_ones() as u64;
    let mut cl_balance = 0;
    let mut num_exited_validators = 0;

    // Iterate the validator exit epochs
    for validator_index in membership.iter_ones() {
        let expeted_gindex = beacon_state_gindices::validator_exit_epoch(validator_index as u64);
        let (gindex, value) = values
            .next()
            .expect("Missing withdrawal_credentials value in multiproof");
        assert_eq!(gindex, expeted_gindex);
        if u64_from_b256(&value, 0) <= current_epoch {
            num_exited_validators += 1;
        }
    }

    // accumulate the balances
    // This is a little tricky as multiple balances are packed into a single gindex
    let mut current_leaf = values.next().expect("Missing valdator balance value in multiproof");
    for validator_index in membership.iter_ones() {
        let expeted_gindex = beacon_state_gindices::validator_balance(validator_index as u64);
        if current_leaf.0 != expeted_gindex {
            current_leaf = values.next().expect("Missing valdator balance value in multiproof");
        }
        assert_eq!(current_leaf.0, expeted_gindex);
        let balance = u64_from_b256(&current_leaf.1, validator_index as usize % 4);
        cl_balance += balance;
    }

    let journal = Journal {
        block_root,
        num_validators,
        cl_balance,
        num_exited_validators,
    };
    env::commit(&journal);
}

/// Slice an 8 byte u64 out of a 32 byte chunk
/// pos gives the position (e.g. first 8 bytes, second 8 bytes, etc.)
fn u64_from_b256(node: &B256, pos: usize) -> u64 {
    u64::from_le_bytes(node[pos * 8..(pos + 1) * 8].try_into().unwrap())
}
