// Copyright 2023 RISC Zero, Inc.
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

use bitvec::prelude::*;
use gindices::presets::mainnet::beacon_state::{self as beacon_state_gindices};
use gindices::presets::mainnet::historical_batch as historical_batch_gindices;
use guest_io::validator_membership::{
    ContinuationType::{LongRange, SameSlot, ShortRange},
    Input, Journal, ProofType,
};
use guest_io::WITHDRAWAL_CREDENTIALS;
use tracing_risc0::Risc0Formatter;
use tracing_subscriber::fmt::format::FmtSpan;

use risc0_zkvm::{guest::env, serde::to_vec};

pub fn main() {
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
        .with_writer(env::stdout)
        .without_time()
        .event_format(Risc0Formatter)
        .init();

    let Input {
        multiproof,
        state_root,
        proof_type,
        self_program_id,
        max_validator_index,
    } = env::read::<Input>();

    // verify the multi-proof which verifies leaf values
    multiproof
        .verify(&state_root)
        .expect("Failed to verify multiproof");
    let mut values = multiproof.values();

    let (start_validator_index, mut membership) = match proof_type {
        ProofType::Initial => (0, BitVec::<u32, Lsb0>::new()),
        ProofType::Continuation {
            prior_max_validator_index,
            ref prior_membership,
            ..
        } => (prior_max_validator_index + 1, prior_membership.clone()),
    };

    if let ProofType::Continuation {
        prior_state_root,
        prior_slot,
        prior_max_validator_index,
        prior_membership,
        cont_type,
    } = proof_type
    {
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
            LongRange {
                hist_summary_multiproof,
            } => {
                let historical_summary_root =
                    multiproof // using a get here for now but this does cause an extra iteration through the values :(
                        .get(beacon_state_gindices::historical_summaries(
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

        // Verify the prior membership proof
        let prior_proof_journal = Journal {
            self_program_id: self_program_id.into(),
            state_root: prior_state_root,
            max_validator_index: prior_max_validator_index,
            membership: prior_membership.clone(),
        };
        env::verify(self_program_id, &to_vec(&prior_proof_journal).unwrap())
            .expect("Failed to verify prior proof");
    }

    for validator_index in start_validator_index..=max_validator_index {
        let value = values
            .next_assert_gindex(beacon_state_gindices::validator_withdrawal_credentials(
                validator_index,
            ))
            .unwrap();
        membership.push(value == &WITHDRAWAL_CREDENTIALS);
    }

    let journal = Journal {
        self_program_id: self_program_id.into(),
        state_root,
        max_validator_index,
        membership,
    };
    env::commit(&journal);
}
