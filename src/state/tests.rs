mod manage_canister {
    use crate::ledger_suite_manager::test_fixtures::{usdc, usdc_metadata, usdt, usdt_metadata};
    use crate::state::test_fixtures::{expect_panic_with_message, new_state};
    use crate::state::{
        Canisters, Index, Ledger, ManageSingleCanister, ManagedCanisterStatus, WasmHash,
    };
    use candid::Principal;
    use std::fmt::Debug;

    #[test]
    fn should_record_created_canister_in_any_order() {
        let mut state = new_state();
        state.record_new_erc20_token(usdc(), usdc_metadata());
        let usdc_index_canister_id = Principal::from_slice(&[1_u8; 29]);
        state.record_created_canister::<Index>(&usdc(), usdc_index_canister_id);
        assert_eq!(
            state.managed_status::<Index>(&usdc()),
            Some(&ManagedCanisterStatus::Created {
                canister_id: usdc_index_canister_id
            })
        );
        let usdc_ledger_canister_id = Principal::from_slice(&[2_u8; 29]);
        assert_ne!(usdc_index_canister_id, usdc_ledger_canister_id);
        state.record_created_canister::<Ledger>(&usdc(), usdc_ledger_canister_id);
        assert_eq!(
            state.managed_status::<Ledger>(&usdc()),
            Some(&ManagedCanisterStatus::Created {
                canister_id: usdc_ledger_canister_id
            })
        );

        state.record_new_erc20_token(usdt(), usdt_metadata());
        let usdt_ledger_canister_id = Principal::from_slice(&[3_u8; 29]);
        state.record_created_canister::<Ledger>(&usdt(), usdt_ledger_canister_id);
        assert_eq!(
            state.managed_status::<Ledger>(&usdt()),
            Some(&ManagedCanisterStatus::Created {
                canister_id: usdt_ledger_canister_id
            })
        );
        let usdt_index_canister_id = Principal::from_slice(&[4_u8; 29]);
        state.record_created_canister::<Index>(&usdt(), usdt_index_canister_id);
        assert_eq!(
            state.managed_status::<Index>(&usdt()),
            Some(&ManagedCanisterStatus::Created {
                canister_id: usdt_index_canister_id
            })
        );
    }

    #[test]
    fn should_record_installed_canister_and_keep_correct_status() {
        fn test<C: Debug>()
        where
            Canisters: ManageSingleCanister<C>,
        {
            let mut state = new_state();
            let canister_id = Principal::from_slice(&[1_u8; 29]);
            let token_id = usdc();

            assert_eq!(state.managed_status::<C>(&token_id), None);

            state.record_new_erc20_token(token_id.clone(), usdc_metadata());
            state.record_created_canister::<C>(&token_id, canister_id);
            assert_eq!(
                state.managed_status::<C>(&token_id),
                Some(&ManagedCanisterStatus::Created { canister_id })
            );

            let wasm_hash = WasmHash::from([1_u8; 32]);
            state.record_installed_canister::<C>(&token_id, wasm_hash.clone());
            assert_eq!(
                state.managed_status::<C>(&token_id),
                Some(&ManagedCanisterStatus::Installed {
                    canister_id,
                    installed_wasm_hash: wasm_hash,
                })
            );
        }

        test::<Index>();
        test::<Ledger>();
    }

    #[test]
    fn should_panic_when_recording_created_canister_for_not_managed_erc20_token() {
        fn test<C: Debug>()
        where
            Canisters: ManageSingleCanister<C>,
        {
            let mut state = new_state();

            expect_panic_with_message(
                || state.record_created_canister::<C>(&usdc(), Principal::from_slice(&[1_u8; 29])),
                "not managed",
            );
        }

        test::<Index>();
        test::<Ledger>();
    }

    #[test]
    fn should_panic_when_recording_twice_same_new_erc20_token() {
        let mut state = new_state();
        let erc20 = usdc();
        state.record_new_erc20_token(erc20.clone(), usdc_metadata());

        expect_panic_with_message(
            || state.record_new_erc20_token(erc20, usdc_metadata()),
            "already managed",
        );
    }

    #[test]
    fn should_panic_when_recording_twice_canister_created() {
        fn test<C: Debug>()
        where
            Canisters: ManageSingleCanister<C>,
        {
            let mut state = new_state();
            let erc20 = usdc();
            state.record_new_erc20_token(erc20.clone(), usdc_metadata());
            let canister_id = Principal::from_slice(&[1_u8; 29]);
            state.record_created_canister::<C>(&erc20, canister_id);

            expect_panic_with_message(
                || state.record_created_canister::<C>(&erc20, canister_id),
                "already created",
            );
        }

        test::<Index>();
        test::<Ledger>();
    }

    #[test]
    fn should_panic_when_recording_installed_canister_but_canister_was_not_created() {
        fn test<C: Debug>()
        where
            Canisters: ManageSingleCanister<C>,
        {
            let mut state = new_state();

            expect_panic_with_message(
                || state.record_installed_canister::<C>(&usdc(), WasmHash::from([1_u8; 32])),
                "no managed canisters",
            );
        }

        test::<Index>();
        test::<Ledger>();
    }
}

