# Withdrawal Credentials Prover

This program uses recursive proving to allow efficiently updatable proofs of which beacon chain validators have their `withdrawal_credentials` set to a particular value. We refer to this as membership.

This is useful in the context of a Lido Accounting oracle as it can be consumed by other proofs which are performing aggregations over data for only Lido validators (e.g. those with `withdrawal_credentials` set to the Lido withdrawal contract).

## How this works

The `withdrawal_credentials` field of a beacon chain validator is practically immutable. Once it is set to a given Ethereum address (field begins with `0x01`) it currently cannot be altered.

> [!note]
> Historically it also supported BLS withdrawal credentials for very early beacon chain adopters (pre merge) which can be updated one time to an ethereum address withdrawal credential after which they are immutable. This is why we consider the field practically immutable and not immutable.

Furthermore the list of validators in the beacon state is append-only. So once a particular range of validator indices has been scanned and checked for a `withdrawal_credentials` match there is no need to scan it again.

Following the [Lido X](https://github.com/succinctlabs/lidox) we use proof composition to efficiently cache existing work and build an updatable proof of withdrawal credential membership. A proof of membership of validators up to index $X+k$, as of beacon state $B_m$ can be checked by verifying:

- Proof of membership of validators up to index $X$ as of $B_n$ (the recursive part)
- That $B_n$ is a prior state to $B_m$ in the chain
    - This can be done by verifying proofs into state.state_roots of $B_m$. Note this limits the gap between proofs to SLOTS_PER_HISTORICAL_ROOT (8192 for mainnet, ~27 hours) slots but this is ok as it perfectly aligns with the Lido oracle update frequency. This also requires smaller, simpler proofs and does not change throughout the history of the beacon chain (unlike `state.historical_roots` which is frozen and replaced at Capella)
- Merkle proofs for the validators with indices [X+1, X+k] in the beacon state, and proofs for their `withdrawal_credentials` and checking that `withdrawal_credentials` == `membership_credentials`

It can be seen that this is a recursive proof and so only $k$ validators need to be processed. Note that this isn't a proof of how many validators as of $B_m$ are members, only those with indices up to $X+k$. It would be trivial to also verify a proof that the number of validators as of $B_m$ is $X+k$ giving a proof of the total membership as of that state.

The output is an oracle that given a validator index can return if it is in the membership set. This can either be a sparse representation (e.g. list of validator indices in set) or a bitfield (1 bit for each validator, value is 1 if in the set), depending on how many validators are members. Since at least a u32 is required for the validator indices then a bitfield should be preferred if more than 1/32 of the validators are members. For Lido ~30% of validators are members so a bit field is definitely preferred.

