use ic_canister_log::log;
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, query, update};
use icrc_twin_ledgers_manager::cmc_client::{self, CmcRunTime, CyclesConvertor};
use icrc_twin_ledgers_manager::ledger_suite_manager::install_ls::InstallLedgerSuiteArgs;
use icrc_twin_ledgers_manager::logs::{DEBUG, ERROR, INFO};

use icrc_twin_ledgers_manager::state::{mutate_state, read_state};
use icrc_twin_ledgers_manager::storage::read_wasm_store;
use icrc_twin_ledgers_manager::{
    endpoints::{AddErc20Arg, AddErc20Error},
    tester::{checker, tester},
    DISCOVERING_ARCHIVES_INTERVAL, ICP_TO_CYCLES_CONVERTION_INTERVAL, MAYBE_TOP_OP_INTERVAL,
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
    ic_cdk_timers::set_timer_interval(DISCOVERING_ARCHIVES_INTERVAL, || ic_cdk::spawn(checker()));

    // Check Canister balances and top-op in case of low in cycles
    ic_cdk_timers::set_timer_interval(MAYBE_TOP_OP_INTERVAL, || ic_cdk::spawn(checker()));
}

#[update]
async fn create_new_erc20_twin(erc20_args: AddErc20Arg) -> Result<(), AddErc20Error> {
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
    match deposit_result {
        Err(transfer_error) => return Err(AddErc20Error::TransferIcpError(transfer_error)),
        Ok(transfer_index) => {
            mutate_state(|s| {
                // Record deposit into state
                s.record_new_icp_deposit(
                    erc20_args
                        .contract
                        .try_into()
                        .expect("This opration should not fail"),
                    transfer_index
                        .0
                        .to_u64()
                        .expect("Nato to u64 should not fail"),
                    twin_creation_fee_amount_in_icp.checked_sub(10_000).unwrap(),
                    ic_cdk::caller(),
                );

                // Add the leadger suit creation to the queue
                s.record_new_ledger_suite_request(install_ledger_suite_args);

                Ok(())
            })
        }
    }
}