mod installed_ledger_suite {
    use crate::endpoints::InstalledNativeLedgerSuite as CandidInstalledNativeLedgerSuite;
    use crate::ledger_suite_manager::test_fixtures::{
        usdc, usdc_matic, usdc_metadata, usdt, usdt_metadata,
    };
    use crate::state::test_fixtures::new_state;
    use crate::state::{Index, InvalidNativeInstalledCanistersError, Ledger, State};
    use assert_matches::assert_matches;
    use candid::{Nat, Principal};

    #[test]
    fn should_fail_when_same_wasm_hash() {
        let state = new_state();
        let mut iceth = iceth_installed_canisters();
        iceth.index_wasm_hash = iceth.ledger_wasm_hash.clone();

        let result = CandidInstalledNativeLedgerSuite::validate(iceth, &state);

        assert_matches!(
            result,
            Err(InvalidNativeInstalledCanistersError::WasmHashError)
        )
    }

    #[test]
    fn should_fail_when_token_already_managed() {
        let mut state = new_state();
        let iceth = iceth_installed_canisters();
        state.record_new_native_erc20_token(iceth.get_erc20_token(), iceth.clone());

        let result = CandidInstalledNativeLedgerSuite::validate(iceth, &state);

        assert_eq!(
            result,
            Err(InvalidNativeInstalledCanistersError::TokenAlreadyManaged)
        )
    }

    #[test]
    fn should_fail_when_principal_already_managed() {
        let mut state = new_state();
        let [usdc_index_canister_id, usdc_ledger_canister_id] = add_usdc_ledger_suite(&mut state);
        let [usdt_index_canister_id, usdt_ledger_canister_id] = add_usdt_ledger_suite(&mut state);

        for id in [
            usdc_index_canister_id,
            usdc_ledger_canister_id,
            usdt_index_canister_id,
            usdt_ledger_canister_id,
        ] {
            let mut iceth = iceth_installed_canisters();
            iceth.ledger = id;
            let result = CandidInstalledNativeLedgerSuite::validate(iceth, &state);
            assert_eq!(
                result,
                Err(InvalidNativeInstalledCanistersError::AlreadyManagedPrincipals)
            );

            let mut iceth = iceth_installed_canisters();
            iceth.index = id;
            let result = CandidInstalledNativeLedgerSuite::validate(iceth, &state);
            assert_eq!(
                result,
                Err(InvalidNativeInstalledCanistersError::AlreadyManagedPrincipals)
            );

            let mut iceth = iceth_installed_canisters();
            iceth.archives.push(id);
            let result = CandidInstalledNativeLedgerSuite::validate(iceth, &state);
            assert_eq!(
                result,
                Err(InvalidNativeInstalledCanistersError::AlreadyManagedPrincipals)
            );
        }
    }

