// use crate::{
//     ledger_suite_manager::{
//         discover_archives::{self, select_equal_to},
//         display_iter,
//     },
//     logs::{DEBUG, INFO},
//     management::{CallError, CanisterRuntime},
//     state::{
//         read_state, Archive, Canister, Erc20Token, Index, Ledger, ManagedCanisterStatus, WasmHash,
//     },
//     storage::{read_wasm_store, wasm_store_try_get, StorableWasm, WasmStoreError},
// };
// use candid::Principal;
// use ic_canister_log::log;
// use serde::{Deserialize, Serialize};

// use super::discover_archives::{discover_archives, DiscoverArchivesError};

// #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
// pub struct UpgradeLedgerSuite {
//     subtasks: Vec<UpgradeLedgerSuiteSubtask>,
//     next_subtask_index: usize,
// }

// impl UpgradeLedgerSuite {
//     /// Create a new upgrade ledger suite task containing multiple subtasks
//     /// depending on which canisters need to be upgraded. Due to the dependencies between the canisters of a ledger suite, e.g.,
//     /// the index pulls transactions from the ledger, the order of the subtasks is important.
//     ///
//     /// The order of the subtasks is as follows:
//     /// 1. Upgrade the index canister
//     /// 2. Upgrade the ledger canister
//     /// 3. Fetch the list of archives from the ledger and upgrade all archive canisters
//     ///
//     /// For each canister, upgrading involves 3 (potentially failing) steps:
//     /// 1. Stop the canister
//     /// 2. Upgrade the canister
//     /// 3. Start the canister
//     ///
//     /// Note that after having upgraded the index, but before having upgraded the ledger, the upgraded index may fetch information from the not yet upgraded ledger.
//     /// However, this is deemed preferable to trying to do some kind of atomic upgrade,
//     /// where the ledger would be stopped before upgrading the index, since this would result in 2 canisters being stopped at the same time,
//     /// which could be more problematic, especially if for some unexpected reason the upgrade fails.
//     fn new(
//         token_id: Erc20Token,
//         ledger_compressed_wasm_hash: Option<WasmHash>,
//         index_compressed_wasm_hash: Option<WasmHash>,
//         archive_compressed_wasm_hash: Option<WasmHash>,
//     ) -> Self {
//         let mut subtasks = Vec::new();
//         if let Some(index_compressed_wasm_hash) = index_compressed_wasm_hash {
//             subtasks.push(UpgradeLedgerSuiteSubtask::UpgradeIndex {
//                 token_id: token_id.clone(),
//                 compressed_wasm_hash: index_compressed_wasm_hash,
//             });
//         }
//         if let Some(ledger_compressed_wasm_hash) = ledger_compressed_wasm_hash {
//             subtasks.push(UpgradeLedgerSuiteSubtask::UpgradeLedger {
//                 token_id: token_id.clone(),
//                 compressed_wasm_hash: ledger_compressed_wasm_hash,
//             });
//         }
//         if let Some(archive_compressed_wasm_hash) = archive_compressed_wasm_hash {
//             subtasks.push(UpgradeLedgerSuiteSubtask::DiscoverArchives {
//                 token_id: token_id.clone(),
//             });
//             subtasks.push(UpgradeLedgerSuiteSubtask::UpgradeArchives {
//                 token_id: token_id.clone(),
//                 compressed_wasm_hash: archive_compressed_wasm_hash,
//             });
//         }
//         Self {
//             subtasks,
//             next_subtask_index: 0,
//         }
//     }

//     fn builder(token_id: Erc20Token) -> UpgradeLedgerSuiteBuilder {
//         UpgradeLedgerSuiteBuilder::new(token_id)
//     }
// }

// #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
// pub enum UpgradeLedgerSuiteSubtask {
//     UpgradeIndex {
//         token_id: Erc20Token,
//         compressed_wasm_hash: WasmHash,
//     },
//     UpgradeLedger {
//         token_id: Erc20Token,
//         compressed_wasm_hash: WasmHash,
//     },
//     DiscoverArchives {
//         token_id: Erc20Token,
//     },
//     UpgradeArchives {
//         token_id: Erc20Token,
//         compressed_wasm_hash: WasmHash,
//     },
// }

