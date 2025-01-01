use std::cell::RefCell;

use std::collections::HashSet;

use crate::ledger_suite_manager::PeriodicTasksTypes;

thread_local! {
    pub static ACTIVE_TASKS:RefCell<Option<HashSet<PeriodicTasksTypes>>>=RefCell::new(Some(HashSet::default()));
}
/// Mutates (part of) the current state using `f`.
///
/// Panics if there is no state.
pub fn mutate_active_tasks<F, R>(f: F) -> R
where
    F: FnOnce(&mut HashSet<PeriodicTasksTypes>) -> R,
{
    ACTIVE_TASKS.with(|s| {
        f(s.borrow_mut()
            .as_mut()
            .expect("BUG: active tasks not initialized"))
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct TimerGuard {
    task: PeriodicTasksTypes,
}
#[derive(Debug, PartialEq, Eq)]
pub enum TimerGuardError {
    AlreadyProcessing,
}

impl TimerGuard {
    pub fn new(task: PeriodicTasksTypes) -> Result<Self, TimerGuardError> {
        mutate_active_tasks(|active_tasks| {
            if !active_tasks.insert(task) {
                return Err(TimerGuardError::AlreadyProcessing);
            }
            Ok(Self { task })
        })
    }
}

impl Drop for TimerGuard {
    fn drop(&mut self) {
        mutate_active_tasks(|active_tasks| {
            active_tasks.remove(&self.task);
        });
    }
}
