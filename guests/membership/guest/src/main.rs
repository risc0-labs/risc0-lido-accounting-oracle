// Copyright 2025 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::usize;

use bincode::deserialize;
use bitvec::prelude::*;
use gindices::presets::mainnet::beacon_state::post_electra as beacon_state_gindices;
use gindices::presets::mainnet::historical_batch as historical_batch_gindices;
use guest_io::validator_membership::{
    ContinuationType::{LongRange, SameSlot, ShortRange},
    Input, Journal, ProofType,
};
use guest_io::{InputWithReceipt, WITHDRAWAL_CREDENTIALS};
use risc0_zkvm::guest::env;

pub fn main() {
    env::log("Reading input");
    let input_bytes = env::read_frame();

    env::log("Deserializing input");
    let InputWithReceipt {
        input:
            Input {
                multiproof,
                state_root,
                proof_type,
                self_program_id,
                max_validator_index,
                hist_summary_multiproof,
            },
        receipt: prior_receipt,
    } = deserialize(&input_bytes).expect("Failed to deserialize input");

    // verify the multi-proof which verifies leaf values
    env::log("Verifying SSZ multiproof");
    multiproof
        .verify(&state_root)
        .expect("Failed to verify multiproof");
    let mut values = multiproof.values();

    let (start_validator_index, mut membership) = match proof_type {
        ProofType::Initial => (0, BitVec::<u32, Lsb0>::new()),
        ProofType::Continuation {
            prior_max_validator_index,
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
                        .next_assert_gindex(beacon_state_gindices::state_roots(prior_slot))
                        .unwrap();
                    assert_eq!(stored_root, &prior_state_root);
                }
                LongRange => {
                    let hist_summary_multiproof = hist_summary_multiproof.expect(
                        "Missing historical summary multiproof for a long range continuation",
                    );
                    let historical_summary_root =
                        multiproof // using a get here for now but this does cause an extra iteration through the values
                            .get::<32>(beacon_state_gindices::historical_summaries(
                                prior_slot,
                            ))
                            .unwrap();
                    hist_summary_multiproof
                        .verify(&historical_summary_root)
                        .expect("Failed to verify historical summary multiproof given the root in the current state");
                    let stored_root = hist_summary_multiproof
                        .get(historical_batch_gindices::state_roots(prior_slot))
                        .unwrap();
                    assert_eq!(stored_root, &prior_state_root);
                }
            }
            let prior_receipt = prior_receipt.expect("Missing prior receipt for continuation");
            // ensure the values in the journal match
            let prior_proof_journal = Journal {
                self_program_id,
                state_root: prior_state_root,
                max_validator_index: prior_max_validator_index,
                membership: prior_membership,
            };
            assert_eq!(
                prior_receipt.journal.bytes,
                prior_proof_journal.to_bytes().unwrap()
            );
            // Verify the prior membership proof.
            env::log("Verifying prior membership ZK proof");
            #[cfg(not(feature = "skip-verify"))]
            prior_receipt
                .verify(self_program_id)
                .expect("Failed to verify prior receipt");

            (
                prior_max_validator_index + 1,
                prior_proof_journal.membership,
            )
        }
    };

    // Reserve the capacity for the membership bitvector to save cycles reallocating
    // and to save memory by not overallocating
    membership.reserve(
        (max_validator_index - start_validator_index)
            .try_into()
            .unwrap_or(usize::MAX),
    );

    env::log("Enumerating validators");
    for validator_index in start_validator_index..=max_validator_index {
        let value = values
            .next_assert_gindex(beacon_state_gindices::validator_withdrawal_credentials(
                validator_index,
            ))
            .unwrap();
        membership.push(value == &WITHDRAWAL_CREDENTIALS);
    }

    let journal = Journal {
        self_program_id,
        state_root,
        max_validator_index,
        membership,
    };
    env::commit(&journal);
}
