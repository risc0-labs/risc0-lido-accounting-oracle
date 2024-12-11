use alloy_primitives::B256;
use risc0_zkvm::{guest::env, serde::to_vec};

use lido_oracle_core::{
    gindices::presets::mainnet::{state_roots_gindex, validator_withdrawal_credentials_gindex},
    Input, Journal, Multiproof, ProofType,
};

pub fn main() {
    let Input {
        mut multiproof,
        current_state_root,
        proof_type,
        self_program_id,
        withdrawal_credentials,
        up_to_validator_index,
        ..
    } = env::read::<Input>();

    // verify the multi-proof which verifies all contained values in one go
    multiproof
        .verify(current_state_root)
        .expect("Failed to verify multiproof");
    multiproof.build_values_lookup();

    let (prior_up_to_validator_index, mut membership) = match proof_type {
        ProofType::Initial => (0, Vec::new()),
        ProofType::Continuation {
            prior_state_root,
            prior_slot,
            prior_up_to_validator_index,
            prior_membership,
        } => {
            // Verify the pre-state requirement
            assert!(verify_is_prestate(&multiproof, prior_state_root, prior_slot,));

            // Verify the prior membership proof
            let prior_proof_journal = Journal {
                self_program_id: self_program_id,
                state_root: prior_state_root,
                up_to_validator_index: prior_up_to_validator_index,
                withdrawal_credentials: withdrawal_credentials,
                membership: prior_membership.clone(),
            };
            env::verify(self_program_id, &to_vec(&prior_proof_journal).unwrap()).expect("Failed to verify prior proof");
            (prior_up_to_validator_index, prior_membership)
        }
    };

    // Extend the membership set with the new validators
    let validator_is_member = |validator_index: u64| {
        multiproof
            .get(
                validator_withdrawal_credentials_gindex(validator_index)
                    .try_into()
                    .unwrap(),
            )
            .unwrap()
            .expect("Missing withdrawal_credential value in the multiproof")
            == &withdrawal_credentials
    };

    for validator_index in prior_up_to_validator_index..up_to_validator_index {
        if validator_is_member(validator_index) {
            membership.push(validator_index);
        }
    }

    let journal = Journal {
        self_program_id,
        state_root: current_state_root,
        up_to_validator_index,
        withdrawal_credentials,
        membership,
    };
    env::commit(&journal);
}

/// Verify that the the given prior_state_root is a precursor to the current state in the blockchain
/// This is done byu checking it is in the `state_roots` list in the current state which stores the
/// previous SLOTS_PER_HISTORICAL_ROOT (8192 for mainnet) state roots
fn verify_is_prestate(multiproof: &Multiproof, prior_state_root: B256, prior_slot: u64) -> bool {
    multiproof
        .get(state_roots_gindex(prior_slot).try_into().unwrap())
        .unwrap()
        .expect("missing state_root value in multiproof")
        == &prior_state_root
}
