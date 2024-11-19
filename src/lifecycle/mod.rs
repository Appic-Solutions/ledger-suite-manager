use crate::endpoints::{InitArg, UpgradeArg};
use crate::logs::INFO;
use crate::state::{init_state, mutate_state, read_state, ChainId, LedgerSuiteCreationFee, State};
use crate::storage::{mutate_wasm_store, read_wasm_store, record_icrc1_ledger_suite_wasms};
use ic_canister_log::log;
use std::str::FromStr;

pub fn init(init_arg: InitArg) {
    log!(
        INFO,
        "[init]: initialized orchestrator with arg: {:?}",
        init_arg
    );

    // Generate the state by init args
    let mut generate_state_from_init_args =
        State::try_from(init_arg).expect("ERROR: failed to initialize ledger suite orchestrator");

    // Add the first wasm suite (Ledger,Index,Archive) to wasm store.
    // Wasms are genrated by (index,ledger,archive) wasms located in wasms directory.
    let ledger_suite_version =
        mutate_wasm_store(|s| record_icrc1_ledger_suite_wasms(s, ic_cdk::api::time()))
            .expect("BUG: failed to record icrc1 ledger suite wasms during init");

    // Add the ls version (wasm hashes) to the state.
    generate_state_from_init_args.init_ledger_suite_version(ledger_suite_version);

    // Init the state with generated state that includes first ledger suite version.
    init_state(generate_state_from_init_args);
}

pub fn post_upgrade(upgrade_arg: Option<UpgradeArg>) {
    if let Some(arg) = upgrade_arg {
        if let Some(update) = arg.cycles_management {
            mutate_state(|s| update.apply(s.cycles_management_mut()));
        }
        if let Some(update) = arg.twin_ls_creation_fees {
            mutate_state(|s| s.upate_minimum_tokens_for_new_ledger_suite(update.into()));
        }

        if let Some(update) = arg.new_minter_ids {
            let remapped_minter_ids = update
                .into_iter()
                .map(|(chain_id, principal_id)| (ChainId::from(chain_id), principal_id))
                .collect();
            mutate_state(|s| s.record_new_minter_ids(remapped_minter_ids));
        }

        // TODO: Mechanism for upgrading ledger suite wasm hash.
    }
}
