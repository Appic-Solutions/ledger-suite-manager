pub mod cmc_declrations;

use async_trait::async_trait;
use candid::{CandidType, Nat, Principal};
use num_traits::ToPrimitive;

use ic_canister_log::log;
use icrc_ledger_types::{
    icrc1::account::Account,
    icrc2::transfer_from::{TransferFromArgs, TransferFromError},
};

use ic_ledger_types::{
    AccountIdentifier as IcpAccountIdentifier, Memo as IcpMemo, Subaccount as IcpSubaccount,
    Tokens, TransferArgs as IcpTransferArgs, TransferError, DEFAULT_FEE,
};

type BlockIndex = u64;
type Cycles = u128;

use cmc_declrations::{NotifyError, NotifyTopUpArg, NotifyTopUpResult};
use serde::de::DeserializeOwned;
use std::fmt::Debug;

use crate::{
    logs::{DEBUG, INFO},
    management::{CallError, Reason},
};

pub const MEMO_TOP_UP_CANISTER: IcpMemo = IcpMemo(0x50555054); // == 'TPUP'

pub const MAINNET_CYCLE_MINTER_CANISTER_ID: Principal =
    Principal::from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x01, 0x01]);

pub const DEFAULT_TRANSFER_FEE: Tokens = Tokens::from_e8s(10_000);

pub const MAINNET_LEDGER_CANISTER_ID: Principal =
    Principal::from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x01, 0x01]);

const TRANSFER_METHOD: &str = "transfer";
const NOTIFY_TOP_UP_METHOD: &str = "notify_top_up";
const TRANSFER_FROM_METHOD: &str = "icrc2_transfer_from";
const ICP_BALANCE_FUNCTION: &str = "icrc1_balance_of";
#[async_trait]

pub trait CmcRunTime {
    fn id(&self) -> Principal;

    // ICP balance of canister
    async fn icp_balance(&self) -> Result<u64, IcpToCyclesConvertionError>;

    // Transfers icp to cycles minter canister
    async fn transfer_cmc(&self, icp_amount: u64)
        -> Result<BlockIndex, IcpToCyclesConvertionError>;

    // calls notify_top_op function of cyclesminter canister to convert icp into cycles
    async fn notify_top_up(&self, block_index: u64) -> Result<Cycles, IcpToCyclesConvertionError>;

    // Uses icrc2_transfer_from function to deposit functoin
    async fn deposit_icp(
        &self,
        icp_amount: u64,
        from: Principal,
        from_subaccount: Option<[u8; 32]>,
    ) -> Result<Result<Nat, TransferFromError>, CallError>;

    // Making inter canister calls
    async fn call_canister<I, O>(
        &self,
        canister_id: Principal,
        method: &str,
        args: I,
    ) -> Result<O, CallError>
    where
        I: CandidType + Debug + Send + 'static,
        O: CandidType + DeserializeOwned + Debug + 'static;
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum IcpToCyclesConvertionError {
    CallError(CallError),
    TransferError(TransferError),
    NotifyError(NotifyError),
    ZeroIcpBalance,
}

impl From<CallError> for IcpToCyclesConvertionError {
    fn from(value: CallError) -> Self {
        Self::CallError(value)
    }
}

pub struct CyclesConvertor {}

#[async_trait]
impl CmcRunTime for CyclesConvertor {
    fn id(&self) -> Principal {
        ic_cdk::id()
    }

    async fn icp_balance(&self) -> Result<u64, IcpToCyclesConvertionError> {
        let result: Nat = self
            .call_canister(
                MAINNET_LEDGER_CANISTER_ID,
                ICP_BALANCE_FUNCTION,
                Account {
                    owner: self.id(),
                    subaccount: None,
                },
            )
            .await?;
        Ok(result.0.to_u64().unwrap())
    }

