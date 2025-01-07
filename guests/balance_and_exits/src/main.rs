use alloy_primitives::B256;
use bitvec::prelude::*;
use bitvec::vec::BitVec;
use gindices::presets::mainnet::beacon_block as beacon_block_gindices;
use gindices::presets::mainnet::beacon_state as beacon_state_gindices;
use guest_io::balance_and_exits::{Input, Journal};
use risc0_zkvm::guest::env;
use ssz_multiproofs::Multiproof;
use tracing_subscriber::fmt::format::FmtSpan;

use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields};
use tracing_subscriber::fmt::{self, format::Writer};
use tracing_subscriber::registry::LookupSpan;

struct Risc0Formatter;

impl<S, N> FormatEvent<S, N> for Risc0Formatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        // Write the custom field
        write!(writer, "R0VM[cycles={}]", env::cycle_count())?;

        // Use the default formatter to format the rest of the event
        fmt::format().without_time().format_event(ctx, writer, event)
    }
}

pub fn main() {
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
        .with_writer(env::stdout)
        .without_time()
        .event_format(Risc0Formatter)
        .init();

    let Input {
        block_root,
        membership,
        block_multiproof,
        state_multiproof: multiproof,
        ..
    } = env::read::<Input>();

    block_multiproof
        .verify(&block_root)
        .expect("Failed to verify block multiproof");

    let state_root = verify_state_root_in_block(&block_multiproof, &block_root);

    multiproof
        .verify(&state_root)
        .expect("Failed to verify state multiproof");
    let mut values = multiproof.values();

    let num_validators = membership.count_ones() as u64;
    let num_exited_validators = count_exited_validators(&mut values, &membership, 0); // TODO: Use actual current epoch
    let cl_balance = accumulate_balances(&mut values, &membership);

    let journal = Journal {
        block_root,
        num_validators,
        cl_balance,
        num_exited_validators,
    };
    env::commit(&journal);
}

#[tracing::instrument(skip(block_multiproof))]
fn verify_state_root_in_block<'a>(block_multiproof: &'a Multiproof, state_root: &B256) -> &'a B256 {
    let (state_root_gindex, state_root) = block_multiproof
        .values()
        .next()
        .expect("Missing state root in multiproof");
    assert_eq!(state_root_gindex, beacon_block_gindices::state_root());
    state_root
}

#[tracing::instrument(skip(values, membership))]
fn count_exited_validators<'a, I: Iterator<Item = (u64, &'a B256)>>(
    values: &mut I,
    membership: &BitVec<u32, Lsb0>,
    current_epoch: u64,
) -> u64 {
    let mut num_exited_validators = 0;
    // Iterate the validator exit epochs
    for validator_index in membership.iter_ones() {
        let expeted_gindex = beacon_state_gindices::validator_exit_epoch(validator_index as u64);
        let (gindex, value) = values.next().expect("Missing validator_exit_epoch value in multiproof");
        assert_eq!(gindex, expeted_gindex);
        if u64_from_b256(&value, 0) <= current_epoch {
            num_exited_validators += 1;
        }
    }
    num_exited_validators
}

fn accumulate_balances<'a, I: Iterator<Item = (u64, &'a B256)>>(values: &mut I, membership: &BitVec<u32, Lsb0>) -> u64 {
    // accumulate the balances
    // This is a little tricky as multiple balances are packed into a single gindex
    let mut cl_balance = 0;
    let mut current_leaf = values.next().expect("Missing valdator balance value in multiproof");
    for validator_index in membership.iter_ones() {
        let expeted_gindex = beacon_state_gindices::validator_balance(validator_index as u64);
        if current_leaf.0 != expeted_gindex {
            current_leaf = values.next().expect("Missing valdator balance value in multiproof");
        }
        assert_eq!(current_leaf.0, expeted_gindex);
        let balance = u64_from_b256(&current_leaf.1, validator_index as usize % 4);
        cl_balance += balance;
    }
    cl_balance
}

/// Slice an 8 byte u64 out of a 32 byte chunk
/// pos gives the position (e.g. first 8 bytes, second 8 bytes, etc.)
fn u64_from_b256(node: &B256, pos: usize) -> u64 {
    u64::from_le_bytes(node[pos * 8..(pos + 1) * 8].try_into().unwrap())
}
