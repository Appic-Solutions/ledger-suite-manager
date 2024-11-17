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
        Canister, Canisters, CanistersMetadata, Erc20Token, Hash, IndexCanister, LedgerCanister,
        ManagedCanisterStatus as StateManagedCasniter,
    },
};

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
            ledger: Some(LedgerCanister::new(StateManagedCasniter::Installed {
                canister_id: value.ledger,
                installed_wasm_hash: Hash::from_str(&value.ledger_wasm_hash)
                    .expect("Wasm Hashes Provided by minter casniter should not fail"),
            })),
            index: Some(IndexCanister::new(StateManagedCasniter::Installed {
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

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub struct ManagedCanisterIds {
    pub ledger: Option<Principal>,
    pub index: Option<Principal>,
    pub archives: Vec<Principal>,
}

impl From<Canisters> for ManagedCanisterIds {
    fn from(canisters: Canisters) -> Self {
        Self {
            ledger: canisters.ledger_canister_id().cloned(),
            index: canisters.index_canister_id().cloned(),
            archives: canisters.archive_canister_ids().to_vec(),
        }
    }
}

impl Display for ManagedCanisterIds {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedCanisterIds")
            .field(
                "ledger",
                &self
                    .ledger
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or("pending".to_string()),
            )
            .field(
                "index",
                &self
                    .index
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or("pending".to_string()),
            )
            .field(
                "archives",
                &self
                    .archives
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub enum ManagedCanisterStatus {
    Created {
        canister_id: Principal,
    },
    Installed {
        canister_id: Principal,
        installed_wasm_hash: String,
    },
}

impl<T> From<&Canister<T>> for ManagedCanisterStatus {
    fn from(canister: &Canister<T>) -> Self {
        let canister_id = *canister.canister_id();
        match canister.installed_wasm_hash() {
            None => ManagedCanisterStatus::Created { canister_id },
            Some(installed_wasm_hash) => ManagedCanisterStatus::Installed {
                canister_id,
                installed_wasm_hash: installed_wasm_hash.to_string(),
            },
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub struct ManagedCanisters {
    pub erc20_contract: Erc20Contract,
    pub twin_erc20_token_symbol: String,
    pub ledger: Option<ManagedCanisterStatus>,
    pub index: Option<ManagedCanisterStatus>,
    pub archives: Vec<Principal>,
}

impl From<(Erc20Token, Canisters)> for ManagedCanisters {
    fn from((token, canisters): (Erc20Token, Canisters)) -> Self {
        ManagedCanisters {
            erc20_contract: Erc20Contract {
                chain_id: candid::Nat::from(*token.chain_id().as_ref()),
                address: token.address().to_string(),
            },
            twin_erc20_token_symbol: canisters.metadata.token_symbol.to_string(),
            ledger: canisters.ledger.as_ref().map(ManagedCanisterStatus::from),
            index: canisters.index.as_ref().map(ManagedCanisterStatus::from),
            archives: canisters.archives.clone(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub struct LedgerSuiteVersion {
    pub ledger_compressed_wasm_hash: String,
    pub index_compressed_wasm_hash: String,
    pub archive_compressed_wasm_hash: String,
}

impl From<crate::state::LedgerSuiteVersion> for LedgerSuiteVersion {
    fn from(value: crate::state::LedgerSuiteVersion) -> Self {
        Self {
            ledger_compressed_wasm_hash: value.ledger_compressed_wasm_hash.to_string(),
            index_compressed_wasm_hash: value.index_compressed_wasm_hash.to_string(),
            archive_compressed_wasm_hash: value.archive_compressed_wasm_hash.to_string(),
        }
    }
}

type ChainId = Nat;

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub struct LedgerManagerInfo {
    pub managed_canisters: Vec<ManagedCanisters>,
    pub cycles_management: CyclesManagement,
    pub more_controller_ids: Vec<Principal>,
    pub minter_ids: Vec<(ChainId, Principal)>,
    pub ledger_suite_version: Option<LedgerSuiteVersion>,
}
