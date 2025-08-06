use crate::membership::io::Journal as MembershipJounal;
use crate::{u64_from_b256, Node};
use alloy_primitives::{Address, U256};
use bitvec::prelude::*;
use bitvec::vec::BitVec;
use gindices::presets::mainnet::beacon_block as beacon_block_gindices;
use gindices::presets::mainnet::beacon_state::post_electra as beacon_state_gindices;
use risc0_steel::ethereum::EthChainSpec;
use risc0_steel::Account;
use risc0_zkvm::guest::env;
use risc0_zkvm::Receipt;
use ssz_multiproofs::ValueIterator;

use crate::error::Result;
use io::{Input, Journal};

pub mod io;

pub fn generate_oracle_report(
    spec: &EthChainSpec,
    input: &Input,
    membership_receipt: Receipt,
    membership_program_id: [u32; 8],
    withdrawal_vault_address: Address,
) -> Result<Journal> {
    let Input {
        block_root,
        membership,
        block_multiproof,
        state_multiproof: multiproof,
        evm_input,
    } = input;

    // obtain the withdrawal vault balance from the EVM input
    let evm_env = evm_input.clone().into_env(spec);
    let account = Account::new(withdrawal_vault_address, &evm_env);
    let withdrawal_vault_balance: U256 = account.info().balance;

    env::log("Verifying block multiproof");
    block_multiproof
        .verify(&block_root)
        .expect("Failed to verify block multiproof");
    let mut block_values = block_multiproof.values();

    let slot = get_slot(&mut block_values);
    let state_root = get_state_root(&mut block_values);

    env::log("Verifying state multiproof");
    multiproof
        .verify(&state_root)
        .expect("Failed to verify state multiproof");
    let mut values = multiproof.values();

    // Compute the required values from the beacon state values
    env::log("Computing validator count, balances, exited validators");
    let num_validators = membership.count_ones() as u64;
    let num_exited_validators = count_exited_validators(&mut values, &membership, slot);
    let cl_balance = accumulate_balances(&mut values, &membership);

    env::log("Verifying validator membership proof");
    verify_membership(
        membership_program_id,
        state_root,
        membership,
        membership_receipt,
    );

    // Commit the journal
    let journal = Journal {
        clBalanceGwei: U256::from(cl_balance),
        withdrawalVaultBalanceWei: withdrawal_vault_balance.into(),
        totalDepositedValidators: U256::from(num_validators),
        totalExitedValidators: U256::from(num_exited_validators),
        blockRoot: *block_root,
        commitment: evm_env.into_commitment(),
    };

    Ok(journal)
}

fn verify_membership(
    membership_program_id: [u32; 8],
    state_root: &Node,
    membership: &BitVec<u32, Lsb0>,
    membership_receipt: Receipt,
) {
    let j = MembershipJounal {
        self_program_id: membership_program_id.into(),
        state_root: state_root.clone().into(),
        membership: membership.clone(), // TODO: Avoid cloning this it is large
    };
    let membership_receipt = membership_receipt;
    assert_eq!(membership_receipt.journal.bytes, j.to_bytes().unwrap());
    membership_receipt
        .verify(membership_program_id)
        .expect("Failed to verify membership receipt");
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
