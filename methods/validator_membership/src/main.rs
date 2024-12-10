use alloy_primitives::U256;
use bitvec::prelude::*;

use risc0_zkvm::serde::to_vec;
use risc0_zkvm::{guest::env, sha::Digest};

use lido_oracle_core::{
    beacon_types::presets::mainnet::{BeaconState, SLOTS_PER_HISTORICAL_ROOT},
    Input, Journal, Multiproof, Node,
};

pub fn main() {
    let input: Input = env::read::<Input>();

    // verify the multi-proof which verifies all contain values in one go
    input
        .multiproof
        .verify(input.current_state_root.into())
        .expect("Failed to verify multiproof");

    // Verify the prior membership proof
    let prior_proof_journal = Journal {
        self_program_id: input.self_program_id,
        state_root: input.prior_state_root,
        max_validator_index: input.prior_max_validator_index,
        withdrawal_credentials: input.withdrawal_credentials,
        membership: input.prior_membership,
    };
    // env::verify(input.self_program_id, &to_vec(&prior_proof_journal).unwrap()).expect("Failed to verify prior proof");

    // Verify the pre-state requirement
    assert!(verify_is_prestate(
        &input.multiproof,
        input.prior_state_root.into(),
        input.prior_slot as usize,
    ));

    // Verify the inclusion proofs for every validator

    // Update the membership bitfield with the new validators
}

fn verify_is_prestate(multiproof: &Multiproof, prior_state_root: Node, prior_slot: usize) -> bool {
    multiproof
        .get::<BeaconState>(&["state_roots".into(), (prior_slot % SLOTS_PER_HISTORICAL_ROOT).into()])
        .unwrap()
        == prior_state_root
}
