use candid::Nat;
use ic_canister_log::log;
use ic_cdk::api::management_canister::main::{
    canister_status, CanisterIdRecord, CanisterStatusResponse,
};
use ic_cdk_macros::{init, post_upgrade, query, update};
use lsm::appic_helper_client::appic_helper_types::IcpTokenType;
use lsm::cmc_client::{CmcRunTime, CyclesConvertor};
use lsm::endpoints::{
    Erc20Contract, InstalledNativeLedgerSuite, InvalidNativeInstalledCanistersError,
    LedgerManagerInfo, ManagedCanisterIds, ManagedCanisters,
};
use lsm::ledger_suite_manager::install_ls::InstallLedgerSuiteArgs;
use lsm::ledger_suite_manager::{
    process_convert_icp_to_cycles, process_discover_archives, process_install_ledger_suites,
    process_maybe_topup,
};

use lsm::appic_helper_client::appic_helper_types::CandidIcpToken;
use lsm::lifecycle::{self, LSMarg};
use lsm::logs::INFO;
use lsm::state::{mutate_state, read_state, Canisters, Erc20Token};
use lsm::storage::read_wasm_store;
use lsm::{
    appic_helper_client::appic_helper_types::{
        CandidAddErc20TwinLedgerSuiteRequest, CandidErc20TwinLedgerSuiteFee,
        CandidErc20TwinLedgerSuiteStatus,
    },
    appic_helper_client::AppicHelperClient,
    INSTALL_LEDGER_SUITE_INTERVAL,
};
use lsm::{
    endpoints::{AddErc20Arg, AddErc20Error},
    DISCOVER_ARCHIVES_INTERVAL, ICP_TO_CYCLES_CONVERSION_INTERVAL, MAYBE_TOP_OP_INTERVAL,
};

use num_traits::ToPrimitive;

fn main() {}

#[init]
fn init(arg: LSMarg) {
    match arg {
        LSMarg::Init(init_arg) => {
            // Initialize canister state and wasm_store.
            lifecycle::init(init_arg);
        }
        LSMarg::Upgrade(_upgrade_arg) => ic_cdk::trap("Can not initialize with upgrade args."),
    }

    // Set up timers
    setup_timers();
}

#[post_upgrade]
fn post_upgrade(upgrade_args: Option<LSMarg>) {
    // Upgrade necessary parts if needed

    match upgrade_args {
        Some(LSMarg::Init(_)) => {
            ic_cdk::trap("cannot upgrade canister state with init args");
        }
        Some(LSMarg::Upgrade(upgrade_args)) => lifecycle::post_upgrade(Some(upgrade_args)),
        None => lifecycle::post_upgrade(None),
    }

    // Set up timers
    setup_timers();
}
fn setup_timers() {
    // Check ICP Balance and convert to Cycles
    ic_cdk_timers::set_timer_interval(ICP_TO_CYCLES_CONVERSION_INTERVAL, || {
        ic_cdk::spawn(process_convert_icp_to_cycles())
    });

    // Discovering Archives spawned by ledgers.
    ic_cdk_timers::set_timer_interval(DISCOVER_ARCHIVES_INTERVAL, || {
        ic_cdk::spawn(process_discover_archives())
    });

    // Check Canister balances and top-op in case of low in cycles
    ic_cdk_timers::set_timer_interval(MAYBE_TOP_OP_INTERVAL, || {
        ic_cdk::spawn(process_maybe_topup())
    });

    // Check Canister balances and top-op in case of low in cycles
    ic_cdk_timers::set_timer_interval(INSTALL_LEDGER_SUITE_INTERVAL, || {
        ic_cdk::spawn(process_install_ledger_suites())
    });
}

#[update]
async fn get_canister_status() -> CanisterStatusResponse {
    canister_status(CanisterIdRecord {
        canister_id: ic_cdk::id(),
    })
    .await
    .expect("failed to fetch canister status")
    .0
}

#[query]
fn twin_canister_ids_by_contract(contract: Erc20Contract) -> Option<ManagedCanisterIds> {
    let token_id = Erc20Token::try_from(contract)
        .unwrap_or_else(|e| ic_cdk::trap(&format!("Invalid ERC-20 contract: {:?}", e)));
    read_state(|s| s.managed_canisters(&token_id).cloned()).map(ManagedCanisterIds::from)
}

#[query]
fn all_twins_canister_ids() -> Vec<ManagedCanisters> {
    read_state(|s| {
        let managed_canisters: Vec<ManagedCanisters> = s
            .all_managed_canisters_iter()
            .map(|(token_id, canisters)| (token_id, canisters.clone()).into())
            .collect();
        return managed_canisters;
    })
}

#[query]
fn get_lsm_info() -> LedgerManagerInfo {
    read_state(|s| {
        let erc20_canisters: Vec<(Erc20Token, &Canisters)> =
            s.all_managed_canisters_iter().collect();

        // Check if paying by appic tokens is activated or not
        let ls_creation_appic_fee = match s.minimum_tokens_for_new_ledger_suite().appic {
            Some(fee) => Some(Nat::from(fee)),
            None => None,
        };

        let all_minter_ids = s.all_minter_ids();
        LedgerManagerInfo {
            managed_canisters: erc20_canisters
                .into_iter()
                .map(|(token_id, canisters)| (token_id, canisters.clone()).into())
                .collect(),
            cycles_management: s.cycles_management().clone(),
            more_controller_ids: s.more_controller_ids().to_vec(),
            minter_ids: all_minter_ids
                .into_iter()
                .map(|(chain_id, minter_id)| (Nat::from(chain_id.as_ref().clone()), minter_id))
                .collect(),
            ledger_suite_version: s.ledger_suite_version().cloned().map(|v| v.into()),
            ls_creation_icp_fee: Nat::from(s.minimum_tokens_for_new_ledger_suite().icp),

            // The feature might not be activate.
            ls_creation_appic_fee,
        }
    })
}

