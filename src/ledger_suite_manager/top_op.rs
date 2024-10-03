use num_traits::ToPrimitive;
use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::{
    ledger_suite_manager::{
        discover_archives::{self, select_equal_to},
        display_iter, ensure_canister_is_installed,
    },
    logs::{DEBUG, INFO},
    management::{CallError, CanisterRuntime},
    state::{read_state, Archive, Erc20Token, Index, Ledger, ManagedCanisterStatus, WasmHash},
    storage::{read_wasm_store, wasm_store_try_get, StorableWasm, WasmStoreError},
};
use candid::{Nat, Principal};
use futures::future;
use ic_canister_log::log;
use serde::{Deserialize, Serialize};

use crate::ledger_suite_manager::TaskError;

async fn maybe_top_up<R: CanisterRuntime>(runtime: &R) -> Result<(), TaskError> {
    let managed_principals: BTreeSet<_> =
        read_state(|s| s.all_managed_principals().cloned().collect());
    if managed_principals.is_empty() {
        log!(INFO, "[maybe_top_up]: No managed canisters to top-up");
        return Ok(());
    }
    let cycles_management = read_state(|s| s.cycles_management().clone());
    let minimum_orchestrator_cycles =
        cycles_to_u128(cycles_management.minimum_orchestrator_cycles());
    let minimum_monitored_canister_cycles =
        cycles_to_u128(cycles_management.minimum_monitored_canister_cycles());
    let top_up_amount = cycles_to_u128(cycles_management.cycles_top_up_increment.clone());
    log!(
        INFO,
        "[maybe_top_up]: Managed canisters {}. \
        Cycles management: {cycles_management:?}. \
    Required amount of cycles for orchestrator to be able to top-up: {minimum_orchestrator_cycles}. \
    Monitored canister minimum target cycles balance {minimum_monitored_canister_cycles}", display_iter(&managed_principals)
    );

    let mut orchestrator_cycle_balance = match runtime.canister_cycles(runtime.id()).await {
        Ok(balance) => balance,
        Err(e) => {
            log!(
                INFO,
                "[maybe_top_up] failed to get orchestrator status, with error: {:?}",
                e
            );
            return Err(TaskError::CanisterStatusError(e));
        }
    };
    if orchestrator_cycle_balance < minimum_orchestrator_cycles {
        return Err(TaskError::InsufficientCyclesToTopUp {
            required: minimum_orchestrator_cycles,
            available: orchestrator_cycle_balance,
        });
    }

    let results = future::join_all(
        managed_principals
            .iter()
            .map(|p| runtime.canister_cycles(*p)),
    )
    .await;
    assert!(!results.is_empty());

    for (canister_id, cycles_result) in managed_principals.into_iter().zip(results) {
        match cycles_result {
            Ok(balance) => {
                match (
                    balance.cmp(&minimum_monitored_canister_cycles),
                    orchestrator_cycle_balance.cmp(&minimum_orchestrator_cycles),
                ) {
                    (Ordering::Greater, _) | (Ordering::Equal, _) => {
                        log!(
                            DEBUG,
                            "[maybe_top_up] canister {canister_id} has enough cycles {balance}"
                        );
                    }
                    (_, Ordering::Less) => {
                        return Err(TaskError::InsufficientCyclesToTopUp {
                            required: minimum_orchestrator_cycles,
                            available: orchestrator_cycle_balance,
                        });
                    }
                    (Ordering::Less, Ordering::Equal) | (Ordering::Less, Ordering::Greater) => {
                        log!(
                            DEBUG,
                            "[maybe_top_up] Sending {top_up_amount} cycles to canister {canister_id} with current balance {balance}"
                        );
                        match runtime.send_cycles(canister_id, top_up_amount) {
                            Ok(()) => {
                                orchestrator_cycle_balance -= top_up_amount;
                            }
                            Err(e) => {
                                log!(
                                    INFO,
                                    "[maybe_top_up] failed to send cycles to {}, with error: {:?}",
                                    canister_id,
                                    e
                                );
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log!(
                    INFO,
                    "[maybe_top_up] failed to get canister status of {}, with error: {:?}",
                    canister_id,
                    e
                );
            }
        }
    }

    Ok(())
}

fn cycles_to_u128(cycles: Nat) -> u128 {
    cycles
        .0
        .to_u128()
        .expect("BUG: cycles does not fit in a u128")
}
