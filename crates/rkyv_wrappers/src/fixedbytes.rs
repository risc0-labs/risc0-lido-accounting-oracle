use alloy_primitives::FixedBytes;
use rkyv::{
    rancor::Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Archived, Deserialize, Place, Resolver, Serialize,
};

pub struct FixedBytesWrapper;

impl<const N: usize> ArchiveWith<FixedBytes<N>> for FixedBytesWrapper {
    type Archived = Archived<[u8; N]>;
    type Resolver = Resolver<[u8; N]>;

    fn resolve_with(field: &FixedBytes<N>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        field.0.resolve(resolver, out);
    }
}

impl<S, const N: usize> SerializeWith<FixedBytes<N>, S> for FixedBytesWrapper
where
    S: Fallible + ?Sized,
    Vec<u32>: Serialize<S>,
{
    fn serialize_with(
        field: &FixedBytes<N>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        field.0.serialize(serializer)
    }
}

impl<D, const N: usize> DeserializeWith<Archived<[u8; N]>, FixedBytes<N>, D> for FixedBytesWrapper
where
    D: Fallible + ?Sized,
    Archived<[u8; N]>: Deserialize<[u8; N], D>,
{
    fn deserialize_with(
        field: &Archived<[u8; N]>,
        deserializer: &mut D,
    ) -> Result<FixedBytes<N>, D::Error> {
        let arr: [u8; N] = field.deserialize(deserializer)?;
        Ok(arr.into())
    }
}