    #[test]
    fn should_validate() {
        let mut state = new_state();
        let iceth = iceth_installed_canisters();
        let expected_iceth = validated_iceth_canisters();

        assert_eq!(
            CandidInstalledNativeLedgerSuite::validate(iceth.clone(), &state),
            Ok(expected_iceth.clone())
        );

        add_usdc_ledger_suite(&mut state);
        assert_eq!(
            CandidInstalledNativeLedgerSuite::validate(iceth.clone(), &state),
            Ok(expected_iceth.clone())
        );

        add_usdt_ledger_suite(&mut state);
        assert_eq!(
            CandidInstalledNativeLedgerSuite::validate(iceth, &state),
            Ok(expected_iceth)
        );
    }

    #[test]
    fn should_validate_native_same_contract_addresses_but_different_chain_id() {
        let mut state = new_state();

        let iceth = iceth_installed_canisters();
        let icmatic = icmatic_installed_canisters();
        state.record_new_native_erc20_token(iceth.get_erc20_token(), iceth.clone());

        let result = CandidInstalledNativeLedgerSuite::validate(icmatic.clone(), &state).unwrap();

        assert_eq!(result, icmatic)
    }

    #[test]
    fn should_validate_same_erc20_contract_addresses_but_different_chain_id() {
        let mut state = new_state();
        add_usdc_ledger_suite(&mut state);

        if let None = state.managed_canisters(&usdc_matic()) {
            assert!(true);
        } else {
            assert!(false)
        }
    }
    fn validated_iceth_canisters() -> CandidInstalledNativeLedgerSuite {
        let iceth = iceth_installed_canisters();
        CandidInstalledNativeLedgerSuite {
            decimals: iceth.decimals,
            fee: iceth.fee,
            logo: iceth.logo,
            name: iceth.name,
            symbol: iceth.symbol,
            ledger: iceth.ledger,
            ledger_wasm_hash: iceth.ledger_wasm_hash.parse().unwrap(),
            index: iceth.index,
            index_wasm_hash: iceth.index_wasm_hash.parse().unwrap(),
            archives: iceth.archives,
            chain_id: iceth.chain_id,
        }
    }

    fn iceth_installed_canisters() -> CandidInstalledNativeLedgerSuite {
        CandidInstalledNativeLedgerSuite {
            decimals: 18_u8,
            fee: Nat::from(1000000000000000_u64),
            logo: "".to_string(),
            name: "".to_string(),
            symbol: "icETH".to_string(),
            ledger: "ss2fx-dyaaa-aaaar-qacoq-cai".parse().unwrap(),

            index: "s3zol-vqaaa-aaaar-qacpa-cai".parse().unwrap(),

            archives: vec!["xob7s-iqaaa-aaaar-qacra-cai".parse().unwrap()],
            ledger_wasm_hash: "8457289d3b3179aa83977ea21bfa2fc85e402e1f64101ecb56a4b963ed33a1e6"
                .to_string(),
            index_wasm_hash: "eb3096906bf9a43996d2ca9ca9bfec333a402612f132876c8ed1b01b9844112a"
                .to_string(),
            chain_id: Nat::from(1_u64),
        }
    }

    fn icmatic_installed_canisters() -> CandidInstalledNativeLedgerSuite {
        CandidInstalledNativeLedgerSuite {
            decimals: 18_u8,
            fee: Nat::from(100000_u64),
            logo: "".to_string(),
            name: "".to_string(),
            symbol: "icMATIC".to_string(),
            ledger: "ryjl3-tyaaa-aaaaa-aaaba-cai".parse().unwrap(),

            index: "r7inp-6aaaa-aaaaa-aaabq-cai".parse().unwrap(),

            archives: vec!["renrk-eyaaa-aaaaa-aaada-cai".parse().unwrap()],
            ledger_wasm_hash: "8457289d3b3179aa83977ea21bfa2fc85e402e1f64101ecb56a4b963ed33a1e6"
                .to_string(),
            index_wasm_hash: "eb3096906bf9a43996d2ca9ca9bfec333a402612f132876c8ed1b01b9844112a"
                .to_string(),
            chain_id: Nat::from(137_u64),
        }
    }

