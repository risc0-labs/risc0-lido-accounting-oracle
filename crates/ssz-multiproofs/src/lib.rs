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

pub(crate) type Node = alloy_primitives::B256;
pub(crate) type Descriptor = BitVec<u32, Lsb0>;
