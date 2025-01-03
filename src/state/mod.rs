#[cfg(test)]
pub(crate) mod test_fixtures;

#[cfg(test)]
mod tests;

use candid::{Nat, Principal};
use ic_cdk::trap;
use ic_ethereum_types::Address;
use ic_stable_structures::{storable::Bound, Cell, Storable};
use num_traits::ToPrimitive;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_bytes::ByteArray;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Display, Formatter};
use std::iter::once;
use std::marker::PhantomData;
use std::str::FromStr;

use crate::endpoints::{
    CyclesManagement, Erc20Contract, InitArg, InstalledNativeLedgerSuite,
    InvalidNativeInstalledCanistersError, UpdateLedgerSuiteCreationFee,
};
use crate::ledger_suite_manager::install_ls::InstallLedgerSuiteArgs;
use crate::storage::memory::{state_memory, StableMemory};

thread_local! {
    pub static STATE: RefCell<Cell<ConfigState, StableMemory>> = RefCell::new(Cell::init(
   state_memory(), ConfigState::default())
    .expect("failed to initialize stable cell for state"));
}

const WASM_HASH_LENGTH: usize = 32;

/// `Wasm<Canister>` is a wrapper around a wasm binary and its memoized hash.
/// It provides a type-safe way to handle wasm binaries for different canisters.
#[derive(Debug)]
pub struct Wasm<T> {
    binary: Vec<u8>,
    hash: WasmHash,
    marker: PhantomData<T>,
}

pub type LedgerWasm = Wasm<Ledger>;
pub type IndexWasm = Wasm<Index>;
pub type ArchiveWasm = Wasm<Archive>;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
#[serde(from = "serde_bytes::ByteArray<N>", into = "serde_bytes::ByteArray<N>")]
pub struct Hash<const N: usize>([u8; N]);

impl<const N: usize> Default for Hash<N> {
    fn default() -> Self {
        Self([0; N])
    }
}

impl<const N: usize> From<ByteArray<N>> for Hash<N> {
    fn from(value: ByteArray<N>) -> Self {
        Self(value.into_array())
    }
}

impl<const N: usize> From<Hash<N>> for ByteArray<N> {
    fn from(value: Hash<N>) -> Self {
        ByteArray::new(value.0)
    }
}

impl<const N: usize> AsRef<[u8]> for Hash<N> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<const N: usize> From<[u8; N]> for Hash<N> {
    fn from(value: [u8; N]) -> Self {
        Self(value)
    }
}

impl<const N: usize> From<Hash<N>> for [u8; N] {
    fn from(value: Hash<N>) -> Self {
        value.0
    }
}

impl<const N: usize> FromStr for Hash<N> {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let expected_num_hex_chars = N * 2;
        if s.len() != expected_num_hex_chars {
            return Err(format!(
                "Invalid hash: expected {} characters, got {}",
                expected_num_hex_chars,
                s.len()
            ));
        }
        let mut bytes = [0u8; N];
        hex::decode_to_slice(s, &mut bytes).map_err(|e| format!("Invalid hex string: {}", e))?;
        Ok(Self(bytes))
    }
}

impl<const N: usize> Display for Hash<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl<const N: usize> Storable for Hash<N> {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::from(self.as_ref())
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        assert_eq!(bytes.len(), N, "Hash representation is {}-bytes long", N);
        let mut be_bytes = [0u8; N];
        be_bytes.copy_from_slice(bytes.as_ref());
        Self(be_bytes)
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: N as u32,
        is_fixed_size: true,
    };
}

pub type WasmHash = Hash<WASM_HASH_LENGTH>;

impl WasmHash {
    /// Creates an array of wasm hashes from an array of their respective string representations.
    /// This method preserves the order of the input strings:
    /// element with index i in the input, will have index i in the output.
    /// The input strings are expected to be distinct and valid wasm hashes.
    ///
    /// # Errors
    /// * If any of the strings is not a valid wasm hash.
    /// * If there are any duplicates.
    pub fn from_distinct_opt_str<const N: usize>(
        hashes: [Option<&str>; N],
    ) -> Result<[Option<WasmHash>; N], String> {
        let mut duplicates = BTreeSet::new();
        let mut result = Vec::with_capacity(N);
        for maybe_hash in hashes {
            match maybe_hash {
                None => {
                    result.push(None);
                }
                Some(hash) => {
                    let hash = WasmHash::from_str(hash)?;
                    if !duplicates.insert(hash.clone()) {
                        return Err(format!("Duplicate hash: {}", hash));
                    }
                    result.push(Some(hash));
                }
            }
        }
        Ok(result
            .try_into()
            .map_err(|_err| "failed to convert to fixed size array")
            .expect("BUG: failed to convert to fixed size array"))
    }
}

