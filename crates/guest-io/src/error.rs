#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "builder")]
    #[error("Error in merklization: {0}")]
    Merklization(#[from] ssz_rs::MerkleizationError),

    #[error("SSZ Multiprove error: {0}")]
    SszMultiproof(#[from] ssz_multiproofs::Error),

    #[error("Failed to convert between integers: {0}")]
    IntegerConversion(#[from] std::num::TryFromIntError),

    #[error("The fork version is not currently supported")]
    UnsupportedFork,

    #[error("Historical batch not provided but it is required for proving states are linked over the number of slots they span")]
    MissingHistoricalBatch,
}

pub type Result<T> = std::result::Result<T, Error>;
