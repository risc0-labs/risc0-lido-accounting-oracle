mod error;
mod multiproof_builder;

pub use error::{Error, Result};
pub use multiproof_builder::Multiproof;
#[cfg(feature = "builder")]
pub use multiproof_builder::MultiproofBuilder;
