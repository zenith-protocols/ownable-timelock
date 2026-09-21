extern crate std;

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, ContractFunctionSet as _, Ledger as _, MockAuth, MockAuthInvoke},
    vec,
    xdr::{ScErrorCode, ScErrorType},
    Address, BytesN, Env, Error, IntoVal, InvokeError, Symbol, Val, Vec,
};
use stellar_governance::timelock::OperationState;

use crate::{OwnableTimelock, OwnableTimelockClient};

mod target {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

    #[contracttype]
    enum Key {
        Owner,
        WasmHash,
    }

    #[contract]
    pub struct Target;

    #[contractimpl]
    impl Target {
        pub fn __constructor(e: &Env, owner: Address) {
            e.storage().instance().set(&Key::Owner, &owner);
        }

        pub fn upgrade(e: &Env, new_wasm_hash: BytesN<32>) {
            let owner: Address = e.storage().instance().get(&Key::Owner).unwrap();
            owner.require_auth();
            e.storage().instance().set(&Key::WasmHash, &new_wasm_hash);
        }

        pub fn wasm_hash(e: &Env) -> Option<BytesN<32>> {
            e.storage().instance().get(&Key::WasmHash)
        }
    }
}

use target::{Target, TargetClient};

const MIN_DELAY: u32 = 100;
const START_LEDGER: u32 = 1000;

type Failure = Result<Error, InvokeError>;

const UNAUTHORIZED: Failure = Ok(Error::from_type_and_code(
    ScErrorType::Context,
    ScErrorCode::InvalidAction,
));
const OWNER_NOT_SET: Failure = Ok(Error::from_contract_error(2100));
const NOT_READY: Failure = Ok(Error::from_contract_error(4002));

/// A timelock owning a target, and one operation: `target.upgrade(hash)`.
struct Fixture<'a> {
    e: Env,
    timelock: OwnableTimelockClient<'a>,
    target: TargetClient<'a>,
    owner: Address,
    function: Symbol,
    args: Vec<Val>,
    hash: BytesN<32>,
    zero: BytesN<32>,
}

impl Fixture<'_> {
    fn new() -> Self {
        let e = Env::default();
        e.ledger().set_sequence_number(START_LEDGER);

        let owner = Address::generate(&e);
        let timelock_id = e.register(OwnableTimelock, (owner.clone(), MIN_DELAY));
        let target_id = e.register(Target, (timelock_id.clone(),));
        let hash = BytesN::from_array(&e, &[7; 32]);

        Fixture {
            timelock: OwnableTimelockClient::new(&e, &timelock_id),
            target: TargetClient::new(&e, &target_id),
            owner,
            function: symbol_short!("upgrade"),
            args: vec![&e, hash.into_val(&e)],
            zero: BytesN::from_array(&e, &[0; 32]),
            hash,
            e,
        }
    }

    /// Authorizes `signer` for exactly one timelock call and nothing beneath
    /// it.
    fn sign(&self, signer: &Address, fn_name: &str, args: Vec<Val>) {
        self.e.mock_auths(&[MockAuth {
            address: signer,
            invoke: &MockAuthInvoke {
                contract: &self.timelock.address,
                fn_name,
                args,
                sub_invokes: &[],
            },
        }]);
    }

    fn id(&self) -> BytesN<32> {
        let (t, z) = (&self.target.address, &self.zero);
        self.timelock
            .hash_operation(t, &self.function, &self.args, z, z)
    }

    fn try_schedule(&self, signer: &Address) -> Result<BytesN<32>, Failure> {
        let (t, z) = (&self.target.address, &self.zero);
        self.sign(
            signer,
            "schedule_op",
            (t, &self.function, &self.args, z, z, MIN_DELAY).into_val(&self.e),
        );
        self.timelock
            .try_schedule_op(t, &self.function, &self.args, z, z, &MIN_DELAY)
            .map(Result::unwrap)
    }

    fn try_execute(&self, signer: &Address) -> Result<(), Failure> {
        let (t, z) = (&self.target.address, &self.zero);
        self.sign(
            signer,
            "execute_op",
            (t, &self.function, &self.args, z, z).into_val(&self.e),
        );
        self.timelock
            .try_execute_op(t, &self.function, &self.args, z, z)
            .map(|_| ())
    }

    fn try_cancel(&self, signer: &Address) -> Result<(), Failure> {
        self.sign(signer, "cancel_op", (self.id(),).into_val(&self.e));
        self.timelock.try_cancel_op(&self.id()).map(Result::unwrap)
    }

    fn state(&self) -> OperationState {
        self.timelock.get_operation_state(&self.id())
    }

    fn advance(&self, ledgers: u32) {
        self.e
            .ledger()
            .set_sequence_number(self.e.ledger().sequence() + ledgers);
    }
}

#[test]
fn constructor_sets_owner_and_min_delay() {
    let f = Fixture::new();

    assert_eq!(f.timelock.get_owner(), Some(f.owner.clone()));
    assert_eq!(f.timelock.get_min_delay(), MIN_DELAY);
    assert_eq!(f.state(), OperationState::Unset);
}

