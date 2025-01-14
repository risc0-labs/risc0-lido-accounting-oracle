#[cfg(feature = "builder")]
mod builder;
mod error;
mod multiproof;

#[cfg(all(test, feature = "builder"))]
mod tests;

use bitvec::prelude::*;

#[cfg(feature = "builder")]
pub use builder::MultiproofBuilder;
pub use error::{Error, Result};
pub use multiproof::{Multiproof, ValueIterator};

pub type Node = [u8; 32];
pub(crate) type Descriptor = BitVec<u32, Lsb0>;
