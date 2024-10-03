mod discover_archives;
mod install_ls;
mod top_op;
mod upgrade_ls;
use candid::Principal;
use discover_archives::DiscoverArchivesError;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Display};
use std::str::FromStr;
use std::time::Duration;
use upgrade_ls::UpgradeLedgerSuiteError;

use crate::management::{CallError, Reason};
use crate::state::{Canister, Erc20Token, ManagedCanisterStatus, WasmHash};
use crate::storage::WasmStoreError;

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
pub enum Task {
    InstallLedgerSuite,
    UpgradeLedgerSuite,
    MaybeTopUp,
    DiscoverArchives,
    NotifyErc20Added {
        erc20_token: Erc20Token,
        minter_id: Principal,
    },
}

impl Task {
    fn is_periodic(&self) -> bool {
        match self {
            Task::InstallLedgerSuite => false,
            Task::MaybeTopUp => true,
            Task::NotifyErc20Added { .. } => false,
            Task::DiscoverArchives => true,
            Task::UpgradeLedgerSuite => false,
        }
    }
}

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

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
pub struct TaskExecution {
    pub execute_at_ns: u64,
    pub task_type: Task,
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

fn ensure_canister_is_installed<T>(
    token_id: &Erc20Token,
    canister: Option<Canister<T>>,
) -> Result<Principal, UpgradeLedgerSuiteError> {
    match canister {
        None => Err(UpgradeLedgerSuiteError::CanisterNotReady {
            token_id: token_id.clone(),
            status: None,
            message: "canister not yet created".to_string(),
        }),
        Some(canister) => match canister.status() {
            ManagedCanisterStatus::Created { canister_id } => {
                Err(UpgradeLedgerSuiteError::CanisterNotReady {
                    token_id: token_id.clone(),
                    status: Some(ManagedCanisterStatus::Created {
                        canister_id: *canister_id,
                    }),
                    message: "canister not yet installed".to_string(),
                })
            }
            ManagedCanisterStatus::Installed {
                canister_id,
                installed_wasm_hash: _,
            } => Ok(*canister_id),
        },
    }
}
