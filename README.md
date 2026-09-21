# Ownable Timelock

Single-owner timelock for Soroban contracts.

## Overview

Make this contract the owner of another contract, and every privileged call to
that contract has to be scheduled on-chain and wait out a fixed delay before it
can run.

Built on the timelock and ownable modules of
[OpenZeppelin Stellar Contracts](https://github.com/OpenZeppelin/stellar-contracts/tree/v0.7.0)
v0.7.0, pinned exactly.

## What it does

The owner schedules an operation: a target contract, a function and its
arguments. Once the delay has passed, the owner executes it, and the timelock
calls the target. Until then the owner can cancel it. Nobody else can do any of
the three.

The minimum delay is set at deployment and cannot be changed. It is counted in
ledgers, not seconds.

Ownership is OpenZeppelin's `Ownable`. It can be transferred in two steps, and
it can be renounced. A renounce takes effect immediately and freezes everything,
including operations that were already scheduled or ready.

## Usage

The owned contract gates its privileged functions on its owner, which is the
timelock:

```rust
pub fn upgrade(e: &Env, new_wasm_hash: BytesN<32>) {
    let owner: Address = e.storage().instance().get(&OWNER).unwrap();
    owner.require_auth();
    e.deployer().update_current_contract_wasm(new_wasm_hash);
}
```

The timelock's owner then calls `schedule_op` with the target, `upgrade` and the
hash, and after the delay calls `execute_op` with the same arguments. The
timelock's call satisfies the target's `require_auth` on its own.

## Build and test

Requires stellar-cli 25.2.0 or newer. The OpenZeppelin crates need
`stellar contract build`; plain `cargo build` is refused.

```
make build
make test
make coverage   # needs cargo-llvm-cov
make fmt
```

## License

MIT, see [LICENSE](LICENSE).
