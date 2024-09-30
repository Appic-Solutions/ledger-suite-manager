mod install_ls;

use candid::Principal;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Display};
use std::str::FromStr;
use std::time::Duration;

use crate::management::CallError;
use crate::state::{Erc20Token, WasmHash};
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
    // DiscoverArchivesError(DiscoverArchivesError),
    // UpgradeLedgerSuiteError(UpgradeLedgerSuiteError),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
pub struct TaskExecution {
    pub execute_at_ns: u64,
    pub task_type: Task,
}
