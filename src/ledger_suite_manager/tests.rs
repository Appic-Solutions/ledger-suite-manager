use crate::endpoints::{AddErc20Arg, CyclesManagement, InitArg, LedgerInitArg};
use crate::ledger_suite_manager::test_fixtures::{usdc, usdc_metadata};
use crate::ledger_suite_manager::tests::mock::MockCanisterRuntime;
use crate::ledger_suite_manager::{InstallLedgerSuiteArgs, Task, TaskError};
use crate::management::{CallError, CanisterRuntime, Reason};
use crate::state::test_fixtures::new_state;
use crate::state::{
    read_state, Canisters, IndexCanister, LedgerCanister, LedgerSuiteVersion,
    ManagedCanisterStatus, State, WasmHash,
};
use crate::storage::{mutate_wasm_store, record_icrc1_ledger_suite_wasms};
use crate::storage::{ARCHIVE_NODE_BYTECODE, INDEX_BYTECODE, LEDGER_BYTECODE};
use candid::Principal;
use icrc_ledger_types::icrc3::archive::{GetArchivesArgs, GetArchivesResult};

const ORCHESTRATOR_PRINCIPAL: Principal = Principal::from_slice(&[0_u8; 29]);
const LEDGER_PRINCIPAL: Principal = Principal::from_slice(&[1_u8; 29]);
const INDEX_PRINCIPAL: Principal = Principal::from_slice(&[2_u8; 29]);
const MINTER_PRINCIPAL: Principal = Principal::from_slice(&[3_u8; 29]);

fn init_state() {
    crate::state::init_state(new_state());
    let _version = register_embedded_wasms();
}

fn register_embedded_wasms() -> LedgerSuiteVersion {
    mutate_wasm_store(|s| record_icrc1_ledger_suite_wasms(s, 1_620_328_630_000_000_000)).unwrap()
}

fn usdc_install_args() -> InstallLedgerSuiteArgs {
    InstallLedgerSuiteArgs {
        contract: usdc(),
        minter_id: MINTER_PRINCIPAL,
        ledger_init_arg: ledger_init_arg(),
        ledger_compressed_wasm_hash: read_ledger_wasm_hash(),
        index_compressed_wasm_hash: read_index_wasm_hash(),
    }
}

fn ledger_init_arg() -> LedgerInitArg {
    LedgerInitArg {
        transfer_fee: 10_000_u32.into(),
        decimals: 6,
        token_name: "Ethereum Twin USDC".to_string(),
        token_symbol: "icUSDC".to_string(),
        token_logo: "".to_string(),
    }
}

fn read_index_wasm_hash() -> WasmHash {
    WasmHash::from(ic_crypto_sha2::Sha256::hash(INDEX_BYTECODE))
}

fn read_ledger_wasm_hash() -> WasmHash {
    WasmHash::from(ic_crypto_sha2::Sha256::hash(LEDGER_BYTECODE))
}

fn read_archive_wasm_hash() -> WasmHash {
    WasmHash::from(ic_crypto_sha2::Sha256::hash(ARCHIVE_NODE_BYTECODE))
}

mod mock {
    use crate::ledger_suite_manager::CallError;
    use crate::management::CanisterRuntime;
    use async_trait::async_trait;
    use candid::CandidType;
    use candid::Principal;
    use core::fmt::Debug;
    use mockall::mock;
    use serde::de::DeserializeOwned;
    use std::marker::Send;

