#[cfg(test)]
pub(crate) mod test_fixtures;

#[cfg(test)]
pub mod tests;

pub mod discover_archives;
pub mod icp_cycles_convertor;
pub mod install_ls;
pub mod top_up;
use crate::cmc_client::CyclesConvertor;
use crate::ledger_suite_manager::icp_cycles_convertor::convert_icp_balance_to_cycles;
// mod upgrade_ls;
use crate::ledger_suite_manager::top_up::maybe_top_up;
use crate::logs::{DEBUG, INFO};
use candid::Principal;
use discover_archives::{discover_archives, select_all, DiscoverArchivesError};
use futures::task;
use ic_canister_log::log;
use install_ls::{install_ledger_suite, notify_erc20_added, InstallLedgerSuiteArgs};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Display};
use std::str::FromStr;
use std::time::Duration;
// use upgrade_ls::{UpgradeLedgerSuite, UpgradeLedgerSuiteError};

use crate::guard::TimerGuard;
use crate::management::{CallError, IcCanisterRuntime, Reason};
use crate::state::{
    mutate_state, read_state, Canister, Erc20Token, ManagedCanisterStatus, WasmHash,
};
use crate::storage::WasmStoreError;

// User for TimerGaurd to prevent Concurrency problems
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize, Hash, Copy)]
pub enum PeriodicTasksTypes {
    InstallLedgerSuite,
    // UpgradeLedgerSuite,
    MaybeTopUp,
    DiscoverArchives,
    ConvertIcpToCycles,
    NotifyErc20Added,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
pub enum Task {
    InstallLedgerSuite(InstallLedgerSuiteArgs),
    // UpgradeLedgerSuite(UpgradeLedgerSuite),
    MaybeTopUp,
    DiscoverArchives,
    NotifyErc20Added,
    ConvertIcpToCycles,
}

#[derive(Clone, Debug)]
pub enum TaskError {
    CanisterCreationError(CallError),
    InstallCodeError(CallError),
    CanisterStatusError(CallError),
    WasmHashNotFound(WasmHash),
    WasmStoreError(WasmStoreError),
    LedgerNotFound(Erc20Token),
    InterCanisterCallError(CallError),
    InsufficientCyclesToTopUp { required: u128, available: u128 },
    DiscoverArchivesError(DiscoverArchivesError),
    // UpgradeLedgerSuiteError(UpgradeLedgerSuiteError),
}

impl TaskError {
    /// If the error is recoverable, the task should be retried.
    /// Otherwise, the task should be discarded.
    fn is_recoverable(&self) -> bool {
        match self {
            TaskError::CanisterCreationError(_) => true,
            TaskError::InstallCodeError(_) => true,
            TaskError::CanisterStatusError(_) => true,
            TaskError::WasmHashNotFound(_) => false,
            TaskError::WasmStoreError(_) => false,
            TaskError::LedgerNotFound(_) => true, //ledger may not yet be created
            TaskError::InterCanisterCallError(e) => is_recoverable(e),
            TaskError::InsufficientCyclesToTopUp { .. } => false, //top-up task is periodic, will retry on next interval
            TaskError::DiscoverArchivesError(e) => e.is_recoverable(),
            // TaskError::UpgradeLedgerSuiteError(e) => e.is_recoverable(),
        }
    }
}

fn is_recoverable(e: &CallError) -> bool {
    match &e.reason {
        Reason::OutOfCycles => true,
        Reason::CanisterError(msg) => msg.ends_with("is stopped") || msg.ends_with("is stopping"),
        Reason::Rejected(_) => false,
        Reason::TransientInternalError(_) => true,
        Reason::InternalError(_) => false,
    }
}

fn display_iter<I: Display, T: IntoIterator<Item = I>>(v: T) -> String {
    format!(
        "[{}]",
        v.into_iter()
            .map(|x| format!("{}", x))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub async fn process_install_ledger_suites() {
    let _gaurd = match TimerGuard::new(PeriodicTasksTypes::InstallLedgerSuite) {
        Ok(gaurd) => gaurd,
        Err(e) => {
            log!(
                DEBUG,
                "Failed retrieving timer guard to install ledger suites: {e:?}",
            );
            return;
        }
    };
    let twin_ledger_suites_to_be_installed =
        read_state(|s| s.twin_ledger_suites_to_be_installed.clone());

    let runtime = IcCanisterRuntime {};

    for (contract, install_args) in twin_ledger_suites_to_be_installed {
        log!(
            INFO,
            "Installing a ledger suite for contract address: {}, chain_id:{:?}",
            contract.address(),
            contract.chain_id()
        );

        let ledger_suite_result = install_ledger_suite(&install_args, &runtime).await;
        match ledger_suite_result {
            Ok(_) => {
                mutate_state(|s| s.remove_installed_ls_from_installing_queue(contract.clone()));
                log!(
                    INFO,
                    "Installed a ledger suite for contract address: {}, chain_id:{:?}",
                    contract.address(),
                    contract.chain_id()
                );
            }

            Err(task_error) => match task_error.is_recoverable() {
                true => {
                    log!(
                        INFO,
                        "Failed to install for contract address: {}, chain_id:{:?}. Error is recoverable and will try again in the next iterationn",
                        contract.address(),
                        contract.chain_id()
                    );
                }
                false => {
                    mutate_state(|s| s.record_failed_ls_install(contract.clone(), install_args));
                    log!(
                        DEBUG,
                        "Failed to install due to {:?} for contract address: {}, chain_id:{:?}. Error is not recoverable.",
                        task_error,
                        contract.address(),
                        contract.chain_id()
                    );
                }
            },
        }
    }
}

pub async fn process_discover_archives() {
    let _gaurd = match TimerGuard::new(PeriodicTasksTypes::DiscoverArchives) {
        Ok(gaurd) => gaurd,
        Err(e) => {
            log!(
                DEBUG,
                "Failed retrieving timer guard to run discover_archives process: {e:?}",
            );
            return;
        }
    };

    let runtime = IcCanisterRuntime {};

    let archive_discovery_result = discover_archives(select_all(), &runtime).await;

    match archive_discovery_result {
        Ok(_) => {}
        Err(task_error) => match task_error.is_recoverable() {
            true => {
                log!(
                    INFO,
                    "Failed to discover archives. Error is recoverable and will try again in the next iteration");
            }
            false => {
                log!(
                    DEBUG,
                    "Failed to discover archives, Error is not recoverable. error: {:?}",
                    task_error
                );
            }
        },
    }
}

pub async fn process_notify_add_erc20() {
    let _gaurd = match TimerGuard::new(PeriodicTasksTypes::NotifyErc20Added) {
        Ok(gaurd) => gaurd,
        Err(e) => {
            log!(
                DEBUG,
                "Failed retrieving timer guard to run Notify minter process: {e:?}",
            );
            return;
        }
    };

    let runtime = IcCanisterRuntime {};

    let erc20_tokens_to_be_added_to_minters = read_state(|s| s.notify_add_erc20_list.clone());

    for (token, minter_id) in erc20_tokens_to_be_added_to_minters {
        let notify_minters_result: Result<(), TaskError> =
            notify_erc20_added(&token, &minter_id, &runtime).await;

        match notify_minters_result {
            Ok(_) => {
                mutate_state(|s| s.remove_erc20_from_minter_notification_list(&token));
                log!(INFO, "Notified minter: {}, of token:{:?}", minter_id, token);
            }
            Err(task_error) => match task_error.is_recoverable() {
                true => {
                    log!(
                    INFO,
                    "Failed to Notify minter. Error is recoverable and will try again in the next iterationn. {:?}",task_error);
                }
                false => {
                    log!(
                        DEBUG,
                        "Failed to Notify minter, Error is not recoverable. error: {:?}",
                        task_error
                    );
                }
            },
        }
    }
}

pub async fn process_maybe_topup() {
    let _gaurd = match TimerGuard::new(PeriodicTasksTypes::MaybeTopUp) {
        Ok(gaurd) => gaurd,
        Err(e) => {
            log!(
                DEBUG,
                "Failed retrieving timer guard to run maybe_top_up process suites: {e:?}",
            );
            return;
        }
    };

    let runtime = IcCanisterRuntime {};

    let top_up_result = maybe_top_up(&runtime).await;

    match top_up_result {
        Ok(_) => {}
        Err(task_error) => match task_error.is_recoverable() {
            true => {
                log!(
                    INFO,
                    "Failed to run maybe_top_up process. Error is recoverable and will try again in the next iteration");
            }
            false => {
                log!(
                    DEBUG,
                    "Failed to run maybe_top_up process, Error is not recoverable. error: {:?}",
                    task_error
                );
            }
        },
    }
}

pub async fn proccess_convert_icp_to_cycles() {
    let _gaurd = match TimerGuard::new(PeriodicTasksTypes::ConvertIcpToCycles) {
        Ok(gaurd) => gaurd,
        Err(e) => {
            log!(
                DEBUG,
                "Failed retrieving timer guard to run icpto cycles conversion process suites: {e:?}",
            );
            return;
        }
    };

    let runtime = CyclesConvertor {};

    let top_up_result = convert_icp_balance_to_cycles(runtime).await;

    match top_up_result {
        Ok(cycles) => {
            log!(INFO, "Toped_up casniter with {} cycles.", cycles);
        }
        Err(cycles_error) => {
            log!(
                INFO,
                "Failed to mint new cycles and top_up casniter. reason: {:?}",
                cycles_error
            );
        }
    }
}
