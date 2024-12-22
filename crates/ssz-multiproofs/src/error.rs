#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "builder")]
    #[error("Error in merklization: {0}")]
    Merklization(#[from] ssz_rs::MerkleizationError),

    #[error("No gindices provided to multiproof builder")]
    EmptyProof,

    #[error("Attempted to use an invalid gindex. Cannot be zero")]
    InvalidGeneralizedIndex,

    #[error("Attempted to verify an invalid merkle multiproof")]
    InvalidProof,
}

pub type Result<T> = std::result::Result<T, Error>;