    mock! {
        pub CanisterRuntime{}

        #[async_trait]
        impl CanisterRuntime for CanisterRuntime {

            fn id(&self) -> Principal;

            fn time(&self) -> u64;

            fn global_timer_set(&self, timestamp: u64);

            async fn create_canister(
                &self,
                controllers: Vec<Principal>,
                cycles_for_canister_creation: u64,
            ) -> Result<Principal, CallError>;

            async fn stop_canister(&self, canister_id: Principal) -> Result<(), CallError>;

            async fn start_canister(&self, canister_id: Principal) -> Result<(), CallError>;

            async fn install_code(
                &self,
                canister_id: Principal,
                wasm_module:Vec<u8>,
                arg: Vec<u8>,
            ) -> Result<(), CallError>;

            async fn upgrade_canister(
                &self,
                canister_id: Principal,
                wasm_module:Vec<u8>,
            ) -> Result<(), CallError>;

            async fn canister_cycles(
                &self,
                canister_id: Principal,
            ) -> Result<u128, CallError>;

            fn send_cycles(
                &self,
                canister_id: Principal,
                cycles: u128
            ) -> Result<(), CallError>;

            async fn call_canister<I, O>(
                &self,
                canister_id: Principal,
                method: &str,
                args: I,
            ) -> Result<O, CallError>
            where
                I: CandidType + Debug + Send + 'static,
                O: CandidType + DeserializeOwned + Debug + 'static;
        }
    }
}

mod install_ledger_suite_args {
    use crate::endpoints::{AddErc20Arg, InitArg, LedgerInitArg};
    use crate::ledger_suite_manager::tests::{usdc_metadata, MINTER_PRINCIPAL};
    use crate::ledger_suite_manager::{
        install_ls::InvalidAddErc20ArgError, Erc20Token, InstallLedgerSuiteArgs,
    };
    use crate::state::test_fixtures::{expect_panic_with_message, new_state, new_state_from};
    use crate::state::{ChainId, IndexWasm, LedgerSuiteVersion, LedgerWasm, WasmHash};
    use crate::storage::test_fixtures::{embedded_ledger_suite_version, empty_wasm_store};
    use crate::storage::{record_icrc1_ledger_suite_wasms, WasmStore};
    use assert_matches::assert_matches;
    use candid::Nat;
    use proptest::proptest;

    const ERC20_CONTRACT_ADDRESS: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

    #[test]
    fn should_error_if_minter_id_missing() {
        let state = new_state();
        let wasm_store = wasm_store_with_icrc1_ledger_suite();

        assert_matches!(
            InstallLedgerSuiteArgs::validate_add_erc20(&state, &wasm_store, valid_add_erc20_arg()),
            Err(InvalidAddErc20ArgError::ChainIdNotSupported( error )) if error.contains("ERROR: Target evm chain is not yet supported")
        );
    }

    #[test]
    fn should_error_if_contract_is_already_managed() {
        let mut state = new_state_from(InitArg {
            minter_ids: vec![(Nat::from(1_u64), MINTER_PRINCIPAL)],
            ..Default::default()
        });
        let wasm_store = wasm_store_with_icrc1_ledger_suite();
        state.update_ledger_suite_version(embedded_ledger_suite_version());
        let arg = valid_add_erc20_arg();
        let contract: Erc20Token = arg.contract.clone().try_into().unwrap();
        state.record_new_erc20_token(contract.clone(), usdc_metadata());

        assert_eq!(
            InstallLedgerSuiteArgs::validate_add_erc20(&state, &wasm_store, arg),
            Err(InvalidAddErc20ArgError::Erc20ContractAlreadyManaged(
                contract
            ))
        );
    }

    proptest! {

        #[test]
        fn should_error_on_invalid_ethereum_address(invalid_address in "0x[0-9a-fA-F]{0,39}|[0-9a-fA-F]{41,}") {
            let mut state = new_state();
            let wasm_store = wasm_store_with_icrc1_ledger_suite();
            state.update_ledger_suite_version(embedded_ledger_suite_version());
            let mut arg = valid_add_erc20_arg();
            arg.contract.address = invalid_address;
            assert_matches!(
                InstallLedgerSuiteArgs::validate_add_erc20(&state, &wasm_store, arg),
                Err(InvalidAddErc20ArgError::InvalidErc20Contract(_))
            );
        }

        #[test]
        fn should_error_on_large_chain_id(offset in 0_u128..=u64::MAX as u128) {
            let mut state = new_state();
            let wasm_store = wasm_store_with_icrc1_ledger_suite();
            state.update_ledger_suite_version(embedded_ledger_suite_version());
            let mut arg = valid_add_erc20_arg();
            arg.contract.chain_id = Nat::from((u64::MAX as u128) + offset);

            assert_matches!(
                InstallLedgerSuiteArgs::validate_add_erc20(&state, &wasm_store, arg),
                Err(InvalidAddErc20ArgError::InvalidErc20Contract(_))
            );
        }
    }