    fn add_usdc_ledger_suite(state: &mut State) -> [Principal; 2] {
        state.record_new_erc20_token(usdc(), usdc_metadata());
        let usdc_index_canister_id = Principal::from_slice(&[1_u8; 29]);
        state.record_created_canister::<Index>(&usdc(), usdc_index_canister_id);
        let usdc_ledger_canister_id = Principal::from_slice(&[2_u8; 29]);
        state.record_created_canister::<Ledger>(&usdc(), usdc_ledger_canister_id);
        [usdc_index_canister_id, usdc_ledger_canister_id]
    }

    fn add_usdt_ledger_suite(state: &mut State) -> [Principal; 2] {
        state.record_new_erc20_token(usdt(), usdt_metadata());
        let usdt_index_canister_id = Principal::from_slice(&[3_u8; 29]);
        state.record_created_canister::<Index>(&usdt(), usdt_index_canister_id);
        let usdt_ledger_canister_id = Principal::from_slice(&[4_u8; 29]);
        state.record_created_canister::<Ledger>(&usdt(), usdt_ledger_canister_id);
        [usdt_index_canister_id, usdt_ledger_canister_id]
    }
}

mod wasm_hash {
    use crate::state::WasmHash;
    use assert_matches::assert_matches;
    use proptest::arbitrary::any;
    use proptest::array::uniform32;
    use proptest::{prop_assert_eq, proptest};
    use std::str::FromStr;

    proptest! {
        #[test]
        fn should_decode_display_string(hash in uniform32(any::<u8>())) {
            let parsed_hash = WasmHash::from_str(&WasmHash::from(hash).to_string()).unwrap();
            prop_assert_eq!(parsed_hash.as_ref(), &hash);
        }

        #[test]
        fn should_error_on_invalid_hash(invalid_hash in "[0-9a-fA-F]{0,63}|[0-9a-fA-F]{65,}") {
           assert_matches!(WasmHash::from_str(&invalid_hash), Err(_));
        }

         #[test]
        fn should_accept_valid_hash(valid_hash in "[0-9a-fA-F]{64}") {
            let result = WasmHash::from_str(&valid_hash).unwrap();
            prop_assert_eq!(result.as_ref(), &hex::decode(valid_hash).unwrap()[..]);
        }
    }
}

mod validate_config {
    use crate::endpoints::InitArg;
    use crate::state::test_fixtures::{arb_init_arg, arb_principal};
    use crate::state::{InvalidStateError, State};
    use candid::Nat;
    use proptest::collection::vec;
    use proptest::proptest;

    proptest! {
        #[test]
        fn should_accept_valid_config(init_arg in arb_init_arg(0..=9)) {
            let state = State::try_from(init_arg.clone()).expect("valid init arg");

           assert_eq!(state.more_controller_ids, init_arg.more_controller_ids);
        }

        #[test]
        fn should_error_when_too_many_additional_controllers(additional_controllers in vec(arb_principal(), 10..=100)) {
            let init_arg = InitArg {
                more_controller_ids:additional_controllers.clone(),
                minter_ids:vec![],
                cycles_management:None,
                twin_ls_creation_fee_icp_token: Nat::from(0_u64),
                twin_ls_creation_fee_appic_token: None
            };

            let result = State::try_from(init_arg);

           assert_eq!(result, Err(InvalidStateError::TooManyAdditionalControllers{max: 9, actual: additional_controllers.len()}));
        }
    }
}

mod schema_upgrades {
    use crate::endpoints::CyclesManagement;
    use crate::ledger_suite_manager::install_ls::InstallLedgerSuiteArgs;
    use crate::ledger_suite_manager::test_fixtures::{usdc, usdc_ledger_suite};
    use crate::ledger_suite_manager::PeriodicTasksTypes;
    use crate::state::test_fixtures::arb_state;
    use crate::state::{
        decode, encode, Canisters, CanistersMetadata, ChainId, Erc20Token, IndexCanister,
        LedgerCanister, LedgerSuiteCreationFee, LedgerSuiteVersion, ManagedCanisters,
        ReceivedDeposit, State,
    };
    use candid::{Deserialize, Principal};
    use proptest::proptest;
    use serde::Serialize;
    use std::collections::{BTreeMap, HashSet};

