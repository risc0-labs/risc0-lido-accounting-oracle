use core::num;

use alloy_primitives::B256;
use bitvec::prelude::*;
use lido_oracle_core::{
    gindices::presets::mainnet::{state_roots_gindex, validator_balance_gindex},
    io::balance_and_exits::{Input, Journal},
};
use risc0_zkvm::{guest::env, serde::to_vec};

pub fn main() {
    let Input {
        block_root,
        membership,
        multiproof,
        ..
    } = env::read::<Input>();
    multiproof.verify(block_root).expect("Failed to verify multiproof");
    let mut leaves = multiproof.leaves();

    let num_validators = membership.count_ones() as u64;
    let mut cl_balance = 0;
    let mut num_exited_validators = 0;

    // accumulate the balances first
    let mut current_leaf = leaves.next().expect("Missing valdator balance value in multiproof");
    for validator_index in membership.iter_ones().rev() {
        let expeted_gindex = validator_balance_gindex(validator_index as u64);
        if current_leaf.0 != expeted_gindex {
            current_leaf = leaves.next().expect("Missing valdator balance value in multiproof");
        }
        assert_eq!(current_leaf.0, expeted_gindex);
        let balance = u64_from_node(&current_leaf.1, validator_index as usize % 4);
        cl_balance += balance;
    }

    // Then the exit status
    // for validator_index in membership.iter_ones() {}

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
fn u64_from_node(node: &B256, pos: usize) -> u64 {
    u64::from_le_bytes(node[pos * 8..(pos + 1) * 8].try_into().unwrap())
}