#[test]
fn owner_schedules_and_executes_upgrade_after_delay() {
    let f = Fixture::new();

    assert_eq!(f.try_schedule(&f.owner), Ok(f.id()));
    assert_eq!(f.state(), OperationState::Waiting);
    assert_eq!(
        f.timelock.get_operation_ledger(&f.id()),
        START_LEDGER + MIN_DELAY
    );

    f.advance(MIN_DELAY - 1);
    assert_eq!(f.try_execute(&f.owner), Err(NOT_READY));

    f.advance(1);
    assert_eq!(f.state(), OperationState::Ready);
    assert_eq!(f.try_execute(&f.owner), Ok(()));

    assert_eq!(f.state(), OperationState::Done);
    assert_eq!(f.target.wasm_hash(), Some(f.hash.clone()));
}

#[test]
fn owner_cancels_operation() {
    let f = Fixture::new();

    f.try_schedule(&f.owner).unwrap();
    assert_eq!(f.try_cancel(&f.owner), Ok(()));
    assert_eq!(f.state(), OperationState::Unset);

    f.advance(MIN_DELAY);
    assert_eq!(f.try_execute(&f.owner), Err(NOT_READY));
}

#[test]
fn other_signer_cannot_schedule_execute_or_cancel() {
    let f = Fixture::new();
    let other = Address::generate(&f.e);

    assert_eq!(f.try_schedule(&other), Err(UNAUTHORIZED));

    f.try_schedule(&f.owner).unwrap();
    f.advance(MIN_DELAY);
    assert_eq!(f.try_execute(&other), Err(UNAUTHORIZED));
    assert_eq!(f.try_cancel(&other), Err(UNAUTHORIZED));

    assert_eq!(f.state(), OperationState::Ready);
    assert_eq!(f.target.wasm_hash(), None);
}

#[test]
fn renounce_is_immediate_and_freezes_scheduled_operations() {
    let f = Fixture::new();

    f.try_schedule(&f.owner).unwrap();
    f.advance(MIN_DELAY);

    let ledger = f.e.ledger().sequence();
    f.sign(&f.owner, "renounce_ownership", Vec::new(&f.e));
    f.timelock.renounce_ownership();

    assert_eq!(f.e.ledger().sequence(), ledger);
    assert_eq!(f.timelock.get_owner(), None);
    assert_eq!(f.try_schedule(&f.owner), Err(OWNER_NOT_SET));
    assert_eq!(f.try_execute(&f.owner), Err(OWNER_NOT_SET));
    assert_eq!(f.try_cancel(&f.owner), Err(OWNER_NOT_SET));
    assert_eq!(f.state(), OperationState::Ready);
    assert_eq!(f.target.wasm_hash(), None);
}

#[test]
#[should_panic(expected = "Error(Contract, #2101)")]
fn renounce_rejects_pending_transfer() {
    let f = Fixture::new();
    f.e.mock_all_auths();

    f.timelock
        .transfer_ownership(&Address::generate(&f.e), &(START_LEDGER + 100));
    f.timelock.renounce_ownership();
}

#[test]
fn new_owner_takes_over_after_transfer() {
    let f = Fixture::new();
    let new_owner = Address::generate(&f.e);

    f.try_schedule(&f.owner).unwrap();
    f.e.mock_all_auths();
    f.timelock
        .transfer_ownership(&new_owner, &(START_LEDGER + 100));
    f.timelock.accept_ownership();
    assert_eq!(f.timelock.get_owner(), Some(new_owner.clone()));

    f.advance(MIN_DELAY);
    assert_eq!(f.try_execute(&f.owner), Err(UNAUTHORIZED));
    assert_eq!(f.try_execute(&new_owner), Ok(()));
    assert_eq!(f.target.wasm_hash(), Some(f.hash.clone()));
}

#[test]
fn target_rejects_direct_upgrade() {
    let f = Fixture::new();

    assert_eq!(f.target.try_upgrade(&f.hash), Err(UNAUTHORIZED));
    f.e.mock_auths(&[MockAuth {
        address: &f.owner,
        invoke: &MockAuthInvoke {
            contract: &f.target.address,
            fn_name: "upgrade",
            args: (f.hash.clone(),).into_val(&f.e),
            sub_invokes: &[],
        },
    }]);
    assert_eq!(f.target.try_upgrade(&f.hash), Err(UNAUTHORIZED));
    assert_eq!(f.target.wasm_hash(), None);
}

#[test]
fn exposes_no_administrative_entrypoints() {
    let f = Fixture::new();

    f.e.as_contract(&f.timelock.address, || {
        assert!(OwnableTimelock
            .call("get_min_delay", f.e.clone(), &[])
            .is_some());
        for function in [
            "update_delay",
            "set_min_delay",
            "set_owner",
            "upgrade",
            "__check_auth",
        ] {
            assert!(OwnableTimelock.call(function, f.e.clone(), &[]).is_none());
        }
    });
}
