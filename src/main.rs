use std::str::FromStr;

use ic_canister_log::log;
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, query, update};
use ic_ethereum_types::Address;
use icrc_twin_ledgers_manager::cmc_client::{self, CmcRunTime, CyclesConvertor};
use icrc_twin_ledgers_manager::endpoints::{
    InstalledNativeLedgerSuite, InvalidNativeInstalledCanistersError,
};
use icrc_twin_ledgers_manager::ledger_suite_manager::install_ls::InstallLedgerSuiteArgs;
use icrc_twin_ledgers_manager::ledger_suite_manager::{
    process_discover_archives, process_install_ledger_suites,
};
use icrc_twin_ledgers_manager::logs::{DEBUG, ERROR, INFO};

use icrc_twin_ledgers_manager::state::{mutate_state, read_state, ChainId, Erc20Token};
use icrc_twin_ledgers_manager::storage::read_wasm_store;
use icrc_twin_ledgers_manager::INSTALL_LEDGER_SUITE_INTERVAL;
use icrc_twin_ledgers_manager::{
    endpoints::{AddErc20Arg, AddErc20Error},
    tester::{checker, tester},
    DISCOVER_ARCHIVES_INTERVAL, ICP_TO_CYCLES_CONVERTION_INTERVAL, MAYBE_TOP_OP_INTERVAL,
};

use num_traits::{CheckedSub, ToPrimitive};

fn main() {
    tester();
}

fn setup_timers() {
    // Check ICP Balance and convert to Cycles
    ic_cdk_timers::set_timer_interval(ICP_TO_CYCLES_CONVERTION_INTERVAL, || {
        ic_cdk::spawn(checker())
    });

    // Discovering Archives Spwaned by ledgers.
    ic_cdk_timers::set_timer_interval(DISCOVER_ARCHIVES_INTERVAL, || {
        ic_cdk::spawn(process_discover_archives())
    });

    // Check Canister balances and top-op in case of low in cycles
    ic_cdk_timers::set_timer_interval(MAYBE_TOP_OP_INTERVAL, || ic_cdk::spawn(checker()));

    // Check Canister balances and top-op in case of low in cycles
    ic_cdk_timers::set_timer_interval(INSTALL_LEDGER_SUITE_INTERVAL, || {
        ic_cdk::spawn(process_install_ledger_suites())
    });
}

#[update]
fn add_new_native_ls(
    native_ls: InstalledNativeLedgerSuite,
) -> Result<(), InvalidNativeInstalledCanistersError> {
    // Validate args corectness

    let validate_native_ls = read_state(|s| native_ls.validate(s))?;

    let erc20_token = validate_native_ls.get_erc20_token();

    // Add the native ldger suite to the state
    mutate_state(|s| s.record_new_native_erc20_token(erc20_token, validate_native_ls));

    Ok(())
}

#[update]
async fn add_new_erc20_ls(erc20_args: AddErc20Arg) -> Result<(), AddErc20Error> {
    // Validate args correctness
    let install_ledger_suite_args = read_state(|s| {
        read_wasm_store(|w| InstallLedgerSuiteArgs::validate_add_erc20(s, w, erc20_args.clone()))
    })?;

    // Get amount of icps required for ledger suite creation
    let twin_creation_fee_amount_in_icp =
        match read_state(|s| s.minimum_tokens_for_new_ledger_suite()) {
            Some(fees) => fees.icp.clone(),
            None => {
                return Err(AddErc20Error::InternalError(
                    "Failed to get twin token creation fees".to_string(),
                ))
            }
        };
    // Deposit Icp or appic tokens as fee
    let cycles_client = CyclesConvertor {};

    let deposit_result = cycles_client
        .deposit_icp(
            twin_creation_fee_amount_in_icp.try_into().unwrap(),
            ic_cdk::caller(),
            None,
        )
        .await?;

    let erc20_token: Erc20Token = erc20_args
        .contract
        .try_into()
        .expect("This opration should not fail");

    match deposit_result {
        Err(transfer_error) => return Err(AddErc20Error::TransferIcpError(transfer_error)),
        Ok(transfer_index) => {
            mutate_state(|s| {
                // Record deposit into state
                s.record_new_icp_deposit(
                    erc20_token.clone(),
                    transfer_index
                        .0
                        .to_u64()
                        .expect("Nat to u64 should not fail"),
                    twin_creation_fee_amount_in_icp.checked_sub(10_000).unwrap(),
                    ic_cdk::caller(),
                );

                // Add the leadger suit creation to the queue
                s.record_new_ledger_suite_request(erc20_token, install_ledger_suite_args);

                Ok(())
            })
        }
    }
}
