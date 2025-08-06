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

use crate::input::{
    ContinuationType::{LongRange, SameSlot, ShortRange},
    Input, ProofType,
};
use crate::journal::Journal;
use crate::{u64_from_b256, Node};
use alloy_primitives::{Address, U256};
use alloy_sol_types::SolType;
use bitvec::prelude::*;
use bitvec::vec::BitVec;
use gindices::presets::mainnet::beacon_state::post_electra as beacon_state_gindices;
use gindices::presets::mainnet::{
    beacon_block as beacon_block_gindices, historical_batch as historical_batch_gindices,
};
use risc0_steel::ethereum::EthChainSpec;
use risc0_steel::Account;
use sha2::{Digest, Sha256};
use ssz_multiproofs::ValueIterator;

use crate::error::Result;
use bytemuck::cast_slice;

pub fn generate_oracle_report(
    input: &Input,
    spec: &EthChainSpec,
    withdrawal_credentials: &[u8; 32],
    withdrawal_vault_address: Address,
) -> Result<Journal> {
    let Input {
        self_program_id,
        block_root,
        block_multiproof,
        state_multiproof: multiproof,
        evm_input,
        proof_type,
    } = input;

    // obtain the withdrawal vault balance from the EVM input
    let evm_env = evm_input.clone().into_env(spec);
    let account = Account::new(withdrawal_vault_address, &evm_env);
    let withdrawal_vault_balance: U256 = account.info().balance;

    tracing::info!("Verifying block multiproof");
    block_multiproof
        .verify(&block_root)
        .expect("Failed to verify block multiproof");
    let mut block_values = block_multiproof.values();

    let slot = get_slot(&mut block_values);
    let state_root = get_state_root(&mut block_values);

    tracing::info!("Verifying state multiproof");
    multiproof
        .verify(&state_root)
        .expect("Failed to verify state multiproof");
    let mut values = multiproof.values();

    let mut membership = match proof_type {
        ProofType::Initial => BitVec::<u32, Lsb0>::new(),
        ProofType::Continuation {
            prior_membership,
            cont_type,
            prior_receipt,
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
                LongRange {
                    hist_summary_multiproof,
                } => {
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

            let journal = Journal::abi_decode(&prior_receipt.journal.bytes)
                .expect("journal ABI decode failed");
            assert_eq!(journal.membershipCommitment, hash_bitvec(&prior_membership));

            prior_receipt
                .verify(*self_program_id)
                .expect("Failed to verify prior receipt");

            prior_membership.clone() // TODO: Avoid cloning this it is large
        }
    };

    let n_validators = u64_from_b256(
        multiproof
            .get(beacon_state_gindices::validator_count())
            .expect("validators len not available in multiproof"),
        0,
    );

    // Reserve the capacity for the membership bitvector to save cycles reallocating
    membership.reserve(n_validators.saturating_sub(membership.len() as u64) as usize);

    for validator_index in (membership.len() as u64)..n_validators {
        let value = values.next_assert_gindex(
            beacon_state_gindices::validator_withdrawal_credentials(validator_index),
        )?;
        membership.push(value == withdrawal_credentials);
    }

    // Compute the required oracle values from the beacon state values
    tracing::info!("Computing validator count, balances, exited validators");
    let num_exited_validators = count_exited_validators(&mut values, &membership, slot);

    let _ = values // slurp this out of the iterator, we already read it earlier
        .next_assert_gindex(beacon_state_gindices::validator_count())
        .expect("validator count not found in multiproof");

    let cl_balance = accumulate_balances(&mut values, &membership);

    // Commit the journal
    let journal = Journal {
        clBalanceGwei: U256::from(cl_balance),
        withdrawalVaultBalanceWei: withdrawal_vault_balance.into(),
        totalDepositedValidators: U256::from(n_validators),
        totalExitedValidators: U256::from(num_exited_validators),
        blockRoot: *block_root,
        commitment: evm_env.into_commitment(),
        membershipCommitment: hash_bitvec(&membership).into(),
    };

    Ok(journal)
}

fn get_slot<'a, I: Iterator<Item = (u64, &'a Node)>>(values: &mut ValueIterator<'a, I, 32>) -> u64 {
    let slot = values
        .next_assert_gindex(beacon_block_gindices::slot())
        .unwrap()
        .into();
    u64_from_b256(slot, 0)
}

fn get_state_root<'a, I: Iterator<Item = (u64, &'a Node)>>(
    values: &mut ValueIterator<'a, I, 32>,
) -> &'a Node {
    values
        .next_assert_gindex(beacon_block_gindices::state_root())
        .unwrap()
}

fn count_exited_validators<'a, I: Iterator<Item = (u64, &'a Node)>>(
    values: &mut ValueIterator<'a, I, 32>,
    membership: &BitVec<u32, Lsb0>,
    slot: u64,
) -> u64 {
    let current_epoch = slot / 32;
    let mut num_exited_validators = 0;
    // Iterate the validator exit epochs
    for validator_index in membership.iter_ones() {
        let value = values
            .next_assert_gindex(beacon_state_gindices::validator_exit_epoch(
                validator_index as u64,
            ))
            .unwrap();
        if u64_from_b256(&value, 0) <= current_epoch {
            num_exited_validators += 1;
        }
    }
    num_exited_validators
}

fn accumulate_balances<'a, I: Iterator<Item = (u64, &'a Node)>>(
    values: &mut ValueIterator<'a, I, 32>,
    membership: &BitVec<u32, Lsb0>,
) -> u64 {
    // accumulate the balances but iterating over the membership bitvec
    // This is a little tricky as multiple balances are packed into a single gindex
    let mut cl_balance = 0;
    let mut current_leaf = (0, &[0_u8; 32]); // 0 is an invalid gindex so this will always be updated on the first validator
    for validator_index in membership.iter_ones() {
        let expeted_gindex = beacon_state_gindices::validator_balance(validator_index as u64);
        if current_leaf.0 != expeted_gindex {
            current_leaf = values.next().expect(&format!(
                "Missing valdator {} balance value in multiproof",
                validator_index,
            ));
        }
        assert_eq!(current_leaf.0, expeted_gindex);
        let balance = u64_from_b256(&current_leaf.1, validator_index as usize % 4);
        cl_balance += balance;
    }
    cl_balance
}

/// Hash a bitvec in a way that includes the bitlength. Just hashing the underlying bytes is not sufficient
/// as any bits above the bitlength would be malleable
fn hash_bitvec(bv: &BitVec<u32>) -> [u8; 32] {
    let mut hasher = Sha256::new();

    // Hash bit length first
    hasher.update(&bv.len().to_le_bytes());

    // Then hash the actual bits as bytes
    let bytes = bv.clone().into_vec();
    hasher.update(cast_slice(&bytes));

    hasher.finalize().into()
}
