use bitvec::prelude::*;
use lido_oracle_core::{
    gindices::presets::mainnet::{state_roots_gindex, validator_withdrawal_credentials_gindex},
    io::validator_membership::{Input, Journal, ProofType},
    WITHDRAWAL_CREDENTIALS,
};
use risc0_zkvm::{guest::env, serde::to_vec};

pub fn main() {
    let Input {
        multiproof,
        current_state_root,
        proof_type,
        self_program_id,
        up_to_validator_index,
    } = env::read::<Input>();

    // verify the multi-proof which verifies leaf values
    multiproof
        .verify(current_state_root)
        .expect("Failed to verify multiproof");
    let mut leaves = multiproof.leaves();

    let (prior_up_to_validator_index, mut membership) = match proof_type {
        ProofType::Initial => (0, BitVec::<u32, Lsb0>::new()),
        ProofType::Continuation {
            prior_up_to_validator_index,
            ref prior_membership,
            ..
        } => (prior_up_to_validator_index, prior_membership.clone()),
    };

    for validator_index in (prior_up_to_validator_index..up_to_validator_index).rev() {
        let expeted_gindex = validator_withdrawal_credentials_gindex(validator_index);
        let (gindex, value) = leaves
            .next()
            .expect("Missing withdrawal_credentials value in multiproof");
        assert_eq!(*gindex, expeted_gindex);
        membership.push(value == &WITHDRAWAL_CREDENTIALS);
    }

    if let ProofType::Continuation {
        prior_state_root,
        prior_slot,
        prior_up_to_validator_index,
        prior_membership,
    } = proof_type
    {
        // Verify the pre-state requirement
        let (gindex, value) = leaves.next().expect("Missing state_root value in multiproof");
        assert_eq!(*gindex, state_roots_gindex(prior_slot));
        assert_eq!(*value, prior_state_root);

        // Verify the prior membership proof
        let prior_proof_journal = Journal {
            self_program_id: self_program_id,
            state_root: prior_state_root,
            up_to_validator_index: prior_up_to_validator_index,
            membership: prior_membership.clone(),
        };
        env::verify(self_program_id, &to_vec(&prior_proof_journal).unwrap()).expect("Failed to verify prior proof");
    }

    let journal = Journal {
        self_program_id,
        state_root: current_state_root,
        up_to_validator_index,
        membership,
    };
    env::commit(&journal);
}
