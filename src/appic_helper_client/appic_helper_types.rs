// This is an experimental feature to generate Rust binding from Candid.
// You may want to manually adjust some of the types.
#![allow(dead_code, unused_imports)]
use candid::{self, CandidType, Decode, Deserialize, Encode, Principal};
use ic_cdk::api::call::CallResult as Result;

#[derive(CandidType, Deserialize, Debug)]
pub enum IcpTokenType {
    #[serde(rename = "ICRC1")]
    Icrc1,
    #[serde(rename = "ICRC2")]
    Icrc2,
    #[serde(rename = "ICRC3")]
    Icrc3,
    #[serde(rename = "DIP20")]
    Dip20,
    Other(String),
}

#[derive(CandidType, Deserialize, Debug)]
pub struct CandidIcpToken {
    pub fee: candid::Nat,
    pub decimals: u8,
    pub usd_price: String,
    pub logo: String,
    pub name: String,
    pub rank: Option<u32>,
    pub ledger_id: Principal,
    pub token_type: IcpTokenType,
    pub symbol: String,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum Operator {
    AppicMinter,
    DfinityCkEthMinter,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct CandidEvmToken {
    pub decimals: u8,
    pub logo: String,
    pub name: String,
    pub erc20_contract_address: String,
    pub chain_id: candid::Nat,
    pub symbol: String,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct TokenPair {
    pub operator: Operator,
    pub evm_token: CandidEvmToken,
    pub icp_token: CandidIcpToken,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum CandidErc20TwinLedgerSuiteStatus {
    PendingApproval,
    Created,
    Installed,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum CandidErc20TwinLedgerSuiteFee {
    Icp(candid::Nat),
    Appic(candid::Nat),
}

#[derive(CandidType, Deserialize, Debug)]
pub struct CandidLedgerSuiteRequest {
    pub erc20_contract: String,
    pub status: CandidErc20TwinLedgerSuiteStatus,
    pub creator: Principal,
    pub evm_token: Option<CandidEvmToken>,
    pub created_at: u64,
    pub fee_charged: CandidErc20TwinLedgerSuiteFee,
    pub chain_id: candid::Nat,
    pub icp_token: Option<CandidIcpToken>,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct GetEvmTokenArgs {
    pub chain_id: candid::Nat,
    pub address: String,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct GetIcpTokenArgs {
    pub ledger_id: Principal,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum TransactionSearchParam {
    TxWithdrawalId(candid::Nat),
    TxMintId(candid::Nat),
    TxHash(String),
}

#[derive(CandidType, Deserialize, Debug)]
pub struct GetTxParams {
    pub chain_id: candid::Nat,
    pub search_param: TransactionSearchParam,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum EvmToIcpStatus {
    Invalid(String),
    PendingVerification,
    Minted,
    Accepted,
    Quarantined,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct CandidEvmToIcp {
    pub status: EvmToIcpStatus,
    pub principal: Principal,
    pub verified: bool,
    pub transaction_hash: String,
    pub value: candid::Nat,
    pub operator: Operator,
    pub time: u64,
    pub subaccount: Option<serde_bytes::ByteBuf>,
    pub block_number: Option<candid::Nat>,
    pub erc20_contract_address: String,
    pub actual_received: Option<candid::Nat>,
    pub ledger_mint_index: Option<candid::Nat>,
    pub chain_id: candid::Nat,
    pub from_address: String,
    pub icrc_ledger_id: Option<Principal>,
    pub total_gas_spent: Option<candid::Nat>,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum IcpToEvmStatus {
    Failed,
    SignedTransaction,
    ReplacedTransaction,
    QuarantinedReimbursement,
    PendingVerification,
    Accepted,
    Reimbursed,
    Successful,
    Created,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct CandidIcpToEvm {
    pub effective_gas_price: Option<candid::Nat>,
    pub status: IcpToEvmStatus,
    pub erc20_ledger_burn_index: Option<candid::Nat>,
    pub destination: String,
    pub verified: bool,
    pub transaction_hash: Option<String>,
    pub withdrawal_amount: candid::Nat,
    pub from: Principal,
    pub operator: Operator,
    pub time: u64,
    pub from_subaccount: Option<serde_bytes::ByteBuf>,
    pub erc20_contract_address: String,
    pub actual_received: Option<candid::Nat>,
    pub chain_id: candid::Nat,
    pub max_transaction_fee: Option<candid::Nat>,
    pub icrc_ledger_id: Option<Principal>,
    pub gas_used: Option<candid::Nat>,
    pub total_gas_spent: Option<candid::Nat>,
    pub native_ledger_burn_index: candid::Nat,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum Transaction {
    EvmToIcp(CandidEvmToIcp),
    IcpToEvm(CandidIcpToEvm),
}

#[derive(CandidType, Deserialize, Debug)]
pub struct Icrc28TrustedOriginsResponse {
    pub trusted_origins: Vec<String>,
}

#[derive(CandidType, Deserialize, Debug)]
pub struct AddEvmToIcpTx {
    pub principal: Principal,
    pub transaction_hash: String,
    pub value: candid::Nat,
    pub operator: Operator,
    pub time: candid::Nat,
    pub subaccount: Option<serde_bytes::ByteBuf>,
    pub erc20_contract_address: String,
    pub chain_id: candid::Nat,
    pub from_address: String,
    pub icrc_ledger_id: Principal,
    pub total_gas_spent: candid::Nat,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum AddEvmToIcpTxError {
    InvalidAddress,
    ChinNotSupported,
    InvalidTokenPairs,
    InvalidTokenContract,
    TxAlreadyExists,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum Result_ {
    Ok,
    Err(AddEvmToIcpTxError),
}

#[derive(CandidType, Deserialize, Debug)]
pub struct AddIcpToEvmTx {
    pub destination: String,
    pub withdrawal_amount: candid::Nat,
    pub from: Principal,
    pub operator: Operator,
    pub time: candid::Nat,
    pub from_subaccount: Option<serde_bytes::ByteBuf>,
    pub erc20_contract_address: String,
    pub chain_id: candid::Nat,
    pub max_transaction_fee: candid::Nat,
    pub icrc_ledger_id: Principal,
    pub native_ledger_burn_index: candid::Nat,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum AddIcpToEvmTxError {
    InvalidDestination,
    ChinNotSupported,
    InvalidTokenPairs,
    InvalidTokenContract,
    TxAlreadyExists,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum Result1 {
    Ok,
    Err(AddIcpToEvmTxError),
}

#[derive(CandidType, Deserialize, Debug)]
pub struct CandidAddErc20TwinLedgerSuiteRequest {
    pub status: CandidErc20TwinLedgerSuiteStatus,
    pub creator: Principal,
    pub icp_ledger_id: Option<Principal>,
    pub icp_token_name: String,
    pub created_at: u64,
    pub fee_charged: CandidErc20TwinLedgerSuiteFee,
    pub icp_token_symbol: String,
    pub evm_token_contract: String,
    pub evm_token_chain_id: candid::Nat,
}

pub struct Service(pub Principal);
impl Service {
    pub async fn add_icp_token(&self, arg0: CandidIcpToken) -> Result<()> {
        ic_cdk::call(self.0, "add_icp_token", (arg0,)).await
    }
    pub async fn get_bridge_pairs(&self) -> Result<(Vec<TokenPair>,)> {
        ic_cdk::call(self.0, "get_bridge_pairs", ()).await
    }
    pub async fn get_erc_20_twin_ls_requests_by_creator(
        &self,
        arg0: Principal,
    ) -> Result<(Vec<CandidLedgerSuiteRequest>,)> {
        ic_cdk::call(self.0, "get_erc20_twin_ls_requests_by_creator", (arg0,)).await
    }
    pub async fn get_evm_token(&self, arg0: GetEvmTokenArgs) -> Result<(Option<CandidEvmToken>,)> {
        ic_cdk::call(self.0, "get_evm_token", (arg0,)).await
    }
    pub async fn get_icp_token(&self, arg0: GetIcpTokenArgs) -> Result<(Option<CandidIcpToken>,)> {
        ic_cdk::call(self.0, "get_icp_token", (arg0,)).await
    }
    pub async fn get_icp_tokens(&self) -> Result<(Vec<CandidIcpToken>,)> {
        ic_cdk::call(self.0, "get_icp_tokens", ()).await
    }
    pub async fn get_transaction(&self, arg0: GetTxParams) -> Result<(Option<Transaction>,)> {
        ic_cdk::call(self.0, "get_transaction", (arg0,)).await
    }
    pub async fn get_txs_by_address(&self, arg0: String) -> Result<(Vec<Transaction>,)> {
        ic_cdk::call(self.0, "get_txs_by_address", (arg0,)).await
    }
    pub async fn get_txs_by_address_principal_combination(
        &self,
        arg0: String,
        arg1: Principal,
    ) -> Result<(Vec<Transaction>,)> {
        ic_cdk::call(
            self.0,
            "get_txs_by_address_principal_combination",
            (arg0, arg1),
        )
        .await
    }
    pub async fn get_txs_by_principal(&self, arg0: Principal) -> Result<(Vec<Transaction>,)> {
        ic_cdk::call(self.0, "get_txs_by_principal", (arg0,)).await
    }
    pub async fn icrc_28_trusted_origins(&self) -> Result<(Icrc28TrustedOriginsResponse,)> {
        ic_cdk::call(self.0, "icrc28_trusted_origins", ()).await
    }
    pub async fn new_evm_to_icp_tx(&self, arg0: AddEvmToIcpTx) -> Result<(Result_,)> {
        ic_cdk::call(self.0, "new_evm_to_icp_tx", (arg0,)).await
    }
    pub async fn new_icp_to_evm_tx(&self, arg0: AddIcpToEvmTx) -> Result<(Result1,)> {
        ic_cdk::call(self.0, "new_icp_to_evm_tx", (arg0,)).await
    }
    pub async fn new_twin_ls_request(
        &self,
        arg0: CandidAddErc20TwinLedgerSuiteRequest,
    ) -> Result<()> {
        ic_cdk::call(self.0, "new_twin_ls_request", (arg0,)).await
    }
    pub async fn request_update_bridge_pairs(&self) -> Result<()> {
        ic_cdk::call(self.0, "request_update_bridge_pairs", ()).await
    }
    pub async fn update_twin_ls_request(
        &self,
        arg0: CandidAddErc20TwinLedgerSuiteRequest,
    ) -> Result<()> {
        ic_cdk::call(self.0, "update_twin_ls_request", (arg0,)).await
    }
}
