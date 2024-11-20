use crate::state::mutate_state;

use crate::ledger_suite_manager::PeriodicTasksTypes;

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
        mutate_state(|s| {
            if !s.active_tasks.insert(task) {
                return Err(TimerGuardError::AlreadyProcessing);
            }
            Ok(Self { task })
        })
    }
}

impl Drop for TimerGuard {
    fn drop(&mut self) {
        mutate_state(|s| {
            s.active_tasks.remove(&self.task);
        });
    }
}
