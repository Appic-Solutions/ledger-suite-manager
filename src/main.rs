use std::time::Duration;

use icrc_twin_ledgers_manager::{
    tester::{checker, tester},
    DISCOVERING_ARCHIVES_INTERVAL, ICP_TO_CYCLES_CONVERTION_INTERVAL, MAYBE_TOP_OP_INTERVAL,
};

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

fn add_erc20_twin() {}
