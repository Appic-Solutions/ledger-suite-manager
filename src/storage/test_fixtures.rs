use crate::storage::{ArchiveWasm, IndexWasm, LedgerSuiteVersion, LedgerWasm, WasmStore};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager};
use ic_stable_structures::DefaultMemoryImpl;

pub fn empty_wasm_store() -> WasmStore {
    WasmStore::init(MemoryManager::init(DefaultMemoryImpl::default()).get(MemoryId::new(0)))
}

pub fn embedded_ledger_suite_version() -> LedgerSuiteVersion {
    LedgerSuiteVersion {
        ledger_compressed_wasm_hash: LedgerWasm::from(crate::storage::LEDGER_BYTECODE)
            .hash()
            .clone(),
        index_compressed_wasm_hash: IndexWasm::from(crate::storage::INDEX_BYTECODE)
            .hash()
            .clone(),
        archive_compressed_wasm_hash: ArchiveWasm::from(crate::storage::ARCHIVE_NODE_BYTECODE)
            .hash()
            .clone(),
    }
}
