use crate::endpoints::CyclesManagement;
use crate::logs::INFO;
use crate::management::CanisterRuntime;
use crate::state::{read_state, ManageSingleCanister, ManagedCanisterStatus};
use crate::storage::{read_wasm_store, wasm_store_try_get, StorableWasm};
use crate::{
    endpoints::{AddErc20Arg, LedgerInitArg},
    state::{
        mutate_state, Canisters, CanistersMetadata, Erc20Token, Index, Ledger, LedgerSuiteVersion,
        State, WasmHash,
    },
    storage::{wasm_store_contain, WasmHashError, WasmStore},
};
use candid::{CandidType, Encode, Nat, Principal};
use ic_base_types::PrincipalId;
use ic_canister_log::log;
use ic_icrc1_index_ng::{IndexArg, InitArg as IndexInitArg};
use ic_icrc1_ledger::{ArchiveOptions, InitArgs as LedgerInitArgs, LedgerArgument};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::Debug;

use super::{Task, TaskError};

const THREE_GIGA_BYTES: u64 = 3_221_225_472;

#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct InstallLedgerSuiteArgs {
    contract: Erc20Token,
    minter_id: Principal,
    ledger_init_arg: LedgerInitArg,
    ledger_compressed_wasm_hash: WasmHash,
    index_compressed_wasm_hash: WasmHash,
}
impl PartialOrd for InstallLedgerSuiteArgs {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InstallLedgerSuiteArgs {
    fn cmp(&self, other: &Self) -> Ordering {
        self.contract.cmp(&other.contract)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum InvalidAddErc20ArgError {
    InvalidErc20Contract(String),
    ChainIdNotSupported(String),
    Erc20ContractAlreadyManaged(Erc20Token),
    WasmHashError(WasmHashError),
    InternalError(String),
}

impl InstallLedgerSuiteArgs {
    pub fn validate_add_erc20(
        state: &State,
        wasm_store: &WasmStore,
        args: AddErc20Arg,
    ) -> Result<InstallLedgerSuiteArgs, InvalidAddErc20ArgError> {
        let token = Erc20Token::try_from(args.contract.clone())
            .map_err(|e| InvalidAddErc20ArgError::InvalidErc20Contract(e.to_string()))?;

        // Check if the chain is supported by checking the minter id
        let minter_id = state.minter_id(token.chain_id()).cloned().ok_or(
            InvalidAddErc20ArgError::ChainIdNotSupported(
                "ERROR: Target evm chain is not yet supported".to_string(),
            ),
        )?;
        if let Some(_canisters) = state.managed_canisters(&token) {
            return Err(InvalidAddErc20ArgError::Erc20ContractAlreadyManaged(token));
        }
        let (ledger_compressed_wasm_hash, index_compressed_wasm_hash) = {
            let LedgerSuiteVersion {
                ledger_compressed_wasm_hash,
                index_compressed_wasm_hash,
                archive_compressed_wasm_hash: _,
            } = state
                .ledger_suite_version()
                .expect("ERROR: ledger suite version missing");
            //TODO XC-138: move read method to state and ensure that hash is in store and remove this.
            assert!(
                //nothing can be changed in AddErc20Arg to fix this.
                wasm_store_contain::<Ledger>(wasm_store, ledger_compressed_wasm_hash),
                "BUG: ledger compressed wasm hash missing"
            );
            assert!(
                //nothing can be changed in AddErc20Arg to fix this.
                wasm_store_contain::<Index>(wasm_store, index_compressed_wasm_hash),
                "BUG: index compressed wasm hash missing"
            );
            (
                ledger_compressed_wasm_hash.clone(),
                index_compressed_wasm_hash.clone(),
            )
        };
        Ok(Self {
            contract: token,
            minter_id,
            ledger_init_arg: args.ledger_init_arg,
            ledger_compressed_wasm_hash,
            index_compressed_wasm_hash,
        })
    }
}

pub async fn install_ledger_suite<R: CanisterRuntime>(
    args: &InstallLedgerSuiteArgs,
    runtime: &R,
) -> Result<(), TaskError> {
    record_new_erc20_token_once(
        args.contract.clone(),
        CanistersMetadata {
            token_symbol: args.ledger_init_arg.token_symbol.clone(),
        },
    );
    let CyclesManagement {
        cycles_for_ledger_creation,
        cycles_for_index_creation,
        cycles_for_archive_creation,
        ..
    } = read_state(|s| s.cycles_management().clone());
    let ledger_canister_id =
        create_canister_once::<Ledger, _>(&args.contract, runtime, cycles_for_ledger_creation)
            .await?;

    let more_controllers = read_state(|s| s.more_controller_ids().to_vec())
        .into_iter()
        .map(PrincipalId)
        .collect();
    install_canister_once::<Ledger, _, _>(
        &args.contract,
        &args.ledger_compressed_wasm_hash,
        &LedgerArgument::Init(icrc1_ledger_init_arg(
            args.minter_id,
            args.ledger_init_arg.clone(),
            runtime.id().into(),
            more_controllers,
            cycles_for_archive_creation,
        )),
        runtime,
    )
    .await?;

    let _index_principal =
        create_canister_once::<Index, _>(&args.contract, runtime, cycles_for_index_creation)
            .await?;
    let index_arg = Some(IndexArg::Init(IndexInitArg {
        ledger_id: ledger_canister_id,
        retrieve_blocks_from_ledger_interval_seconds: None,
    }));
    install_canister_once::<Index, _, _>(
        &args.contract,
        &args.index_compressed_wasm_hash,
        &index_arg,
        runtime,
    )
    .await?;

    // TODO: Schedule notyfing minter for new erc token
    // read_state(|s| {
    //     if let Some(&minter_id) = s.minter_id(args.contract.chain_id()) {
    //         // schedule_now(
    //         //     Task::NotifyErc20Added {
    //         //         erc20_token,
    //         //         minter_id,
    //         //     },
    //         //     runtime,
    //         // );
    //     }
    // });
    Ok(())
}

fn record_new_erc20_token_once(token: Erc20Token, metadata: CanistersMetadata) {
    mutate_state(|s| {
        if s.managed_canisters(&token).is_some() {
            return;
        }
        s.record_new_erc20_token(token, metadata);
    });
}

fn icrc1_ledger_init_arg(
    minter_id: Principal,
    ledger_init_arg: LedgerInitArg,
    archive_controller_id: PrincipalId,
    archive_more_controller_ids: Vec<PrincipalId>,
    cycles_for_archive_creation: Nat,
) -> LedgerInitArgs {
    use ic_icrc1_ledger::FeatureFlags as LedgerFeatureFlags;
    use icrc_ledger_types::icrc::generic_metadata_value::MetadataValue as LedgerMetadataValue;
    use icrc_ledger_types::icrc1::account::Account as LedgerAccount;

    const LEDGER_FEE_SUBACCOUNT: [u8; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0x0f, 0xee,
    ];
    const MAX_MEMO_LENGTH: u16 = 80;
    const ICRC2_FEATURE: LedgerFeatureFlags = LedgerFeatureFlags { icrc2: true };

    LedgerInitArgs {
        minting_account: LedgerAccount::from(minter_id),
        fee_collector_account: Some(LedgerAccount {
            owner: minter_id,
            subaccount: Some(LEDGER_FEE_SUBACCOUNT),
        }),
        initial_balances: vec![],
        transfer_fee: ledger_init_arg.transfer_fee,
        decimals: Some(ledger_init_arg.decimals),
        token_name: ledger_init_arg.token_name,
        token_symbol: ledger_init_arg.token_symbol,
        metadata: vec![(
            "icrc1:logo".to_string(),
            LedgerMetadataValue::from(ledger_init_arg.token_logo),
        )],
        archive_options: icrc1_archive_options(
            archive_controller_id,
            archive_more_controller_ids,
            cycles_for_archive_creation,
        ),
        max_memo_length: Some(MAX_MEMO_LENGTH),
        feature_flags: Some(ICRC2_FEATURE),
        maximum_number_of_accounts: None,
        accounts_overflow_trim_quantity: None,
    }
}

fn icrc1_archive_options(
    archive_controller_id: PrincipalId,
    archive_more_controller_ids: Vec<PrincipalId>,
    cycles_for_archive_creation: Nat,
) -> ArchiveOptions {
    ArchiveOptions {
        trigger_threshold: 2_000,
        num_blocks_to_archive: 1_000,
        node_max_memory_size_bytes: Some(THREE_GIGA_BYTES),
        max_message_size_bytes: None,
        controller_id: archive_controller_id,
        more_controller_ids: Some(archive_more_controller_ids),
        cycles_for_archive_creation: Some(
            cycles_for_archive_creation
                .0
                .to_u64()
                .expect("BUG: cycles for archive creation does not fit in a u64"),
        ),
        max_transactions_per_response: None,
    }
}

async fn create_canister_once<C, R>(
    token: &Erc20Token,
    runtime: &R,
    cycles_for_canister_creation: Nat,
) -> Result<Principal, TaskError>
where
    C: Debug,
    Canisters: ManageSingleCanister<C>,
    R: CanisterRuntime,
{
    if let Some(canister_id) = read_state(|s| {
        s.managed_status::<C>(&token)
            .map(ManagedCanisterStatus::canister_id)
            .cloned()
    }) {
        return Ok(canister_id);
    }
    let canister_id = match runtime
        .create_canister(
            vec![runtime.id()],
            cycles_for_canister_creation
                .0
                .to_u64()
                .expect("BUG: cycles for canister creation does not fit in a u64"),
        )
        .await
    {
        Ok(id) => {
            log!(
                INFO,
                "created {} canister for {:?} at '{}'",
                Canisters::display_name(),
                token,
                id
            );
            id
        }
        Err(e) => {
            log!(
                INFO,
                "failed to create {} canister for {:?}: {}",
                Canisters::display_name(),
                token,
                e
            );
            return Err(TaskError::CanisterCreationError(e));
        }
    };
    mutate_state(|s| s.record_created_canister::<C>(token, canister_id));
    Ok(canister_id)
}

async fn install_canister_once<C, R, I>(
    token: &Erc20Token,
    wasm_hash: &WasmHash,
    init_args: &I,
    runtime: &R,
) -> Result<(), TaskError>
where
    C: Debug + StorableWasm + Send,
    Canisters: ManageSingleCanister<C>,
    R: CanisterRuntime,
    I: Debug + CandidType,
{
    let canister_id = match read_state(|s| s.managed_status::<C>(&token).cloned()) {
        None => {
            panic!(
                "BUG: {} canister is not yet created",
                Canisters::display_name()
            )
        }
        Some(ManagedCanisterStatus::Created { canister_id }) => canister_id,
        Some(ManagedCanisterStatus::Installed { .. }) => return Ok(()),
    };

    let wasm = match read_wasm_store(|s| wasm_store_try_get::<C>(s, wasm_hash)) {
        Ok(Some(wasm)) => Ok(wasm),
        Ok(None) => {
            log!(
                INFO,
                "ERROR: failed to install {} canister for {:?} at '{}': wasm hash {} not found",
                Canisters::display_name(),
                token,
                canister_id,
                wasm_hash
            );
            Err(TaskError::WasmHashNotFound(wasm_hash.clone()))
        }
        Err(e) => {
            log!(
                INFO,
                "ERROR: failed to install {} canister for {:?} at '{}': {:?}",
                Canisters::display_name(),
                token,
                canister_id,
                e
            );
            Err(TaskError::WasmStoreError(e))
        }
    }?;

    match runtime
        .install_code(
            canister_id,
            wasm.to_bytes(),
            Encode!(init_args).expect("BUG: failed to encode init arg"),
        )
        .await
    {
        Ok(_) => {
            log!(
                INFO,
                "successfully installed {} canister for {:?} at '{}' with init args {:?}",
                Canisters::display_name(),
                token,
                canister_id,
                init_args
            );
        }
        Err(e) => {
            log!(
                INFO,
                "failed to install {} canister for {:?} at '{}' with init args {:?}: {}",
                Canisters::display_name(),
                token,
                canister_id,
                init_args,
                e
            );
            return Err(TaskError::InstallCodeError(e));
        }
    };

    mutate_state(|s| s.record_installed_canister::<C>(token, wasm_hash.clone()));

    Ok(())
}

// Type for adding Erc20 to minter
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub struct AddErc20Token {
    pub chain_id: Nat,
    pub address: String,
    pub erc20_token_symbol: String,
    pub erc20_ledger_id: Principal,
}

async fn notify_erc20_added<R: CanisterRuntime>(
    token: &Erc20Token,
    minter_id: &Principal,
    runtime: &R,
) -> Result<(), TaskError> {
    let managed_canisters = read_state(|s| s.managed_canisters(&token).cloned());
    match managed_canisters {
        Some(Canisters {
            ledger: Some(ledger),
            metadata,
            ..
        }) => {
            let args = AddErc20Token {
                chain_id: Nat::from(*token.chain_id().as_ref()),
                address: token.address().to_string(),
                erc20_token_symbol: metadata.token_symbol,
                erc20_ledger_id: *ledger.canister_id(),
            };
            runtime
                .call_canister(*minter_id, "add_erc20_token", args)
                .await
                .map_err(TaskError::InterCanisterCallError)
        }
        _ => Err(TaskError::LedgerNotFound(token.clone())),
    }
}