    #[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
    pub struct ManagedCanistersPreviousVersion {
        canisters: BTreeMap<Erc20Token, CanistersPreviousVersion>,
    }

    impl From<ManagedCanisters> for ManagedCanistersPreviousVersion {
        fn from(value: ManagedCanisters) -> Self {
            ManagedCanistersPreviousVersion {
                canisters: value
                    .canisters
                    .into_iter()
                    .map(|(k, v)| (k, v.into()))
                    .collect(),
            }
        }
    }

    #[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
    pub struct CanistersPreviousVersion {
        pub ledger: Option<LedgerCanister>,
        pub index: Option<IndexCanister>,
        pub archives: Vec<Principal>,
        pub metadata: CanistersMetadataPreviousVersion,
    }

    impl From<Canisters> for CanistersPreviousVersion {
        fn from(value: Canisters) -> Self {
            CanistersPreviousVersion {
                ledger: value.ledger,
                index: value.index,
                archives: value.archives,
                metadata: value.metadata.into(),
            }
        }
    }

    #[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
    pub struct CanistersMetadataPreviousVersion {
        pub token_symbol: String,
    }

    impl From<CanistersMetadata> for CanistersMetadataPreviousVersion {
        fn from(value: CanistersMetadata) -> Self {
            CanistersMetadataPreviousVersion {
                token_symbol: value.token_symbol,
            }
        }
    }

    #[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
    pub struct StatePreviousVersion {
        managed_canisters: ManagedCanistersPreviousVersion,
        cycles_management: CyclesManagement,
        more_controller_ids: Vec<Principal>,
        minter_id: BTreeMap<ChainId, Principal>,
        #[serde(default)]
        ledger_suite_version: Option<LedgerSuiteVersion>,
        twin_ledger_suites_to_be_installed: BTreeMap<Erc20Token, InstallLedgerSuiteArgs>,
        failed_ledger_suite_installs: BTreeMap<Erc20Token, InstallLedgerSuiteArgs>,
        collected_icp_token: u128,
        collected_appic_token: u128,
        minimum_tokens_for_new_ledger_suite: LedgerSuiteCreationFee,
        received_deposits: Vec<ReceivedDeposit>,
        notify_add_erc20_list: BTreeMap<Erc20Token, Principal>,
    }

    impl From<State> for StatePreviousVersion {
        fn from(
            State {
                managed_canisters,
                cycles_management,
                more_controller_ids,
                minter_id,
                ledger_suite_version,
                twin_ledger_suites_to_be_installed,
                failed_ledger_suite_installs,
                collected_icp_token,
                collected_appic_token,
                minimum_tokens_for_new_ledger_suite,
                received_deposits,
                notify_add_erc20_list,
            }: State,
        ) -> Self {
            Self {
                managed_canisters: managed_canisters.into(),
                cycles_management,
                more_controller_ids,
                minter_id,
                ledger_suite_version,
                twin_ledger_suites_to_be_installed,
                failed_ledger_suite_installs,
                collected_icp_token,
                collected_appic_token,
                minimum_tokens_for_new_ledger_suite,
                received_deposits,
                notify_add_erc20_list,
            }
        }
    }

    proptest! {
        #[test]
        fn should_be_able_to_upgrade_state(mut state in arb_state()) {
            state.managed_canisters.canisters.insert(usdc(), usdc_ledger_suite());
            let state_before_upgrade: StatePreviousVersion = state.into();

            let serialized_state_before_upgrade = encode(&state_before_upgrade);
            let state_after_upgrade: State = decode(serialized_state_before_upgrade.as_slice());

            assert_eq!(state_before_upgrade, state_after_upgrade.clone().into());
            assert_eq!(
                state_after_upgrade.ledger_suite_version,
                None
            );
        }
    }
}
