use std::collections::{BTreeMap, BTreeSet};

use candid::Principal;
use ic_canister_log::log;
use icrc_ledger_types::icrc3::archive::{GetArchivesArgs, GetArchivesResult};

use crate::{
    ledger_suite_manager::display_iter,
    logs::{DEBUG, INFO},
    management::{CallError, CanisterRuntime},
    state::{mutate_state, read_state, Erc20Token},
};

use futures::future;

use super::{is_recoverable, TaskError};

#[derive(Clone, PartialEq, Debug)]
pub enum DiscoverArchivesError {
    InterCanisterCallError(CallError),
}

impl DiscoverArchivesError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            DiscoverArchivesError::InterCanisterCallError(e) => is_recoverable(e),
        }
    }
}

impl From<DiscoverArchivesError> for TaskError {
    fn from(value: DiscoverArchivesError) -> Self {
        TaskError::DiscoverArchivesError(value)
    }
}

pub async fn discover_archives<R: CanisterRuntime, F: Fn(&Erc20Token) -> bool>(
    selector: F,
    runtime: &R,
) -> Result<(), DiscoverArchivesError> {
    let ledgers: BTreeMap<_, _> = read_state(|s| {
        s.all_managed_canisters_iter()
            .filter(|(token, _)| selector(token))
            .filter_map(|(token_id, canisters)| {
                canisters
                    .ledger_canister_id()
                    .cloned()
                    .map(|ledger_id| (token_id, ledger_id))
            })
            .collect()
    });
    if ledgers.is_empty() {
        return Ok(());
    }
    log!(
        INFO,
        "[discover_archives]: discovering archives for {:?}",
        ledgers
    );
    let results = future::join_all(
        ledgers
            .values()
            .map(|p| call_ledger_icrc3_get_archives(*p, runtime)),
    )
    .await;
    let mut errors: Vec<(Erc20Token, Principal, DiscoverArchivesError)> = Vec::new();
    for ((token_id, ledger), result) in ledgers.into_iter().zip(results) {
        match result {
            Ok(archives) => {
                //order is not guaranteed by the API of icrc3_get_archives.
                let archives: BTreeSet<_> = archives.into_iter().map(|a| a.canister_id).collect();
                log!(
                    DEBUG,
                    "[discover_archives]: archives for ERC-20 token {:?} with ledger {}: {}",
                    token_id,
                    ledger,
                    display_iter(&archives)
                );
                mutate_state(|s| s.record_archives(&token_id, archives.into_iter().collect()));
            }
            Err(e) => errors.push((token_id, ledger, e)),
        }
    }
    if !errors.is_empty() {
        log!(
            INFO,
            "[discover_archives]: {} errors. Failed to discover archives for {:?}",
            errors.len(),
            errors
        );
        let first_error = errors.swap_remove(0);
        return Err(first_error.2);
    }
    Ok(())
}

async fn call_ledger_icrc3_get_archives<R: CanisterRuntime>(
    ledger_id: Principal,
    runtime: &R,
) -> Result<GetArchivesResult, DiscoverArchivesError> {
    let args = GetArchivesArgs { from: None };
    runtime
        .call_canister(ledger_id, "icrc3_get_archives", args)
        .await
        .map_err(DiscoverArchivesError::InterCanisterCallError)
}

pub fn select_all<T>() -> impl Fn(&T) -> bool {
    |_| true
}

pub fn select_equal_to<T: PartialEq>(expected_value: &T) -> impl Fn(&T) -> bool + '_ {
    move |x| x == expected_value
}
