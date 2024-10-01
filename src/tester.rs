use candid::Principal;
use icrc_ledger_types::icrc1::account::{Account, Subaccount};
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
pub fn checker() {
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
