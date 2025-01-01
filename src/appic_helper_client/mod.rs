pub mod appic_helper_types;
use appic_helper_types::CandidAddErc20TwinLedgerSuiteRequest;
use appic_helper_types::CandidIcpToken;
use async_trait::async_trait;
use candid::CandidType;
use candid::Principal;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

use crate::management::CallError;
use crate::management::Reason;

pub const APPIC_HELPER_CANISTER_ID: &str = "zjydy-zyaaa-aaaaj-qnfka-cai";

#[async_trait]
pub trait Runtime {
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

#[derive(Copy, Clone)]
pub struct IcRunTime();

#[async_trait]
impl Runtime for IcRunTime {
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
        let res: Result<(O,), _> = ic_cdk::api::call::call(canister_id, method, (&args,)).await;

        match res {
            Ok((output,)) => Ok(output),
            Err((code, msg)) => Err(CallError {
                method: method.to_string(),
                reason: Reason::from_reject(code, msg),
            }),
        }
    }
}

pub struct AppicHelperClient {
    runtime: IcRunTime,
    canister_id: Principal,
}

impl AppicHelperClient {
    pub fn new() -> Self {
        Self {
            runtime: IcRunTime(),
            canister_id: Principal::from_text(APPIC_HELPER_CANISTER_ID).unwrap(),
        }
    }

    pub async fn add_icp_token(&self, token: CandidIcpToken) -> Result<(), CallError> {
        self.runtime
            .call_canister(self.canister_id, "add_icp_token", token)
            .await
    }

    pub async fn new_ls_request(
        &self,
        ls_args: CandidAddErc20TwinLedgerSuiteRequest,
    ) -> Result<(), CallError> {
        self.runtime
            .call_canister(self.canister_id, "new_twin_ls_request", ls_args)
            .await
    }

    pub async fn update_ls_request(
        &self,
        ls_args: CandidAddErc20TwinLedgerSuiteRequest,
    ) -> Result<(), CallError> {
        self.runtime
            .call_canister(self.canister_id, "update_twin_ls_request", ls_args)
            .await
    }

    pub async fn request_update_bridge_pairs(&self) -> Result<(), CallError> {
        self.runtime
            .call_canister(self.canister_id, "request_update_bridge_pairs", ())
            .await
    }
}
