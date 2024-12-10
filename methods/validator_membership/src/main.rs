use alloy_primitives::U256;
use bitvec::prelude::*;

use risc0_zkvm::serde::to_vec;
use risc0_zkvm::{guest::env, sha::Digest};

use lido_oracle_core::{Input, Journal};

pub fn main() {
    let input: Input = env::read::<Input>();

    // Verify the prior membership proof
    // let prior_proof_journal = Journal {
    //     self_program_id: input.self_program_id,
    //     state_root: input.prior_state_root,
    //     max_validator_index: input.prior_max_validator_index,
    //     withdrawal_credentials: input.withdrawal_credentials,
    //     membership: input.prior_membership,
    // };
    // env::verify(input.self_program_id, &to_vec(&prior_proof_journal).unwrap()).expect("Failed to verify prior proof");

    // Verify the pre-state requirement
    // assert!(verify_is_prestate(
    //     input.current_state_root,
    //     input.prior_state_root,
    //     input.pre_state_proof
    // ));

    // Verify the inclusion proofs for every validator

    // Update the membership bitfield with the new validators
}

fn verify_is_prestate(current_state_root: U256, prior_state_root: U256, pre_state_proof: Vec<u8>) -> bool {
    true
}
