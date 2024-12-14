mod error;
pub mod gindices;
mod io;
mod multiproof_builder;
mod multiproof_verification;

pub use io::{Input, Journal, ProofType};
pub use multiproof_builder::Multiproof;
#[cfg(feature = "builder")]
pub use multiproof_builder::MultiproofBuilder;
pub(crate) use multiproof_verification::verify_merkle_multiproof;