    #[test]
    fn should_panic_when_ledger_suite_version_missing() {
        let state = new_state_from(InitArg {
            minter_ids: vec![(Nat::from(1_u64), MINTER_PRINCIPAL)],
            ..Default::default()
        });
        let wasm_store = wasm_store_with_icrc1_ledger_suite();
        assert_eq!(state.ledger_suite_version(), None);

        expect_panic_with_message(
            || {
                InstallLedgerSuiteArgs::validate_add_erc20(
                    &state,
                    &wasm_store,
                    valid_add_erc20_arg(),
                )
            },
            "ledger suite version missing",
        );
    }

    #[test]
    fn should_panic_when_ledger_suite_version_not_in_wasm_store() {
        for version in [
            LedgerSuiteVersion {
                ledger_compressed_wasm_hash: WasmHash::default(),
                ..embedded_ledger_suite_version()
            },
            LedgerSuiteVersion {
                index_compressed_wasm_hash: WasmHash::default(),
                ..embedded_ledger_suite_version()
            },
        ] {
            let mut state = new_state_from(InitArg {
                minter_ids: vec![(Nat::from(1_u64), MINTER_PRINCIPAL)],
                ..Default::default()
            });
            state.update_ledger_suite_version(version);
            let wasm_store = wasm_store_with_icrc1_ledger_suite();

            expect_panic_with_message(
                || {
                    InstallLedgerSuiteArgs::validate_add_erc20(
                        &state,
                        &wasm_store,
                        valid_add_erc20_arg(),
                    )
                },
                "wasm hash missing",
            );
        }
    }

    #[test]
    fn should_accept_valid_erc20_arg() {
        let mut state = new_state_from(InitArg {
            minter_ids: vec![(Nat::from(1_u64), MINTER_PRINCIPAL)],
            ..Default::default()
        });
        let wasm_store = wasm_store_with_icrc1_ledger_suite();
        state.update_ledger_suite_version(embedded_ledger_suite_version());
        let arg = valid_add_erc20_arg();
        let ledger_init_arg = arg.ledger_init_arg.clone();

        let result = InstallLedgerSuiteArgs::validate_add_erc20(&state, &wasm_store, arg).unwrap();

        assert_eq!(
            result,
            InstallLedgerSuiteArgs {
                contract: Erc20Token::new(
                    ChainId::from(Nat::from(1_u64)),
                    ERC20_CONTRACT_ADDRESS.parse().unwrap()
                ),
                minter_id: MINTER_PRINCIPAL,
                ledger_init_arg,
                ledger_compressed_wasm_hash: LedgerWasm::from(crate::storage::LEDGER_BYTECODE)
                    .hash()
                    .clone(),
                index_compressed_wasm_hash: IndexWasm::from(crate::storage::INDEX_BYTECODE)
                    .hash()
                    .clone(),
            }
        );
    }

    fn valid_add_erc20_arg() -> AddErc20Arg {
        AddErc20Arg {
            contract: crate::endpoints::Erc20Contract {
                chain_id: Nat::from(1_u8),
                address: ERC20_CONTRACT_ADDRESS.to_string(),
            },
            ledger_init_arg: LedgerInitArg {
                transfer_fee: 10_000_u32.into(),
                decimals: 6,
                token_name: "USD Coin".to_string(),
                token_symbol: "USDC".to_string(),
                token_logo: "".to_string(),
            },
        }
    }

    fn wasm_store_with_icrc1_ledger_suite() -> WasmStore {
        let mut store = empty_wasm_store();
        assert_eq!(
            record_icrc1_ledger_suite_wasms(&mut store, 1_620_328_630_000_000_000,),
            Ok(embedded_ledger_suite_version())
        );
        store
    }
}