#[update]
async fn add_native_ls(
    native_ls: InstalledNativeLedgerSuite,
) -> Result<(), InvalidNativeInstalledCanistersError> {
    let caller = ic_cdk::caller();

    // Validating args correctness
    let validated_native_ls = read_state(|s| native_ls.validate(s))?;

    let erc20_token = validated_native_ls.get_erc20_token();

    let _minter_id = read_state(|s| {
        let minter_id = s.minter_id(erc20_token.chain_id());
        match minter_id {
            Some(minter) => {
                if minter.clone() != caller {
                    return Err(InvalidNativeInstalledCanistersError::NotAllowed);
                }
                return Ok(minter.clone());
            }
            None => return Err(InvalidNativeInstalledCanistersError::NotAllowed),
        };
    })?;

    // Add the native ledger suite to the state
    mutate_state(|s| {
        s.record_new_native_erc20_token(erc20_token.clone(), validated_native_ls.clone())
    });

    log!(
        INFO,
        "Added a native ledger_suite for {:?} with metadata: {:?}",
        erc20_token,
        validated_native_ls
    );

    // Notify Appic helper about new twin native token
    // Generate new icp token to be added to appic helper
    let icp_token = CandidIcpToken {
        fee: validated_native_ls.fee,
        decimals: validated_native_ls.decimals,
        usd_price: "0".to_string(),
        logo: validated_native_ls.logo,
        name: validated_native_ls.name,
        rank: Some(1_u32),
        ledger_id: validated_native_ls.ledger,
        token_type: IcpTokenType::Icrc2,
        symbol: validated_native_ls.symbol,
    };

    // Notify appic helper of new erc20 twins
    let helper_client = AppicHelperClient::new();

    helper_client
        .add_icp_token(icp_token)
        .await
        .map_err(|_e| InvalidNativeInstalledCanistersError::FailedToNotifyAppicHelper)?;

    let _ = helper_client.request_update_bridge_pairs().await;

    Ok(())
}

#[update]
async fn add_erc20_ls(erc20_args: AddErc20Arg) -> Result<(), AddErc20Error> {
    let caller = ic_cdk::caller();

    let time = ic_cdk::api::time();

    // Validate args correctness
    let install_ledger_suite_args = read_state(|s| {
        read_wasm_store(|w| {
            InstallLedgerSuiteArgs::validate_add_erc20(s, w, erc20_args.clone(), caller, time)
        })
    })?;

    // Get amount of ICP token required for ledger suite creation
    let twin_creation_fee_amount_in_icp =
        read_state(|s| s.minimum_tokens_for_new_ledger_suite().icp);
    // Deposit Icp or appic tokens as fee
    let cycles_client = CyclesConvertor {};

    let deposit_result = cycles_client
        .deposit_icp(
            twin_creation_fee_amount_in_icp.try_into().unwrap(),
            caller,
            None,
        )
        .await?;

    let erc20_token: Erc20Token = erc20_args
        .contract
        .try_into()
        .expect("This operation should not fail");

    match deposit_result {
        Err(transfer_error) => Err(AddErc20Error::TransferIcpError(transfer_error)),
        Ok(transfer_index) => {
            // Notify appic helper of new erc20 twins
            let helper_client = AppicHelperClient::new();

            let new_ls_args = CandidAddErc20TwinLedgerSuiteRequest {
                status: CandidErc20TwinLedgerSuiteStatus::PendingApproval,
                creator: caller,
                icp_ledger_id: None,
                icp_token_name: install_ledger_suite_args.ledger_init_arg.token_name.clone(),
                created_at: time,
                fee_charged: CandidErc20TwinLedgerSuiteFee::Icp(
                    twin_creation_fee_amount_in_icp.into(),
                ),
                icp_token_symbol: install_ledger_suite_args
                    .ledger_init_arg
                    .token_symbol
                    .clone(),
                evm_token_contract: install_ledger_suite_args.contract.address().to_string(),
                evm_token_chain_id: Nat::from(
                    install_ledger_suite_args
                        .contract
                        .chain_id()
                        .as_ref()
                        .clone(),
                ),
            };
            let _ = helper_client.new_ls_request(new_ls_args).await;

            // Add request to state
            mutate_state(|s| {
                // Record deposit into state
                s.record_new_icp_deposit(
                    erc20_token.clone(),
                    transfer_index
                        .0
                        .to_u64()
                        .expect("Nat to u64 should not fail"),
                    twin_creation_fee_amount_in_icp.checked_sub(10_000).unwrap(),
                    caller,
                );

                // Add the ledger suit creation to the queue
                s.record_new_ledger_suite_request(erc20_token, install_ledger_suite_args);

                Ok(())
            })
        }
    }?;

    Ok(())
}

// Enable Candid export
ic_cdk::export_candid!();
