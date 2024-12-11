#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "builder")]
    #[error("Error in merklization: {0}")]
    Merklization(#[from] ssz_rs::MerkleizationError),

    #[error("Attempted to use an invalid gindex. Cannot be zero")]
    InvalidGeneralizedIndex,

    #[error("Failed to convert between integers: {0}")]
    IntegerConversion(#[from] std::num::TryFromIntError),

    #[error("Attempted to verify an invalid merkle multiproof")]
    InvalidProof,
}

pub type Result<T> = std::result::Result<T, Error>;
