use ic_canister_log::log;
use ic_ledger_types::{BlockIndex, TransferError};
use num_traits::ToPrimitive;

use crate::{
    cmc_client::{
        cmc_declrations::{NotifyError, NotifyTopUpResult},
        CmcRunTime, DEFAULT_TRANSFER_FEE,
    },
    logs::INFO,
    management::CallError,
};

pub async fn convert_icp_to_cycles<R>(runtime: R) -> Result<u64, IcpToCyCvleConversionError>
where
    R: CmcRunTime,
{
    let mut balance_attempts = 0;
    // Get icp balance of the canister
    let icp_balance = loop {
        match runtime.icp_balance().await {
            Ok(balance) => {
                break balance
                    .0
                    .to_u64()
                    .expect("BUG: cycles for archive creation does not fit in a u64")
            }
            Err(call_error) => {
                balance_attempts += 1;
                if balance_attempts >= 10 {
                    return Err(IcpToCyCvleConversionError::CallError(call_error));
                }
            }
        }
    };

    let mut transfer_cmc_attampts = 0;
    let block_index = loop {
        match runtime
            .transfer_cmc(
                icp_balance
                    .checked_sub(DEFAULT_TRANSFER_FEE.e8s())
                    .expect("BUG: cycles for archive creation does not fit in a u64"),
            )
            .await
        {
            Ok(transfer_result) => match transfer_result {
                Ok(block_index) => break block_index,
                Err(transfer_error) => {
                    return Err(IcpToCyCvleConversionError::ICPTransferError(transfer_error))
                }
            },
            Err(call_error) => {
                transfer_cmc_attampts += 1;
                if transfer_cmc_attampts >= 10 {
                    return Err(IcpToCyCvleConversionError::CallError(call_error));
                }
            }
        }
    };

    let mut top_up_attempts = 0;
    let cycles = loop {
        match runtime.notify_top_up(block_index).await {
            Ok(top_up_result) => match top_up_result {
                NotifyTopUpResult::Ok(cycles) => {
                    break cycles
                        .0
                        .to_u64()
                        .expect("BUG: cycles for archive creation does not fit in a u64")
                }
                NotifyTopUpResult::Err(notify_error) => {
                    return Err(IcpToCyCvleConversionError::TopUpError(notify_error))
                }
            },
            Err(call_error) => {
                top_up_attempts += 1;
                if top_up_attempts >= 10 {
                    return Err(IcpToCyCvleConversionError::CallError(call_error));
                }
            }
        }
    };
    log!(
        INFO,
        "successfully Top-uped canister with '{}' Cycles",
        cycles
    );
    Ok(cycles)
}

#[derive(Clone, PartialEq, Debug)]
pub enum IcpToCyCvleConversionError {
    CallError(CallError),
    TopUpError(NotifyError),
    ICPTransferError(TransferError),
}
