use alloy_primitives::B256;
use gindices::presets::mainnet::beacon_block as beacon_block_gindices;
use gindices::presets::mainnet::beacon_state as beacon_state_gindices;
use guest_io::balance_and_exits::{Input, Journal};
use risc0_zkvm::guest::env;
use tracing_subscriber::fmt::format::FmtSpan;

use tracing::{info, Event, Subscriber};
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
    // #[cfg(feature = "builder")]
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
        .with_writer(env::stdout)
        .without_time()
        .event_format(Risc0Formatter)
        .init();

    tracing::info!("Starting balance and exits program");

    info!("start: reading input");
    let Input {
        block_root,
        membership,
        block_multiproof,
        state_multiproof: multiproof,
        ..
    } = env::read::<Input>();
    info!("end: reading input");

    info!("start: verifying block multiproof");
    block_multiproof
        .verify(&block_root)
        .expect("Failed to verify block multiproof");
    info!("end: verifying block multiproof");

    info!("start: checking block contains state root");
    let (state_root_gindex, state_root) = block_multiproof
        .values()
        .next()
        .expect("Missing state root in multiproof");
    assert_eq!(state_root_gindex, beacon_block_gindices::state_root());
    info!("end: checking block contains state root");

    info!("start: verifying state multiproof");
    multiproof
        .verify(&state_root)
        .expect("Failed to verify state multiproof");
    let mut values = multiproof.values();
    info!("end: verifying state multiproof");

    let current_epoch = 0;
    let num_validators = membership.count_ones() as u64;
    let mut cl_balance = 0;
    let mut num_exited_validators = 0;

    info!("start: iterating validator exits");
    // Iterate the validator exit epochs
    for validator_index in membership.iter_ones() {
        let expeted_gindex = beacon_state_gindices::validator_exit_epoch(validator_index as u64);
        let (gindex, value) = values.next().expect("Missing validator_exit_epoch value in multiproof");
        assert_eq!(gindex, expeted_gindex);
        if u64_from_b256(&value, 0) <= current_epoch {
            num_exited_validators += 1;
        }
    }
    info!("end: iterating validator exits");

    info!("start: iterating balances");

    // accumulate the balances
    // This is a little tricky as multiple balances are packed into a single gindex
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
    info!("end: iterating balances");

    info!("start: writing journal");
    let journal = Journal {
        block_root,
        num_validators,
        cl_balance,
        num_exited_validators,
    };
    env::commit(&journal);
    info!("end: writing journal");
}

/// Slice an 8 byte u64 out of a 32 byte chunk
/// pos gives the position (e.g. first 8 bytes, second 8 bytes, etc.)
fn u64_from_b256(node: &B256, pos: usize) -> u64 {
    u64::from_le_bytes(node[pos * 8..(pos + 1) * 8].try_into().unwrap())
}
