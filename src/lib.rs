//! Timelock owned by a single account, built on the OpenZeppelin Stellar
//! Contracts timelock and ownable modules.
//!
//! The owner schedules, cancels and executes. The minimum delay is fixed.
//! Renouncing ownership freezes every scheduled operation.
#![no_std]

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol, Val, Vec};
use stellar_access::ownable::{self, Ownable};
use stellar_governance::timelock::{self, Operation, OperationState};
use stellar_macros::only_owner;

#[contract]
pub struct OwnableTimelock;

#[contractimpl]
impl OwnableTimelock {
    /// Sets the owner and the minimum delay.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner account.
    /// * `min_delay` - Minimum delay in ledgers.
    pub fn __constructor(e: &Env, owner: Address, min_delay: u32) {
        ownable::set_owner(e, &owner);
        timelock::set_min_delay(e, min_delay);
    }

    /// Schedules an operation and returns its id. Owner only.
    ///
    /// # Arguments
    ///
    /// * `target` - Contract to invoke.
    /// * `function` - Function to invoke on `target`.
    /// * `args` - Arguments for `function`.
    /// * `predecessor` - Operation that must run first, or zero bytes.
    /// * `salt` - Distinguishes identical operations.
    /// * `delay` - Ledgers until ready, at least the minimum delay.
    ///
    /// # Errors
    ///
    /// * refer to [`timelock::schedule_operation`] errors.
    #[only_owner]
    pub fn schedule_op(
        e: &Env,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
        delay: u32,
    ) -> BytesN<32> {
        let operation = Operation {
            target,
            function,
            args,
            predecessor,
            salt,
        };
        timelock::schedule_operation(e, &operation, delay)
    }

    /// Executes a ready operation and returns the target's result. Owner only.
    ///
    /// # Errors
    ///
    /// * refer to [`timelock::execute_operation`] errors.
    #[only_owner]
    pub fn execute_op(
        e: &Env,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> Val {
        let operation = Operation {
            target,
            function,
            args,
            predecessor,
            salt,
        };
        timelock::execute_operation(e, &operation)
    }

    /// Cancels a waiting or ready operation. Owner only.
    ///
    /// # Errors
    ///
    /// * refer to [`timelock::cancel_operation`] errors.
    #[only_owner]
    pub fn cancel_op(e: &Env, operation_id: BytesN<32>) {
        timelock::cancel_operation(e, &operation_id);
    }

    /// Returns the minimum delay in ledgers.
    pub fn get_min_delay(e: &Env) -> u32 {
        timelock::get_min_delay(e)
    }

    /// Returns the id of an operation.
    pub fn hash_operation(
        e: &Env,
        target: Address,
        function: Symbol,
        args: Vec<Val>,
        predecessor: BytesN<32>,
        salt: BytesN<32>,
    ) -> BytesN<32> {
        let operation = Operation {
            target,
            function,
            args,
            predecessor,
            salt,
        };
        timelock::hash_operation(e, &operation)
    }

    /// Returns the state of an operation.
    pub fn get_operation_state(e: &Env, operation_id: BytesN<32>) -> OperationState {
        timelock::get_operation_state(e, &operation_id)
    }

    /// Returns the ready ledger of an operation, `0` if unset, `1` if done.
    pub fn get_operation_ledger(e: &Env, operation_id: BytesN<32>) -> u32 {
        timelock::get_operation_ledger(e, &operation_id)
    }
}

#[contractimpl(contracttrait)]
impl Ownable for OwnableTimelock {}

#[cfg(test)]
mod test;
