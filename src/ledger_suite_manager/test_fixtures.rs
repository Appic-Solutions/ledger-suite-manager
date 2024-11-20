use crate::state::{
    Canisters, CanistersMetadata, Erc20Token, IndexCanister, LedgerCanister, ManagedCanisterStatus,
};

pub const USDC_ADDRESS: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
pub const USDT_ADDRESS: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";

pub fn usdc() -> Erc20Token {
    crate::endpoints::Erc20Contract {
        chain_id: 1_u8.into(),
        address: USDC_ADDRESS.to_string(),
    }
    .try_into()
    .unwrap()
}

pub fn usdc_matic() -> Erc20Token {
    crate::endpoints::Erc20Contract {
        chain_id: 137_u8.into(),
        address: USDC_ADDRESS.to_string(),
    }
    .try_into()
    .unwrap()
}

pub fn usdc_metadata() -> CanistersMetadata {
    CanistersMetadata {
        token_symbol: "icUSDC".to_string(),
    }
}

pub fn usdc_ledger_suite() -> Canisters {
    Canisters {
        ledger: Some(LedgerCanister::new(ManagedCanisterStatus::Installed {
            canister_id: "xevnm-gaaaa-aaaar-qafnq-cai".parse().unwrap(),
            installed_wasm_hash: "8457289d3b3179aa83977ea21bfa2fc85e402e1f64101ecb56a4b963ed33a1e6"
                .parse()
                .unwrap(),
        })),
        index: Some(IndexCanister::new(ManagedCanisterStatus::Installed {
            canister_id: "xrs4b-hiaaa-aaaar-qafoa-cai".parse().unwrap(),
            installed_wasm_hash: "eb3096906bf9a43996d2ca9ca9bfec333a402612f132876c8ed1b01b9844112a"
                .parse()
                .unwrap(),
        })),
        archives: vec!["t4dy3-uiaaa-aaaar-qafua-cai".parse().unwrap()],
        metadata: usdc_metadata(),
    }
}

pub fn usdt() -> Erc20Token {
    crate::endpoints::Erc20Contract {
        chain_id: 1_u8.into(),
        address: USDT_ADDRESS.to_string(),
    }
    .try_into()
    .unwrap()
}

pub fn usdt_metadata() -> CanistersMetadata {
    CanistersMetadata {
        token_symbol: "icUSDT".to_string(),
    }
}
