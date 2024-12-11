use alloy_primitives::B256;
use risc0_zkvm::guest::env;

use lido_oracle_core::{
    gindices::presets::mainnet::{state_roots_gindex, validator_withdrawal_credentials_gindex},
    Input, Journal, Multiproof,
};

pub fn main() {
    let Input {
        self_program_id,
        withdrawal_credentials,
        current_state_root,
        max_validator_index,
        prior_state_root,
        prior_slot,
        prior_max_validator_index,
        prior_membership,
        mut multiproof,
    } = env::read::<Input>();

    // verify the multi-proof which verifies all contained values in one go
    multiproof
        .verify(current_state_root)
        .expect("Failed to verify multiproof");
    multiproof.build_values_lookup();

    // Verify the prior membership proof
    let prior_proof_journal = Journal {
        self_program_id: self_program_id,
        state_root: prior_state_root,
        max_validator_index: prior_max_validator_index,
        withdrawal_credentials: withdrawal_credentials,
        membership: prior_membership.clone(),
    };
    // env::verify(self_program_id, &to_vec(&prior_proof_journal).unwrap()).expect("Failed to verify prior proof");

    // Verify the pre-state requirement
    assert!(verify_is_prestate(&multiproof, prior_state_root, prior_slot,));

    // Extend the membership set with the new validators
    let mut membership = Vec::with_capacity(max_validator_index as usize + 1);
    membership.copy_from_slice(&prior_membership);

    let validator_is_member = |validator_index: u64| {
        multiproof
            .get(
                validator_withdrawal_credentials_gindex(validator_index)
                    .try_into()
                    .unwrap(),
            )
            .unwrap()
            .unwrap()
            == &withdrawal_credentials
    };

    for validator_index in (prior_max_validator_index + 1)..=max_validator_index {
        membership.push(validator_is_member(validator_index));
    }

    let journal = Journal {
        self_program_id,
        state_root: current_state_root,
        max_validator_index,
        withdrawal_credentials,
        membership,
    };
    env::commit(&journal);
}

fn verify_is_prestate(multiproof: &Multiproof, prior_state_root: B256, prior_slot: u64) -> bool {
    multiproof
        .get(state_roots_gindex(prior_slot).try_into().unwrap())
        .unwrap()
        .unwrap()
        == &prior_state_root
}
