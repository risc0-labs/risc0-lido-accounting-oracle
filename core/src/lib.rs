mod balance_iterator;
mod error;
pub mod gindices;
pub mod io;
mod multiproof_builder;
mod multiproof_verification;

pub use multiproof_builder::Multiproof;
#[cfg(feature = "builder")]
pub use multiproof_builder::MultiproofBuilder;
pub(crate) use multiproof_verification::verify_merkle_multiproof;

pub const WITHDRAWAL_CREDENTIALS: alloy_primitives::B256 = alloy_primitives::B256::ZERO;
