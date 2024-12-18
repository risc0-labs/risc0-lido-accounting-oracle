use alloy_primitives::B256;

/// This is a utility to allow iterating over validator balances given an iterator over validator indices and over valid gindices/node pairs
/// The reason this is needed is because 4 u64 balances are packed into a single 256 bit leaf.
pub struct ValidatorBalanceIterator<F: Fn(u64) -> u64> {
    leaves: Box<dyn Iterator<Item = (u64, B256)>>,
    validator_indices: Box<dyn Iterator<Item = u64>>,
    current_leaf: (u64, B256),
    vindex_to_gindex: F,
}

impl<F: Fn(u64) -> u64> ValidatorBalanceIterator<F> {
    pub fn new(
        validator_indices: Box<dyn Iterator<Item = u64>>,
        leaves: Box<dyn Iterator<Item = (u64, B256)>>,
        vindex_to_gindex: F,
    ) -> Self {
        Self {
            leaves,
            validator_indices,
            current_leaf: (0, B256::ZERO),
            vindex_to_gindex,
        }
    }

    pub fn take_leaves(self) -> Box<dyn Iterator<Item = (u64, B256)>> {
        self.leaves
    }
}

impl<F: Fn(u64) -> u64> Iterator for ValidatorBalanceIterator<F> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let validator_index = self.validator_indices.next()?;
        let gindex = (self.vindex_to_gindex)(validator_index);
        if self.current_leaf.0 != gindex {
            self.current_leaf = self.leaves.next()?;
        }
        assert_eq!(self.current_leaf.0, gindex);
        let balance = u64::from_le_bytes(
            self.current_leaf.1
                [(validator_index as usize % 4) * 8..(validator_index as usize % 4 + 1) * 8]
                .try_into()
                .unwrap(),
        );
        Some(balance)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ethereum_consensus::deneb::Gwei;
    use ssz_rs::prelude::*;
    use ssz_rs::List;

    #[test]
    fn test_over_list() {
        let mut balances = List::<Gwei, 10>::default();
        let validators = 0..6;
        validators.for_each(|i| balances.push(i * 10));

        let gindices = (0..6)
            .map(|vindex| List::<Gwei, 10>::generalized_index(&[(vindex as usize).into()]).unwrap())
            .collect::<Vec<_>>();

        let leaf_values = balances.multi_prove_gindices(&gindices).unwrap().0.leaves;
        let leaves = gindices.into_iter().map(|i| i as u64).zip(leaf_values);

        let vindex_to_gindex = |vindex| {
            List::<Gwei, 10>::generalized_index(&[(vindex as usize).into()]).unwrap() as u64
        };

        for b in ValidatorBalanceIterator::new(Box::new(0..6), Box::new(leaves), vindex_to_gindex) {
            println!("balance is {} ", b)
        }
    }
}
