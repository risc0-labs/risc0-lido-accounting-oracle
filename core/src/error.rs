#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "builder")]
    #[error("Error in merklization: {0}")]
    Merklization(#[from] ssz_rs::MerkleizationError),

    #[error("Value lookup table not build. Call build_value_lookup_table() first.")]
    ValueLookupNotBuild,

    #[error("Failed to convert between integers: {0}")]
    IntegerConversion(#[from] std::num::TryFromIntError),
}

pub type Result<T> = std::result::Result<T, Error>;
