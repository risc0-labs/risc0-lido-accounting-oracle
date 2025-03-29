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

use alloy_primitives::U256;
use alloy_sol_types::SolValue;
use bincode::deserialize;
use bitvec::prelude::*;
use bitvec::vec::BitVec;
use gindices::presets::mainnet::beacon_block as beacon_block_gindices;
use gindices::presets::mainnet::beacon_state as beacon_state_gindices;
use guest_io::balance_and_exits::{Input, Journal};
use guest_io::validator_membership::Journal as MembershipJounal;
use guest_io::ANVIL_CHAIN_SPEC;
use guest_io::{InputWithReceipt, WITHDRAWAL_VAULT_ADDRESS};
use membership_builder::VALIDATOR_MEMBERSHIP_ID;
use risc0_steel::Account;
use risc0_zkvm::guest::env;
use risc0_zkvm::Receipt;
use ssz_multiproofs::ValueIterator;
use tracing_risc0::Risc0Formatter;
use tracing_subscriber::fmt::format::FmtSpan;

type Node = [u8; 32];

pub fn main() {
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
        .with_writer(env::stdout)
        .without_time()
        .event_format(Risc0Formatter)
        .init();

    let input_bytes = env::read_frame();
    let InputWithReceipt {
        input:
            Input {
                block_root,
                membership,
                block_multiproof,
                state_multiproof: multiproof,
                evm_input,
            },
        receipt: membership_receipt,
    } = deserialize(&input_bytes).expect("Failed to deserialize input");

    // TODO: Currently block_root is unconstrained making the whole guest unconstrained
    //       This is included as part of the steel commitment and checked on-chain but currently
    //       there is no way to access this from the Steel evm_input.

    // obtain the withdrawal vault balance from the EVM input
    let env = evm_input.into_env().with_chain_spec(&ANVIL_CHAIN_SPEC);
    let account = Account::new(WITHDRAWAL_VAULT_ADDRESS, &env);
    let withdrawal_vault_balance: U256 = account.info().balance;

    block_multiproof
        .verify(&block_root)
        .expect("Failed to verify block multiproof");
    let mut block_values = block_multiproof.values();

    let slot = get_slot(&mut block_values);
    let state_root = get_state_root(&mut block_values);

    multiproof
        .verify(&state_root)
        .expect("Failed to verify state multiproof");
    let mut values = multiproof.values();

    // Compute the required values from the beacon state values
    let num_validators = membership.count_ones() as u64;
    let num_exited_validators = count_exited_validators(&mut values, &membership, slot);
    let validator_count = get_validator_count(&mut values);
    let cl_balance = accumulate_balances(&mut values, &membership);

    // verify the membership proof
    verify_membership(
        state_root,
        membership,
        validator_count,
        membership_receipt.expect("No membership receipt provided"),
    );

    // Commit the journal
    let journal = Journal {
        clBalanceGwei: U256::from(cl_balance),
        withdrawalVaultBalanceWei: withdrawal_vault_balance.into(),
        totalDepositedValidators: U256::from(num_validators),
        totalExitedValidators: U256::from(num_exited_validators),
        commitment: env.into_commitment(),
    };
    env::commit_slice(&journal.abi_encode());
}

fn verify_membership(
    state_root: &Node,
    membership: BitVec<u32, Lsb0>,
    validator_count: u64,
    membership_receipt: Receipt,
) {
    let j = MembershipJounal {
        self_program_id: VALIDATOR_MEMBERSHIP_ID.into(),
        state_root: state_root.clone().into(),
        membership: membership,
        max_validator_index: validator_count - 1,
    };
    let membership_receipt = membership_receipt;
    assert_eq!(membership_receipt.journal.bytes, j.to_bytes().unwrap());
    membership_receipt
        .verify(VALIDATOR_MEMBERSHIP_ID)
        .expect("Failed to verify membership receipt");
}

fn get_slot<'a, I: Iterator<Item = (u64, &'a Node)>>(values: &mut ValueIterator<'a, I, 32>) -> u64 {
    let slot = values
        .next_assert_gindex(beacon_block_gindices::slot())
        .unwrap();
    u64_from_b256(slot, 0)
}

fn get_state_root<'a, I: Iterator<Item = (u64, &'a Node)>>(
    values: &mut ValueIterator<'a, I, 32>,
) -> &'a Node {
    values
        .next_assert_gindex(beacon_block_gindices::state_root())
        .unwrap()
}

fn get_validator_count<'a, I: Iterator<Item = (u64, &'a Node)>>(
    values: &mut ValueIterator<'a, I, 32>,
) -> u64 {
    let validator_count = values
        .next_assert_gindex(beacon_state_gindices::validator_count())
        .unwrap();
    u64_from_b256(validator_count, 0)
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

/// Slice an 8 byte u64 out of a 32 byte chunk
/// pos gives the position (e.g. first 8 bytes, second 8 bytes, etc.)
fn u64_from_b256(node: &Node, pos: usize) -> u64 {
    u64::from_le_bytes(node[pos * 8..(pos + 1) * 8].try_into().unwrap())
}