// struct UpgradeLedgerSuiteBuilder {
//     token_id: Erc20Token,
//     ledger_wasm_hash: Option<WasmHash>,
//     index_wasm_hash: Option<WasmHash>,
//     archive_wasm_hash: Option<WasmHash>,
// }

// impl UpgradeLedgerSuiteBuilder {
//     fn new(token_id: Erc20Token) -> Self {
//         Self {
//             token_id,
//             ledger_wasm_hash: None,
//             index_wasm_hash: None,
//             archive_wasm_hash: None,
//         }
//     }

//     fn ledger_wasm_hash<T: Into<Option<WasmHash>>>(mut self, ledger_wasm_hash: T) -> Self {
//         self.ledger_wasm_hash = ledger_wasm_hash.into();
//         self
//     }

//     fn index_wasm_hash<T: Into<Option<WasmHash>>>(mut self, index_wasm_hash: T) -> Self {
//         self.index_wasm_hash = index_wasm_hash.into();
//         self
//     }

//     fn archive_wasm_hash<T: Into<Option<WasmHash>>>(mut self, archive_wasm_hash: T) -> Self {
//         self.archive_wasm_hash = archive_wasm_hash.into();
//         self
//     }

//     fn build(self) -> UpgradeLedgerSuite {
//         UpgradeLedgerSuite::new(
//             self.token_id,
//             self.ledger_wasm_hash,
//             self.index_wasm_hash,
//             self.archive_wasm_hash,
//         )
//     }
// }

// impl UpgradeLedgerSuiteSubtask {
//     pub async fn execute<R: CanisterRuntime>(
//         &self,
//         runtime: &R,
//     ) -> Result<(), UpgradeLedgerSuiteError> {
//         match self {
//             UpgradeLedgerSuiteSubtask::UpgradeIndex {
//                 token_id,
//                 compressed_wasm_hash,
//             } => {
//                 log!(
//                     INFO,
//                     "Upgrading index canister for {:?} to {}",
//                     token_id,
//                     compressed_wasm_hash
//                 );
//                 let canisters = read_state(|s| s.managed_canisters(token_id).cloned())
//                     .ok_or(UpgradeLedgerSuiteError::TokenNotFound(token_id.clone()))?;
//                 let canister_id = ensure_canister_is_installed(token_id, canisters.index)?;
//                 upgrade_canister::<Index, _>(canister_id, compressed_wasm_hash, runtime).await
//             }
//             UpgradeLedgerSuiteSubtask::UpgradeLedger {
//                 token_id,
//                 compressed_wasm_hash,
//             } => {
//                 log!(
//                     INFO,
//                     "Upgrading ledger canister for {:?} to {}",
//                     token_id,
//                     compressed_wasm_hash
//                 );
//                 let canisters = read_state(|s| s.managed_canisters(token_id).cloned())
//                     .ok_or(UpgradeLedgerSuiteError::TokenNotFound(token_id.clone()))?;
//                 let canister_id = ensure_canister_is_installed(token_id, canisters.ledger)?;
//                 upgrade_canister::<Ledger, _>(canister_id, compressed_wasm_hash, runtime).await
//             }
//             UpgradeLedgerSuiteSubtask::DiscoverArchives { token_id } => {
//                 log!(INFO, "Discovering archive canister(s) for {:?}", token_id);
//                 discover_archives(select_equal_to(token_id), runtime)
//                     .await
//                     .map_err(UpgradeLedgerSuiteError::DiscoverArchivesError)
//             }
//             UpgradeLedgerSuiteSubtask::UpgradeArchives {
//                 token_id,
//                 compressed_wasm_hash,
//             } => {
//                 let archives = read_state(|s| s.managed_canisters(token_id).cloned())
//                     .ok_or(UpgradeLedgerSuiteError::TokenNotFound(token_id.clone()))?
//                     .archives;
//                 if archives.is_empty() {
//                     log!(
//                         INFO,
//                         "No archive canisters found for {:?}. Skipping upgrade of archives.",
//                         token_id
//                     );
//                     return Ok(());
//                 }
//                 log!(
//                     INFO,
//                     "Upgrading archive canisters {} for {:?} to {}",
//                     display_iter(&archives),
//                     token_id,
//                     compressed_wasm_hash
//                 );
//                 //We expect usually 0 or 1 archive, so a simple sequential strategy is good enough.
//                 for canister_id in archives {
//                     upgrade_canister::<Archive, _>(canister_id, compressed_wasm_hash, runtime)
//                         .await?;
//                 }
//                 Ok(())
//             }
//         }
//     }
// }

