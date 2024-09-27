use crate::ledger_suite_manager::{Task, TaskExecution};
use crate::state::{
    Archive, ArchiveWasm, Index, IndexWasm, Ledger, LedgerSuiteVersion, LedgerWasm, Wasm, WasmHash,
};
use crate::storage::memory::{
    deadline_by_task_memory, task_queue_memory, wasm_store_memory, StableMemory,
};
use candid::Deserialize;
use ic_stable_structures::storable::Bound;
use ic_stable_structures::{BTreeMap, Storable};
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;

pub(crate) mod memory {
    use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
    use ic_stable_structures::DefaultMemoryImpl;
    use std::cell::RefCell;

    thread_local! {
         static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
            MemoryManager::init(DefaultMemoryImpl::default())
        );
    }

    const STATE_MEMORY_ID: MemoryId = MemoryId::new(0);
    const WASM_STORE_MEMORY_ID: MemoryId = MemoryId::new(1);
    const TASK_QUEUE_ID: MemoryId = MemoryId::new(2);
    const DEADLINE_BY_TASK_ID: MemoryId = MemoryId::new(3);

    pub type StableMemory = VirtualMemory<DefaultMemoryImpl>;

    pub fn state_memory() -> StableMemory {
        MEMORY_MANAGER.with(|m| m.borrow().get(STATE_MEMORY_ID))
    }

    pub fn wasm_store_memory() -> StableMemory {
        MEMORY_MANAGER.with(|m| m.borrow().get(WASM_STORE_MEMORY_ID))
    }

    pub fn task_queue_memory() -> StableMemory {
        MEMORY_MANAGER.with(|m| m.borrow().get(TASK_QUEUE_ID))
    }

    pub fn deadline_by_task_memory() -> StableMemory {
        MEMORY_MANAGER.with(|m| m.borrow().get(DEADLINE_BY_TASK_ID))
    }
}

pub type WasmStore = BTreeMap<WasmHash, StoredWasm, StableMemory>;

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StoredWasm {
    /// The canister time at which the orchestrator stored this wasm
    /// in nanoseconds since the epoch (1970-01-01).
    timestamp: u64,
    /// The wasm binary.
    #[serde(with = "serde_bytes")]
    binary: Vec<u8>,
    /// Encodes which type of wasm this is.
    marker: u8,
}

impl StoredWasm {
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn marker(&self) -> u8 {
        self.marker
    }
}

pub trait StorableWasm {
    const MARKER: u8;
}

impl StorableWasm for Ledger {
    const MARKER: u8 = 0;
}

impl StorableWasm for Index {
    const MARKER: u8 = 1;
}

impl StorableWasm for Archive {
    const MARKER: u8 = 2;
}

impl Storable for StoredWasm {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        ciborium::ser::into_writer(&self, &mut buf)
            .expect("failed to encode a StorableWasm to bytes");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        ciborium::de::from_reader(bytes.as_ref()).unwrap_or_else(|e| {
            panic!(
                "failed to decode StorableWasm bytes {}: {e}",
                hex::encode(bytes)
            )
        })
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for TaskExecution {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        ciborium::ser::into_writer(&self, &mut buf)
            .expect("failed to encode a TaskExecution to bytes");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        ciborium::de::from_reader(bytes.as_ref()).unwrap_or_else(|e| {
            panic!(
                "failed to decode TaskExecution bytes {}: {e}",
                hex::encode(bytes)
            )
        })
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for Task {
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        ciborium::ser::into_writer(&self, &mut buf).expect("failed to encode a Task to bytes");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        ciborium::de::from_reader(bytes.as_ref())
            .unwrap_or_else(|e| panic!("failed to decode Task bytes {}: {e}", hex::encode(bytes)))
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(Clone, PartialEq, Debug)]
pub enum WasmStoreError {
    WasmMismatch {
        wasm_hash: WasmHash,
        expected_marker: u8,
        actual_marker: u8,
    },
}

/// Inserts the wasm into the store.
///
/// If a wasm with the same hash is already present and has the same type,
/// the store will not be modified.
///
/// # Errors
/// * [`WasmStoreError::WasmMismatch`] if a wasm with the same hash but a different type is already present
///
pub fn wasm_store_try_insert<T: StorableWasm>(
    wasm_store: &mut WasmStore,
    timestamp: u64,
    wasm: Wasm<T>,
) -> Result<(), WasmStoreError> {
    let wasm_hash = wasm.hash().clone();
    let storable_wasm = StoredWasm {
        timestamp,
        binary: wasm.to_bytes(),
        marker: T::MARKER,
    };
    match wasm_store.get(&wasm_hash) {
        Some(stored_wasm) if stored_wasm.marker == storable_wasm.marker => Ok(()),
        Some(stored_wasm) => Err(WasmStoreError::WasmMismatch {
            wasm_hash,
            expected_marker: storable_wasm.marker,
            actual_marker: stored_wasm.marker,
        }),
        None => {
            assert_eq!(
                wasm_store.insert(wasm_hash, storable_wasm),
                None,
                "unexpected previous value"
            );
            Ok(())
        }
    }
}

/// Retrieves a wasm identified by its hash from the store, or `Ok(None)` if no such wasm is present.
///
/// # Errors
/// * [`WasmStoreError::WasmMismatch`] if a wasm with the given hash but an unexpected type is present
pub fn wasm_store_try_get<T: StorableWasm>(
    wasm_store: &WasmStore,
    wasm_hash: &WasmHash,
) -> Result<Option<Wasm<T>>, WasmStoreError> {
    match wasm_store.get(wasm_hash) {
        Some(stored_wasm) => {
            if stored_wasm.marker != T::MARKER {
                return Err(WasmStoreError::WasmMismatch {
                    wasm_hash: wasm_hash.clone(),
                    expected_marker: T::MARKER,
                    actual_marker: stored_wasm.marker,
                });
            }
            Ok(Some(Wasm::from(stored_wasm.binary)))
        }
        None => Ok(None),
    }
}

pub fn record_icrc1_ledger_suite_wasms(
    wasm_store: &mut WasmStore,
    timestamp: u64,
) -> Result<LedgerSuiteVersion, WasmStoreError> {
    let ledger_compressed_wasm_hash =
        record_wasm(wasm_store, timestamp, LedgerWasm::from(LEDGER_BYTECODE))?;
    let index_compressed_wasm_hash =
        record_wasm(wasm_store, timestamp, IndexWasm::from(INDEX_BYTECODE))?;
    let archive_compressed_wasm_hash = record_wasm(
        wasm_store,
        timestamp,
        ArchiveWasm::from(ARCHIVE_NODE_BYTECODE),
    )?;
    Ok(LedgerSuiteVersion {
        ledger_compressed_wasm_hash,
        index_compressed_wasm_hash,
        archive_compressed_wasm_hash,
    })
}

fn record_wasm<T: StorableWasm>(
    wasm_store: &mut WasmStore,
    timestamp: u64,
    wasm: Wasm<T>,
) -> Result<WasmHash, WasmStoreError> {
    let hash = wasm.hash().clone();
    wasm_store_try_insert(wasm_store, timestamp, wasm).map(|()| hash)
}