impl<T> Wasm<T> {
    pub fn new(binary: Vec<u8>) -> Self {
        let hash = WasmHash::from(ic_crypto_sha2::Sha256::hash(binary.as_slice()));
        Self {
            binary,
            hash,
            marker: PhantomData,
        }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        self.binary
    }

    pub fn hash(&self) -> &WasmHash {
        &self.hash
    }
}

impl<T> Clone for Wasm<T> {
    fn clone(&self) -> Self {
        Self::new(self.binary.clone())
    }
}

impl<T> PartialEq for Wasm<T> {
    fn eq(&self, other: &Self) -> bool {
        self.binary.eq(&other.binary)
    }
}

impl<T> From<Vec<u8>> for Wasm<T> {
    fn from(v: Vec<u8>) -> Self {
        Self::new(v)
    }
}

impl<T> From<&[u8]> for Wasm<T> {
    fn from(value: &[u8]) -> Self {
        Self::new(value.to_vec())
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
pub struct Erc20Token(ChainId, Address);

impl Erc20Token {
    pub fn new(chain_id: ChainId, address: Address) -> Self {
        Self(chain_id, address)
    }

    pub fn chain_id(&self) -> &ChainId {
        &self.0
    }

    pub fn address(&self) -> &Address {
        &self.1
    }
}
impl From<Nat> for ChainId {
    fn from(value: Nat) -> Self {
        Self(value.0.to_u64().unwrap())
    }
}

impl TryFrom<Erc20Contract> for Erc20Token {
    type Error = String;

    fn try_from(contract: crate::endpoints::Erc20Contract) -> Result<Self, Self::Error> {
        Ok(Self(
            ChainId(contract.chain_id.0.to_u64().ok_or("chain_id is not u64")?),
            Address::from_str(&contract.address)?,
        ))
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ChainId(u64);

impl AsRef<u64> for ChainId {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}

#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct ManagedCanisters {
    /// Canisters for an ERC-20 token
    /// For native tokens 0x000000000...will be considered as the contract address
    canisters: BTreeMap<Erc20Token, Canisters>,
}

impl ManagedCanisters {
    pub fn find_by_id(&self, token: &Erc20Token) -> Option<&Canisters> {
        self.canisters.get(token)
    }

    pub fn get_mut(&mut self, token: &Erc20Token) -> Option<&mut Canisters> {
        self.canisters.get_mut(&token)
    }

    pub fn insert_once(&mut self, token: Erc20Token, canisters: Canisters) {
        assert_eq!(
            self.find_by_id(&token),
            None,
            "BUG: token {:?} is already managed",
            token
        );

        let previous_element = self.canisters.insert(token, canisters);

        assert_eq!(previous_element, None);
    }

    pub fn all_canisters_iter(&self) -> impl Iterator<Item = (Erc20Token, &Canisters)> {
        self.canisters
            .iter()
            .map(|(key, value)| (Erc20Token::from(key.clone()), value))
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Canisters {
    pub ledger: Option<LedgerCanister>,
    pub index: Option<IndexCanister>,
    pub archives: Vec<Principal>,
    pub metadata: CanistersMetadata,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize, Serialize)]
pub struct CanistersMetadata {
    pub token_symbol: String,
}

impl Canisters {
    pub fn new(metadata: CanistersMetadata) -> Self {
        Self {
            ledger: None,
            index: None,
            archives: vec![],
            metadata,
        }
    }

    pub fn ledger_canister_id(&self) -> Option<&Principal> {
        self.ledger.as_ref().map(LedgerCanister::canister_id)
    }

    pub fn index_canister_id(&self) -> Option<&Principal> {
        self.index.as_ref().map(IndexCanister::canister_id)
    }

    pub fn archive_canister_ids(&self) -> &[Principal] {
        &self.archives
    }

    pub fn principals_iter(&self) -> impl Iterator<Item = &Principal> {
        self.ledger_canister_id()
            .into_iter()
            .chain(self.index_canister_id())
            .chain(self.archive_canister_ids().iter())
    }
}

#[derive(Debug)]
pub struct Canister<T> {
    status: ManagedCanisterStatus,
    marker: PhantomData<T>,
}

impl<T> Clone for Canister<T> {
    fn clone(&self) -> Self {
        Self::new(self.status.clone())
    }
}

impl<T> PartialEq for Canister<T> {
    fn eq(&self, other: &Self) -> bool {
        self.status.eq(&other.status)
    }
}

impl<T> Canister<T> {
    pub fn new(status: ManagedCanisterStatus) -> Self {
        Self {
            status,
            marker: PhantomData,
        }
    }

    pub fn canister_id(&self) -> &Principal {
        self.status.canister_id()
    }

    pub fn installed_wasm_hash(&self) -> Option<&WasmHash> {
        self.status.installed_wasm_hash()
    }

    pub fn status(&self) -> &ManagedCanisterStatus {
        &self.status
    }
}

impl<T> Serialize for Canister<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.status.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for Canister<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        ManagedCanisterStatus::deserialize(deserializer).map(Self::new)
    }
}

#[derive(Debug)]
pub enum Ledger {}

pub type LedgerCanister = Canister<Ledger>;

#[derive(Debug)]
pub enum Index {}

pub type IndexCanister = Canister<Index>;

#[derive(Debug)]
pub enum Archive {}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub enum ManagedCanisterStatus {
    /// Canister created with the given principal
    /// but wasm module is not yet installed.
    Created { canister_id: Principal },

    /// Canister created and wasm module installed.
    /// The wasm_hash reflects the installed wasm module by the lsm
    /// but *may differ* from the one being currently deployed (if another controller did an upgrade)
    Installed {
        canister_id: Principal,
        installed_wasm_hash: WasmHash,
    },
}

impl ManagedCanisterStatus {
    pub fn canister_id(&self) -> &Principal {
        match self {
            ManagedCanisterStatus::Created { canister_id }
            | ManagedCanisterStatus::Installed { canister_id, .. } => canister_id,
        }
    }

    fn installed_wasm_hash(&self) -> Option<&WasmHash> {
        match self {
            ManagedCanisterStatus::Created { .. } => None,
            ManagedCanisterStatus::Installed {
                installed_wasm_hash,
                ..
            } => Some(installed_wasm_hash),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct LedgerSuiteVersion {
    pub ledger_compressed_wasm_hash: WasmHash,
    pub index_compressed_wasm_hash: WasmHash,
    pub archive_compressed_wasm_hash: WasmHash,
}

/// Configuration state of the lsm.
#[derive(Clone, PartialEq, Debug, Default)]
enum ConfigState {
    #[default]
    Uninitialized,
    // This state is only used between wasm module initialization and init().
    Initialized(State),
}

impl ConfigState {
    fn expect_initialized(&self) -> &State {
        match &self {
            ConfigState::Uninitialized => trap("BUG: state not initialized"),
            ConfigState::Initialized(s) => s,
        }
    }
}

impl Storable for ConfigState {
    fn to_bytes(&self) -> Cow<[u8]> {
        match &self {
            ConfigState::Uninitialized => Cow::Borrowed(&[]),
            ConfigState::Initialized(config) => Cow::Owned(encode(config)),
        }
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        if bytes.is_empty() {
            return ConfigState::Uninitialized;
        }
        ConfigState::Initialized(decode(bytes.as_ref()))
    }

    const BOUND: Bound = Bound::Unbounded;
}

fn encode<S: ?Sized + serde::Serialize>(state: &S) -> Vec<u8> {
    let mut buf = vec![];
    ciborium::ser::into_writer(state, &mut buf).expect("failed to encode state");
    buf
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> T {
    ciborium::de::from_reader(bytes)
        .unwrap_or_else(|e| panic!("failed to decode state bytes {}: {e}", hex::encode(bytes)))
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct LedgerSuiteCreationFee {
    pub icp: u128,
    pub appic: Option<u128>,
}

impl LedgerSuiteCreationFee {
    pub fn new(icp: u128, appic: Option<u128>) -> Self {
        Self { icp, appic }
    }
}

impl From<UpdateLedgerSuiteCreationFee> for LedgerSuiteCreationFee {
    fn from(value: UpdateLedgerSuiteCreationFee) -> Self {
        let icp_to_u128 = value.icp.0.to_u128().unwrap();
        let appic_to_u128 = match value.appic {
            Some(appic_nat) => Some(appic_nat.0.to_u128().unwrap()),
            None => None,
        };
        Self::new(icp_to_u128, appic_to_u128)
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ReceivedDeposit {
    pub timestamp: u64,
    pub amount: u128,
    pub from_principal: Principal,
    pub erc20_token: Erc20Token,
    pub transfer_index: u64,
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct State {
    managed_canisters: ManagedCanisters,
    cycles_management: CyclesManagement,
    more_controller_ids: Vec<Principal>,

    // For every evm chain there is a specific minter canister
    minter_id: BTreeMap<ChainId, Principal>,

    // New LedgerSuite requests waiting to be created
    pub twin_ledger_suites_to_be_installed: BTreeMap<Erc20Token, InstallLedgerSuiteArgs>,

    // The ledger suites installs that are not recoverable
    failed_ledger_suite_installs: BTreeMap<Erc20Token, InstallLedgerSuiteArgs>,

    #[serde(default)]
    ledger_suite_version: Option<LedgerSuiteVersion>,

    // Collected icp or appic token in the beginning for ledger suite creation
    collected_icp_token: u128,
    collected_appic_token: u128,
    minimum_tokens_for_new_ledger_suite: LedgerSuiteCreationFee,

    // Received deposits for twin ledger suite creation
    received_deposits: Vec<ReceivedDeposit>,

    // Minters to be notified of new erc20
    pub notify_add_erc20_list: BTreeMap<Erc20Token, Principal>,
}

impl State {
    pub fn more_controller_ids(&self) -> &[Principal] {
        &self.more_controller_ids
    }

    pub fn minter_id(&self, chain_id: &ChainId) -> Option<&Principal> {
        self.minter_id.get(chain_id)
    }

    pub fn minter_id_owned(&self, chain_id: &ChainId) -> Option<Principal> {
        self.minter_id.get(chain_id).copied()
    }

    pub fn all_minter_ids(&self) -> Vec<(ChainId, Principal)> {
        self.minter_id.clone().into_iter().collect()
    }

    pub fn record_new_minter_ids(&mut self, new_minters: Vec<(ChainId, Principal)>) {
        self.minter_id
            .append(&mut BTreeMap::from_iter(new_minters.into_iter()));
    }

    pub fn record_new_erc20_minter_notification(
        &mut self,
        token: &Erc20Token,
        minter_id: &Principal,
    ) {
        self.notify_add_erc20_list
            .insert(token.clone(), minter_id.clone());
    }

    pub fn remove_erc20_from_minter_notification_list(&mut self, token: &Erc20Token) {
        self.notify_add_erc20_list.remove(token);
    }

    pub fn cycles_management(&self) -> &CyclesManagement {
        &self.cycles_management
    }

    pub fn cycles_management_mut(&mut self) -> &mut CyclesManagement {
        &mut self.cycles_management
    }

    pub fn all_managed_canisters_iter(&self) -> impl Iterator<Item = (Erc20Token, &Canisters)> {
        self.managed_canisters.all_canisters_iter()
    }

    pub fn all_managed_principals(&self) -> impl Iterator<Item = &Principal> {
        self.all_managed_canisters_iter()
            .flat_map(|(_, canisters)| canisters.principals_iter())
    }

    pub fn all_managed_tokens_ids_iter(&self) -> impl Iterator<Item = Erc20Token> + '_ {
        self.all_managed_canisters_iter().map(|(id, _)| id)
    }

    pub fn managed_canisters(&self, token_id: &Erc20Token) -> Option<&Canisters> {
        self.managed_canisters.find_by_id(token_id)
    }

    pub fn ledger_suite_version(&self) -> Option<&LedgerSuiteVersion> {
        self.ledger_suite_version.as_ref()
    }

    pub fn minimum_tokens_for_new_ledger_suite(&self) -> LedgerSuiteCreationFee {
        self.minimum_tokens_for_new_ledger_suite.clone()
    }

    pub fn update_minimum_tokens_for_new_ledger_suite(
        &mut self,
        new_ls_fees: LedgerSuiteCreationFee,
    ) {
        self.minimum_tokens_for_new_ledger_suite = new_ls_fees
    }

    /// Initializes the ledger suite version if it is not already set.
    /// No-op if the ledger suite version is already set.
    pub fn init_ledger_suite_version(&mut self, version: LedgerSuiteVersion) {
        if self.ledger_suite_version.is_none() {
            self.ledger_suite_version = Some(version);
        }
    }

    pub fn update_ledger_suite_version(&mut self, new_version: LedgerSuiteVersion) {
        self.ledger_suite_version = Some(new_version);
    }

    fn managed_canisters_mut(&mut self, token_id: &Erc20Token) -> Option<&mut Canisters> {
        self.managed_canisters.get_mut(token_id)
    }

    pub fn managed_status<'a, T: 'a>(
        &'a self,
        token: &Erc20Token,
    ) -> Option<&'a ManagedCanisterStatus>
    where
        Canisters: ManageSingleCanister<T>,
    {
        self.managed_canisters(token)
            .and_then(|c| c.get().map(|c: &Canister<T>| &c.status))
    }

    pub fn record_new_native_erc20_token(
        &mut self,
        token: Erc20Token,
        native_ls: InstalledNativeLedgerSuite,
    ) {
        self.managed_canisters
            .insert_once(token, Canisters::from(native_ls));
    }

    pub fn record_new_erc20_token(&mut self, token: Erc20Token, metadata: CanistersMetadata) {
        self.managed_canisters
            .insert_once(token, Canisters::new(metadata));
    }

    pub fn record_new_icp_deposit(
        &mut self,
        erc20_token: Erc20Token,
        transfer_index: u64,
        icp_amount: u128,
        from_principal: Principal,
    ) {
        let deposit = ReceivedDeposit {
            timestamp: ic_cdk::api::time(),
            amount: icp_amount,
            from_principal,
            erc20_token,
            transfer_index,
        };
        // Add to the collected icp
        self.collected_icp_token += icp_amount;
        self.received_deposits.push(deposit);
    }

    pub fn record_new_ledger_suite_request(
        &mut self,
        erc20_token: Erc20Token,
        install_args: InstallLedgerSuiteArgs,
    ) {
        self.twin_ledger_suites_to_be_installed
            .insert(erc20_token, install_args);
    }

    pub fn record_archives(&mut self, token: &Erc20Token, archives: Vec<Principal>) {
        let canisters = self
            .managed_canisters_mut(token)
            .unwrap_or_else(|| panic!("BUG: token {:?} is not managed", token));
        canisters.archives = archives;
    }

    pub fn record_created_canister<T: Debug>(&mut self, token: &Erc20Token, canister_id: Principal)
    where
        Canisters: ManageSingleCanister<T>,
    {
        let canisters = self
            .managed_canisters_mut(&token)
            .unwrap_or_else(|| panic!("BUG: token {:?} is not managed", token));
        canisters
            .try_insert(Canister::<T>::new(ManagedCanisterStatus::Created {
                canister_id,
            }))
            .unwrap_or_else(|e| {
                panic!(
                    "BUG: canister {} already created: {:?}",
                    Canisters::display_name(),
                    e
                )
            });
    }

    pub fn record_installed_canister<T>(&mut self, token: &Erc20Token, wasm_hash: WasmHash)
    where
        Canisters: ManageSingleCanister<T>,
    {
        let managed_canister: &mut Canister<T> = self
            .managed_canisters_mut(&token)
            .and_then(Canisters::get_mut)
            .unwrap_or_else(|| {
                panic!(
                    "BUG: no managed canisters or no {} canister for {:?}",
                    Canisters::display_name(),
                    token
                )
            });
        let canister_id = *managed_canister.canister_id();
        managed_canister.status = ManagedCanisterStatus::Installed {
            canister_id,
            installed_wasm_hash: wasm_hash,
        };
    }

    pub fn remove_installed_ls_from_installing_queue(&mut self, erc20_token: Erc20Token) {
        self.twin_ledger_suites_to_be_installed.remove(&erc20_token);
    }

    pub fn record_failed_ls_install(
        &mut self,
        erc20_token: Erc20Token,
        install_args: InstallLedgerSuiteArgs,
    ) {
        self.remove_installed_ls_from_installing_queue(erc20_token.clone());
        self.failed_ledger_suite_installs
            .insert(erc20_token, install_args);
    }

    pub fn validate_config(&self) -> Result<(), InvalidStateError> {
        const MAX_ADDITIONAL_CONTROLLERS: usize = 9;
        if self.more_controller_ids.len() > MAX_ADDITIONAL_CONTROLLERS {
            return Err(InvalidStateError::TooManyAdditionalControllers {
                max: MAX_ADDITIONAL_CONTROLLERS,
                actual: self.more_controller_ids.len(),
            });
        }
        Ok(())
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum InvalidStateError {
    TooManyAdditionalControllers { max: usize, actual: usize },
}

impl TryFrom<InitArg> for State {
    type Error = InvalidStateError;
    fn try_from(
        InitArg {
            more_controller_ids,
            minter_ids,
            cycles_management,
            twin_ls_creation_fee_appic_token,
            twin_ls_creation_fee_icp_token,
        }: InitArg,
    ) -> Result<Self, Self::Error> {
        // Parse chain ids from Nat format into ChainId types
        let minter_ids_vec: Vec<(ChainId, Principal)> = minter_ids
            .into_iter()
            .map(|(chain_id, minter_principal)| (ChainId::from(chain_id), minter_principal))
            .collect();

        // Initialize BTreeMap with the parsed vector of minter ids
        let minter_ids_map: BTreeMap<ChainId, Principal> =
            BTreeMap::from_iter(minter_ids_vec.into_iter());

        // Convert Nat to u128 for icp
        let icp_ls_creation_fee = twin_ls_creation_fee_icp_token
            .0
            .to_u128()
            .expect("Should not fail converting Nat into u128");

        // Convert Nat to u128 for appic
        let appic_ls_creation_fee = match twin_ls_creation_fee_appic_token {
            Some(appic_nat) => Some(
                appic_nat
                    .0
                    .to_u128()
                    .expect("Should not fail converting Nat into u128"),
            ),
            None => None,
        };

        // Produce state
        let state = Self {
            managed_canisters: Default::default(),
            cycles_management: cycles_management.unwrap_or_default(),
            more_controller_ids,
            minter_id: minter_ids_map,
            ledger_suite_version: Default::default(),
            twin_ledger_suites_to_be_installed: Default::default(),
            failed_ledger_suite_installs: Default::default(),
            collected_icp_token: 0,
            collected_appic_token: 0,
            minimum_tokens_for_new_ledger_suite: LedgerSuiteCreationFee::new(
                icp_ls_creation_fee,
                appic_ls_creation_fee,
            ),
            received_deposits: Default::default(),
            notify_add_erc20_list: Default::default(),
        };
        state.validate_config()?;
        Ok(state)
    }
}

pub fn read_state<R>(f: impl FnOnce(&State) -> R) -> R {
    STATE.with(|cell| f(cell.borrow().get().expect_initialized()))
}

/// Mutates (part of) the current state using `f`.
///
/// Panics if there is no state.
pub fn mutate_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut State) -> R,
{
    STATE.with(|cell| {
        let mut borrowed = cell.borrow_mut();
        let mut state = borrowed.get().expect_initialized().clone();
        let result = f(&mut state);
        borrowed
            .set(ConfigState::Initialized(state))
            .expect("failed to write state in stable cell");
        result
    })
}

pub fn init_state(state: State) {
    STATE.with(|cell| {
        let mut borrowed = cell.borrow_mut();
        assert_eq!(
            borrowed.get(),
            &ConfigState::Uninitialized,
            "BUG: State is already initialized and has value {:?}",
            borrowed.get()
        );
        borrowed
            .set(ConfigState::Initialized(state))
            .expect("failed to initialize state in stable cell")
    });
}

pub trait ManageSingleCanister<T> {
    fn display_name() -> &'static str;

    fn get(&self) -> Option<&Canister<T>>;

    fn get_mut(&mut self) -> Option<&mut Canister<T>>;

    fn try_insert(&mut self, canister: Canister<T>) -> Result<(), OccupiedError<Canister<T>>>;
}

#[derive(Clone, PartialEq, Debug)]
pub struct OccupiedError<T> {
    value: T,
}

impl ManageSingleCanister<Ledger> for Canisters {
    fn display_name() -> &'static str {
        "ledger"
    }

    fn get(&self) -> Option<&Canister<Ledger>> {
        self.ledger.as_ref()
    }

    fn get_mut(&mut self) -> Option<&mut Canister<Ledger>> {
        self.ledger.as_mut()
    }

    fn try_insert(
        &mut self,
        canister: Canister<Ledger>,
    ) -> Result<(), OccupiedError<Canister<Ledger>>> {
        match self.get() {
            Some(c) => Err(OccupiedError { value: c.clone() }),
            None => {
                self.ledger = Some(canister);
                Ok(())
            }
        }
    }
}

impl ManageSingleCanister<Index> for Canisters {
    fn display_name() -> &'static str {
        "index"
    }

    fn get(&self) -> Option<&Canister<Index>> {
        self.index.as_ref()
    }

    fn get_mut(&mut self) -> Option<&mut Canister<Index>> {
        self.index.as_mut()
    }

    fn try_insert(
        &mut self,
        canister: Canister<Index>,
    ) -> Result<(), OccupiedError<Canister<Index>>> {
        match self.get() {
            Some(c) => Err(OccupiedError { value: c.clone() }),
            None => {
                self.index = Some(canister);
                Ok(())
            }
        }
    }
}

impl InstalledNativeLedgerSuite {
    pub fn validate(
        self,
        state: &State,
    ) -> Result<InstalledNativeLedgerSuite, InvalidNativeInstalledCanistersError> {
        let symbol = self.symbol.clone();
        let token_id = self.get_erc20_token();
        if state.managed_canisters(&token_id).is_some() {
            return Err(InvalidNativeInstalledCanistersError::TokenAlreadyManaged);
        }
        let ledger = self.ledger;
        let ledger_wasm_hash: Hash<WASM_HASH_LENGTH> = Hash::from_str(&self.ledger_wasm_hash)
            .map_err(|_e| InvalidNativeInstalledCanistersError::WasmHashError)?;
        let index: Principal = self.index;
        let index_wasm_hash: Hash<WASM_HASH_LENGTH> = Hash::from_str(&self.index_wasm_hash)
            .map_err(|_e| InvalidNativeInstalledCanistersError::WasmHashError)?;
        if ledger_wasm_hash == index_wasm_hash {
            return Err(InvalidNativeInstalledCanistersError::WasmHashError);
        }
        let archives = self.archives;

        let installed_principals: BTreeSet<_> = once(&ledger)
            .chain(once(&index))
            .chain(archives.iter())
            .collect();
        let managed_principals: BTreeSet<_> = state.all_managed_principals().collect();
        let overlapping_principals: BTreeSet<_> = managed_principals
            .intersection(&installed_principals)
            .collect();
        if !overlapping_principals.is_empty() {
            return Err(InvalidNativeInstalledCanistersError::AlreadyManagedPrincipals);
        }
        Ok(InstalledNativeLedgerSuite {
            decimals: self.decimals,
            fee: self.fee,
            logo: self.logo,
            name: self.name,
            symbol,
            ledger,
            ledger_wasm_hash: ledger_wasm_hash.to_string(),
            index,
            index_wasm_hash: index_wasm_hash.to_string(),
            archives,
            chain_id: self.chain_id,
        })
    }

    pub fn get_erc20_token(&self) -> Erc20Token {
        Erc20Token::new(
            ChainId(self.chain_id.0.to_u64().unwrap()),
            Address::from_str("0x0000000000000000000000000000000000000000")
                .expect("Should not fail"),
        )
    }
}
