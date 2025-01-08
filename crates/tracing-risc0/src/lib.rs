use risc0_zkvm::guest::env;
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields};
use tracing_subscriber::fmt::{self, format::Writer};
use tracing_subscriber::registry::LookupSpan;

pub struct Risc0Formatter;

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
        fmt::format()
            .without_time()
            .format_event(ctx, writer, event)
    }
}
