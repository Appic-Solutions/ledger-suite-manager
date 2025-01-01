use std::time::Duration;

pub mod appic_helper_client;
pub mod cmc_client;
pub mod endpoints;
pub mod guard;
pub mod ledger_suite_manager;
pub mod lifecycle;
pub mod logs;
pub mod management;
pub mod state;
pub mod storage;

pub const ICP_TO_CYCLES_CONVERSION_INTERVAL: Duration = Duration::from_secs(3 * 60 * 60);
pub const DISCOVER_ARCHIVES_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
pub const MAYBE_TOP_OP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
pub const INSTALL_LEDGER_SUITE_INTERVAL: Duration = Duration::from_secs(1 * 60);
