use candid::{CandidType, Deserialize, Nat, Principal};
use icrc_ledger_types::icrc2::transfer_from::TransferFromError;
use serde::Serialize;
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use crate::{
    ledger_suite_manager::install_ls::InvalidAddErc20ArgError,
    management::CallError,
    state::{
        Canisters, CanistersMetadata, Hash, IndexCanister, LedgerCanister, ManagedCanisterStatus,
        State,
    },
};

// #[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
// pub struct

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub struct AddErc20Arg {
    pub contract: Erc20Contract,
    pub ledger_init_arg: LedgerInitArg,
}

impl AddErc20Arg {
    pub fn token_name(&self) -> &str {
        &self.ledger_init_arg.token_name
    }
}

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub enum AddErc20Error {
    TransferIcpError(TransferFromError),
    InvalidErc20Contract(String),
    ChainIdNotSupported(String),
    Erc20TwinTokenAlreadyExists,
    InternalError(String),
}

impl From<InvalidAddErc20ArgError> for AddErc20Error {
    fn from(value: InvalidAddErc20ArgError) -> Self {
        match value {
            InvalidAddErc20ArgError::InvalidErc20Contract(reason) => {
                Self::InvalidErc20Contract(reason)
            }
            InvalidAddErc20ArgError::ChainIdNotSupported(reason) => {
                Self::ChainIdNotSupported(reason)
            }
            InvalidAddErc20ArgError::Erc20ContractAlreadyManaged(_) => {
                Self::Erc20TwinTokenAlreadyExists
            }
            InvalidAddErc20ArgError::WasmHashError(_) => {
                Self::InternalError("Internal Error, please try again later".to_string())
            }
            InvalidAddErc20ArgError::InternalError(_) => {
                Self::InternalError("Internal Error, please try again later".to_string())
            }
        }
    }
}

impl From<CallError> for AddErc20Error {
    fn from(value: CallError) -> Self {
        Self::InternalError("Internal Error, please try again later".to_string())
    }
}

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub struct Erc20Contract {
    pub chain_id: Nat,
    pub address: String,
}

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize, serde::Serialize)]
pub struct LedgerInitArg {
    pub transfer_fee: Nat,
    pub decimals: u8,
    pub token_name: String,
    pub token_symbol: String,
    pub token_logo: String,
}

#[derive(
    Clone, Eq, PartialEq, Ord, PartialOrd, Debug, CandidType, Deserialize, serde::Serialize,
)]

pub struct CyclesManagement {
    pub cycles_for_ledger_creation: Nat,
    pub cycles_for_archive_creation: Nat,
    pub cycles_for_index_creation: Nat,
    pub cycles_top_up_increment: Nat,
}

impl Default for CyclesManagement {
    fn default() -> Self {
        const TEN_TRILLIONS: u64 = 10_000_000_000_000;
        const HUNDRED_TRILLIONS: u64 = 100_000_000_000_000;

        Self {
            cycles_for_ledger_creation: Nat::from(2 * HUNDRED_TRILLIONS),
            cycles_for_archive_creation: Nat::from(HUNDRED_TRILLIONS),
            cycles_for_index_creation: Nat::from(HUNDRED_TRILLIONS),
            cycles_top_up_increment: Nat::from(TEN_TRILLIONS),
        }
    }
}

impl CyclesManagement {
    /// Minimum amount of cycles the orchestrator should always have and some slack.
    ///
    /// The chosen amount must ensure that the orchestrator is always able to spawn a new ICRC1 ledger suite.
    pub fn minimum_orchestrator_cycles(&self) -> Nat {
        self.cycles_for_ledger_creation.clone()
            + self.cycles_for_index_creation.clone()
            + 2_u8 * self.cycles_top_up_increment.clone()
    }

    /// Minimum amount of cycles all monitored canisters should always have and some slack.
    ///
    /// The chosen amount must ensure that the ledger should be able to spawn an archive canister at any time.
    pub fn minimum_monitored_canister_cycles(&self) -> Nat {
        self.cycles_for_archive_creation.clone() + 2_u8 * self.cycles_top_up_increment.clone()
    }
}

#[derive(Clone, PartialEq, Debug, CandidType, Deserialize)]
pub struct InstalledNativeLedgerSuite {
    pub symbol: String,
    pub ledger: Principal,
    pub ledger_wasm_hash: String,
    pub index: Principal,
    pub index_wasm_hash: String,
    pub archives: Vec<Principal>,
    pub chain_id: Nat,
}

impl From<InstalledNativeLedgerSuite> for Canisters {
    fn from(value: InstalledNativeLedgerSuite) -> Self {
        Self {
            ledger: Some(LedgerCanister::new(ManagedCanisterStatus::Installed {
                canister_id: value.ledger,
                installed_wasm_hash: Hash::from_str(&value.ledger_wasm_hash)
                    .expect("Wasm Hashes Provided by minter casniter should not fail"),
            })),
            index: Some(IndexCanister::new(ManagedCanisterStatus::Installed {
                canister_id: value.index,
                installed_wasm_hash: Hash::from_str(&value.index_wasm_hash)
                    .expect("Wasm Hashes Provided by minter casniter should not fail"),
            })),
            archives: value.archives,
            metadata: CanistersMetadata {
                token_symbol: value.symbol,
            },
        }
    }
}

#[derive(Clone, PartialEq, Debug, CandidType)]
pub enum InvalidNativeInstalledCanistersError {
    WasmHashError,
    TokenAlreadyManaged,
    AlreadyManagedPrincipals,
    // Only minter casniters are allowed to add native ledger suites
    NotAllowed,
}
