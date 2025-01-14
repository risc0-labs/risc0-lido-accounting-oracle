use bitvec::prelude::*;
use rkyv::{
    rancor::Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Archived, Deserialize, Place, Resolver, Serialize,
};

pub struct BitVecWrapper;

impl ArchiveWith<BitVec<u32, Lsb0>> for BitVecWrapper {
    type Archived = Archived<Vec<u32>>; // Archive the underlying Vec<u32>
    type Resolver = Resolver<Vec<u32>>;

    fn resolve_with(
        field: &BitVec<u32, Lsb0>,
        resolver: Self::Resolver,
        out: Place<Self::Archived>,
    ) {
        let slice: &[u32] = field.as_raw_slice(); // Access the raw slice
        let vec =
            unsafe { Vec::from_raw_parts(slice.as_ptr() as *mut u32, slice.len(), slice.len()) };
        vec.resolve(resolver, out); // Resolve the Vec<u32>
    }
}

impl<S> SerializeWith<BitVec<u32, Lsb0>, S> for BitVecWrapper
where
    S: Fallible + ?Sized,
    Vec<u32>: Serialize<S>,
{
    fn serialize_with(
        field: &BitVec<u32, Lsb0>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        let slice: &[u32] = field.as_raw_slice(); // Access the raw slice
        let vec =
            unsafe { Vec::from_raw_parts(slice.as_ptr() as *mut u32, slice.len(), slice.len()) };
        vec.serialize(serializer) // Serialize the Vec<u32>
    }
}

impl<D> DeserializeWith<Archived<Vec<u32>>, BitVec<u32, Lsb0>, D> for BitVecWrapper
where
    D: Fallible + ?Sized,
    Archived<Vec<u32>>: Deserialize<Vec<u32>, D>,
{
    fn deserialize_with(
        field: &Archived<Vec<u32>>,
        deserializer: &mut D,
    ) -> Result<BitVec<u32, Lsb0>, D::Error> {
        let vec: Vec<u32> = field.deserialize(deserializer)?; // Deserialize the Vec<u32>
        Ok(BitVec::from_vec(vec)) // Convert the Vec<u32> back into a BitVec<u32, Lsb0>
    }
}
