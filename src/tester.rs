use candid::Principal;
use num_traits::ToPrimitive;

use crate::{
    state::{ArchiveWasm, IndexWasm, LedgerSuiteVersion, LedgerWasm},
    storage::WasmStoreError,
};

// Wasm converted to byte code
pub(crate) const LEDGER_BYTECODE: &[u8] = include_bytes!("../wasm/ledger_canister_u256.wasm.gz");
pub(crate) const INDEX_BYTECODE: &[u8] = include_bytes!("../wasm/index_ng_canister_u256.wasm.gz");
pub(crate) const ARCHIVE_NODE_BYTECODE: &[u8] =
    include_bytes!("../wasm/archive_canister_u256.wasm.gz");
const LEDGER_FEE_SUBACCOUNT: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0f,
    0xee,
];

/// Id of the ledger canister on the IC.
pub const MAINNET_LEDGER_CANISTER_ID: Principal =
    Principal::from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x01, 0x01]);

pub const MAINNET_CYCLE_MINTER_CANISTER_ID: Principal =
    Principal::from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x01, 0x01]);

pub fn tester() {
    println!("{}", MAINNET_CYCLE_MINTER_CANISTER_ID.to_string());
    println!("{}", MAINNET_LEDGER_CANISTER_ID.to_string())
}
pub async fn checker() {
    // let ledger_compressed_wasm_hash = LedgerWasm::from(LEDGER_BYTECODE).hash().clone().to_string();
    // let index_compressed_wasm_hash = IndexWasm::from(INDEX_BYTECODE).hash().clone().to_string();
    // let archive_compressed_wasm_hash = ArchiveWasm::from(ARCHIVE_NODE_BYTECODE)
    //     .hash()
    //     .clone()
    //     .to_string();
    // println!("{:?}", ledger_compressed_wasm_hash);
    // println!("{:?}", index_compressed_wasm_hash);
    // println!("{:?}", archive_compressed_wasm_hash);
}
