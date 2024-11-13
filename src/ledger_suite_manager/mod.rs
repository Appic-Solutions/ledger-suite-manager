pub mod discover_archives;
pub mod icp_cycles_convertor;
pub mod install_ls;
pub mod top_up;
// mod upgrade_ls;
use candid::Principal;
use discover_archives::DiscoverArchivesError;
use install_ls::InstallLedgerSuiteArgs;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Display};
use std::str::FromStr;
use std::time::Duration;
// use upgrade_ls::{UpgradeLedgerSuite, UpgradeLedgerSuiteError};

use crate::management::{CallError, Reason};
use crate::state::{Canister, Erc20Token, ManagedCanisterStatus, WasmHash};
use crate::storage::WasmStoreError;

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
pub enum Task {
    InstallLedgerSuite(InstallLedgerSuiteArgs),
    // UpgradeLedgerSuite(UpgradeLedgerSuite),
    MaybeTopUp,
    DiscoverArchives,
    NotifyErc20Added {
        erc20_token: Erc20Token,
        minter_id: Principal,
    },
    ConvertIcpToCycles,
}

impl Task {
    fn is_periodic(&self) -> bool {
        match self {
            Task::InstallLedgerSuite { .. } => false,
            Task::MaybeTopUp => true,
            Task::NotifyErc20Added { .. } => false,
            Task::DiscoverArchives => true,
            // Task::UpgradeLedgerSuite => false,
            Task::ConvertIcpToCycles => true,
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
