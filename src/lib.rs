use std::time::Duration;

pub mod cmc_client;
pub mod endpoints;
pub mod guard;
pub mod ledger_suite_manager;
pub mod lifecycle;
pub mod logs;
pub mod management;
pub mod state;
pub mod storage;
pub mod tester;

pub const ICP_TO_CYCLES_CONVERTION_INTERVAL: Duration = Duration::from_secs(60 * 60);
pub const DISCOVER_ARCHIVES_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
pub const MAYBE_TOP_OP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
pub const INSTALL_LEDGER_SUITE_INTERVAL: Duration = Duration::from_secs(60);
pub const NOTIFY_ADD_ERC20_INTERVAL: Duration = Duration::from_secs(5 * 60);
