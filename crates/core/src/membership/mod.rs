pub mod io;

use crate::{error::Result, u64_from_b256};
use bitvec::prelude::*;
use gindices::presets::mainnet::beacon_state::post_electra as beacon_state_gindices;
use gindices::presets::mainnet::historical_batch as historical_batch_gindices;
use io::{
    ContinuationType::{LongRange, SameSlot, ShortRange},
    Input, ProofType,
};
use risc0_zkvm::Receipt;

/// Given an input, check the required SSZ proofs and update the membership bitfield accordingly
pub fn update_membership(
    input: &Input,
    prior_receipt: Option<Receipt>,
    withdrawal_credentials: &[u8; 32],
) -> Result<BitVec<u32, Lsb0>> {
    let Input {
        multiproof,
        state_root,
        proof_type,
        hist_summary_multiproof,
        ..
    } = input;

    multiproof.verify(&state_root)?;
    let mut values = multiproof.values();

    let mut membership = match proof_type {
        ProofType::Initial => BitVec::<u32, Lsb0>::new(),
        ProofType::Continuation {
            prior_membership,
            cont_type,
            prior_slot,
            prior_state_root,
        } => {
            match cont_type {
                SameSlot => {
                    assert_eq!(state_root, prior_state_root);
                }
                ShortRange => {
                    let stored_root = values
                        .next_assert_gindex(beacon_state_gindices::state_roots(*prior_slot))?;
                    assert_eq!(stored_root, &prior_state_root);
                }
                LongRange => {
                    let hist_summary_multiproof = hist_summary_multiproof.as_ref().expect(
                        "Missing historical summary multiproof for a long range continuation",
                    );
                    let historical_summary_root =
                        multiproof // using a get here for now but this does cause an extra iteration through the values
                            .get(beacon_state_gindices::historical_summaries(
                                *prior_slot,
                            ))
                            .unwrap();
                    hist_summary_multiproof
                        .verify(&historical_summary_root)
                        .expect("Failed to verify historical summary multiproof given the root in the current state");
                    let stored_root = hist_summary_multiproof
                        .get(historical_batch_gindices::state_roots(*prior_slot))
                        .unwrap();
                    assert_eq!(stored_root, &prior_state_root);
                }
            }

            // ensure the values in the journal match
            let prior_proof_journal = io::Journal {
                self_program_id: input.self_program_id,
                state_root: prior_state_root.clone(),
                membership: prior_membership.clone(), // TODO: Avoid cloning this it is large
            };

            let prior_receipt = prior_receipt.expect("Missing prior receipt for continuation");

            assert_eq!(
                prior_receipt.journal.bytes,
                prior_proof_journal.to_bytes().unwrap()
            );

            prior_receipt
                .verify(input.self_program_id)
                .expect("Failed to verify prior receipt");

            prior_proof_journal.membership
        }
    };

    let n_validators = u64_from_b256(
        values.next_assert_gindex(beacon_state_gindices::validator_count())?,
        0,
    );

    // Reserve the capacity for the membership bitvector to save cycles reallocating
    // and to save memory by not overallocating
    membership.reserve(n_validators.saturating_sub(membership.len() as u64) as usize);

    for validator_index in (membership.len() as u64 + 1)..n_validators {
        let value = values.next_assert_gindex(
            beacon_state_gindices::validator_withdrawal_credentials(validator_index),
        )?;
        membership.push(value == withdrawal_credentials);
    }

    Ok(membership)
}