    async fn transfer_cmc(
        &self,
        icp_amount: u64,
    ) -> Result<BlockIndex, IcpToCyclesConvertionError> {
        let target_subaccount = IcpSubaccount::from(self.id());

        let transfer_args = IcpTransferArgs {
            memo: MEMO_TOP_UP_CANISTER,
            amount: Tokens::from_e8s(icp_amount),
            fee: DEFAULT_TRANSFER_FEE,
            from_subaccount: None,
            to: IcpAccountIdentifier::new(&self.id(), &target_subaccount),
            created_at_time: None,
        };
        // Transfering icp into cycles minting canister
        let result: Result<u64, ic_ledger_types::TransferError> = self
            .call_canister(MAINNET_LEDGER_CANISTER_ID, TRANSFER_METHOD, transfer_args)
            .await?;

        match result {
            Ok(block_index) => Ok(block_index),
            Err(error) => Err(IcpToCyclesConvertionError::TransferError(error)),
        }
    }

    async fn notify_top_up(&self, block_index: u64) -> Result<Cycles, IcpToCyclesConvertionError> {
        let top_up_args = NotifyTopUpArg {
            canister_id: self.id(),
            block_index,
        };

        let result: NotifyTopUpResult = self
            .call_canister(
                MAINNET_CYCLE_MINTER_CANISTER_ID,
                NOTIFY_TOP_UP_METHOD,
                top_up_args,
            )
            .await?;
        match result {
            NotifyTopUpResult::Ok(cycles) => Ok(cycles.0.to_u128().unwrap()),
            NotifyTopUpResult::Err(notify_error) => {
                Err(IcpToCyclesConvertionError::NotifyError(notify_error))
            }
        }
    }

    async fn deposit_icp(
        &self,
        icp_amount: u64,
        from: Principal,
        from_subaccount: Option<[u8; 32]>,
    ) -> Result<Result<Nat, TransferFromError>, CallError> {
        let transfer_from_args = TransferFromArgs {
            spender_subaccount: None,
            from: Account {
                owner: from,
                subaccount: from_subaccount,
            },
            to: Account {
                owner: self.id(),
                subaccount: None,
            },
            amount: icp_amount
                .checked_sub(DEFAULT_FEE.e8s())
                .unwrap_or_else(|| {
                    log!(
                        INFO,
                        "Subtracting Tokens '{}' - '{}' failed because the underlying u64 underflowed",
                        icp_amount,
                        DEFAULT_FEE.e8s()
                    );

                    panic!(
                        "Subtracting Tokens {} - {} failed because the underlying u64 underflowed",
                        icp_amount,
                        DEFAULT_FEE.e8s()
                    )
                })
                .into(),
            fee: Some(DEFAULT_TRANSFER_FEE.e8s().into()),
            memo: None,
            created_at_time:Some(ic_cdk::api::time()),
        };

        let result = self
            .call_canister(
                MAINNET_LEDGER_CANISTER_ID,
                TRANSFER_FROM_METHOD,
                transfer_from_args,
            )
            .await;
        result
    }

    async fn call_canister<I, O>(
        &self,
        canister_id: Principal,
        method: &str,
        args: I,
    ) -> Result<O, CallError>
    where
        I: CandidType + Debug + Send + 'static,
        O: CandidType + DeserializeOwned + Debug + 'static,
    {
        log!(
            DEBUG,
            "Calling canister '{}' with method '{}' and payload '{:?}'",
            canister_id,
            method,
            args
        );
        let res: Result<(O,), _> = ic_cdk::api::call::call(canister_id, method, (&args,)).await;
        log!(
            DEBUG,
            "Result of calling canister '{}' with method '{}' and payload '{:?}': {:?}",
            canister_id,
            method,
            args,
            res
        );

        match res {
            Ok((output,)) => Ok(output),
            Err((code, msg)) => Err(CallError {
                method: method.to_string(),
                reason: Reason::from_reject(code, msg),
            }),
        }
    }
}
