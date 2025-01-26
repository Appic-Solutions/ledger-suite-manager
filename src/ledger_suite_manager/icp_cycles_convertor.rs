use crate::cmc_client::{
    CmcRunTime, CyclesConvertor, IcpToCyclesConversionError, DEFAULT_TRANSFER_FEE,
};

pub async fn convert_icp_balance_to_cycles(
    cycles_convertor: CyclesConvertor,
) -> Result<u128, IcpToCyclesConversionError> {
    let icp_balance = cycles_convertor.icp_balance().await?;

    // Fetch icp balance
    if icp_balance == 0 {
        return Err(IcpToCyclesConversionError::ZeroIcpBalance);
    }

    // Transfer available icp to Cycles minter canister
    let transfer_block_index = cycles_convertor
        .transfer_cmc(icp_balance - DEFAULT_TRANSFER_FEE.e8s())
        .await?;

    // Notify cycles minter canister to top up the canister with cycles
    let cycles_toped_up = cycles_convertor.notify_top_up(transfer_block_index).await?;

    Ok(cycles_toped_up)
}