// async fn upgrade_canister<T: StorableWasm, R: CanisterRuntime>(
//     canister_id: Principal,
//     wasm_hash: &WasmHash,
//     runtime: &R,
// ) -> Result<(), UpgradeLedgerSuiteError> {
//     let wasm = match read_wasm_store(|s| wasm_store_try_get::<T>(s, wasm_hash)) {
//         Ok(Some(wasm)) => Ok(wasm),
//         Ok(None) => Err(UpgradeLedgerSuiteError::WasmHashNotFound(wasm_hash.clone())),
//         Err(e) => Err(UpgradeLedgerSuiteError::WasmStoreError(e)),
//     }?;

//     log!(DEBUG, "Stopping canister {}", canister_id);
//     runtime
//         .stop_canister(canister_id)
//         .await
//         .map_err(UpgradeLedgerSuiteError::StopCanisterError)?;

//     log!(
//         DEBUG,
//         "Upgrading wasm module of canister {} to {}",
//         canister_id,
//         wasm_hash
//     );
//     runtime
//         .upgrade_canister(canister_id, wasm.to_bytes())
//         .await
//         .map_err(UpgradeLedgerSuiteError::UpgradeCanisterError)?;

//     log!(DEBUG, "Starting canister {}", canister_id);
//     runtime
//         .start_canister(canister_id)
//         .await
//         .map_err(UpgradeLedgerSuiteError::StartCanisterError)?;

//     log!(
//         DEBUG,
//         "Upgrade of canister {} to {} completed",
//         canister_id,
//         wasm_hash
//     );
//     Ok(())
// }

// #[derive(Clone, PartialEq, Debug)]
// pub enum UpgradeLedgerSuiteError {
//     TokenNotFound(Erc20Token),
//     CanisterNotReady {
//         token_id: Erc20Token,
//         status: Option<ManagedCanisterStatus>,
//         message: String,
//     },
//     StopCanisterError(CallError),
//     StartCanisterError(CallError),
//     UpgradeCanisterError(CallError),
//     WasmHashNotFound(WasmHash),
//     WasmStoreError(WasmStoreError),
//     DiscoverArchivesError(DiscoverArchivesError),
// }

// impl UpgradeLedgerSuiteError {
//     fn is_recoverable(&self) -> bool {
//         match self {
//             UpgradeLedgerSuiteError::TokenNotFound(_) => false,
//             UpgradeLedgerSuiteError::CanisterNotReady { .. } => true,
//             UpgradeLedgerSuiteError::WasmHashNotFound(_) => false,
//             UpgradeLedgerSuiteError::WasmStoreError(_) => false,
//             UpgradeLedgerSuiteError::StopCanisterError(_) => true,
//             UpgradeLedgerSuiteError::StartCanisterError(_) => true,
//             UpgradeLedgerSuiteError::UpgradeCanisterError(_) => true,
//             UpgradeLedgerSuiteError::DiscoverArchivesError(e) => e.is_recoverable(),
//         }
//     }
// }

// fn ensure_canister_is_installed<T>(
//     token_id: &Erc20Token,
//     canister: Option<Canister<T>>,
// ) -> Result<Principal, UpgradeLedgerSuiteError> {
//     match canister {
//         None => Err(UpgradeLedgerSuiteError::CanisterNotReady {
//             token_id: token_id.clone(),
//             status: None,
//             message: "canister not yet created".to_string(),
//         }),
//         Some(canister) => match canister.status() {
//             ManagedCanisterStatus::Created { canister_id } => {
//                 Err(UpgradeLedgerSuiteError::CanisterNotReady {
//                     token_id: token_id.clone(),
//                     status: Some(ManagedCanisterStatus::Created {
//                         canister_id: *canister_id,
//                     }),
//                     message: "canister not yet installed".to_string(),
//                 })
//             }
//             ManagedCanisterStatus::Installed {
//                 canister_id,
//                 installed_wasm_hash: _,
//             } => Ok(*canister_id),
//         },
//     }
// }
