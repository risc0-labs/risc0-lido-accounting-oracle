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
    for validator_index in membership.iter_ones() {
        let expeted_gindex = validator_balance_gindex(validator_index as u64);
        if current_index > expeted_gindex {
            panic!("Missing validator balance value in multiproof");
        }
        let (gindex, value) = leaves.next().expect("Missing valdator balance value in multiproof");
        assert_eq!(*gindex, expeted_gindex);
        cl_balance += value;
    }

    // Then the exit status
    for validator_index in membership.iter_ones() {}

    let journal = Journal {
        block_root,
        num_validators,
        cl_balance,
        num_exited_validators,
    };
    env::commit(&journal);
}
