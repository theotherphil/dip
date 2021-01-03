//! An `Event` type, and helper functions to log these to the console.
//! These are solely for debugging and tracing purposes - they do not affect query evaluation.

use crate::{Key, Memo, Slot, Value};
use std::fmt::Write;

/// `Database` is currently hardcoded to use `EventLogger` to log these events to the console.
#[derive(Clone)]
pub(crate) enum Event {
    Set(Slot, Value, usize),
    Get(Slot),
    StartedQueryEvaluation,
    CompletedQueryEvaluation,
    StoreMemo(Option<Memo>, Memo),
    ReadMemo(Option<Memo>),
    MemoForInputQuery,
    MemoVerifiedAtCurrentRevision,
    ValueComparison(Value, Value, usize),
    StartedInputChecks(usize),
    CompletedInputChecks(bool),
    ChangedAt(Slot, usize),
    PushActiveQuery,
    PopActiveQuery,
}

/// Logs `Events` to the console.
pub(crate) struct EventLogger {
    indent: usize,
}

/// Helper macro used in `EventLogger` to make it slightly less verbose to log indented lines.
macro_rules! log {
    ($self:expr, $($arg:tt)+) => {{
        print!("{}", Self::TAB.repeat($self.indent));
        println!($($arg)+)
    }}
}

impl EventLogger {
    pub(crate) fn new() -> EventLogger {
        EventLogger { indent: 0 }
    }

    /// Logs an `Event` to the console.
    pub(crate) fn log_event(&mut self, event: &Event) {
        match event {
            Event::Set(slot, value, revision) => {
                log!(
                    self,
                    "Setting ({}, {}) to {}",
                    slot.id,
                    print_key(&slot.key),
                    value
                );
                log!(self, "Global revision is now {}", revision);
            }
            Event::Get(slot) => {
                log!(self, "Query {}", print_slot_as_function_call(slot));
            }
            Event::StartedQueryEvaluation => {
                log!(self, "Running query function");
                self.indent += 1;
            }
            Event::CompletedQueryEvaluation => {
                self.indent -= 1;
            }
            Event::StoreMemo(old_memo, memo) => {
                if old_memo.is_some() {
                    log!(self, "Updating stored memo to: {}", print_memo(memo))
                } else {
                    log!(self, "Storing memo: {}", print_memo(memo))
                }
            }
            Event::ReadMemo(memo) => {
                match memo {
                    Some(memo) => log!(self, "Existing memo: {}", print_memo(memo)),
                    None => log!(self, "No memo currently exists"),
                };
            }
            Event::ValueComparison(old_value, new_value, current_revision) => {
                let result = match old_value == new_value {
                    true => format!(
                        "New value {} is the same as the memo value, so not updating changed_at",
                        new_value
                    ),
                    false => format!(
                        "New value {} != memo value {}, so updating changed_at to {}",
                        new_value, old_value, current_revision
                    ),
                };
                log!(self, "{}", result);
            }
            Event::StartedInputChecks(verified_at) => {
                log!(
                    self,
                    "Checking inputs to see if any have changed since revision {}, when this memo was last verified",
                    verified_at
                );
                self.indent += 1;
            }
            Event::CompletedInputChecks(any_inputs_have_changed) => {
                self.indent -= 1;
                let result = match any_inputs_have_changed {
                    false => "valid as no inputs have changed",
                    true => "invalid as an input has changed",
                };
                log!(self, "Memo is {}", result)
            }
            Event::MemoForInputQuery => {
                log!(self, "Memo is valid as this is an input query");
            }
            Event::MemoVerifiedAtCurrentRevision => {
                log!(
                    self,
                    "Memo is valid as it was verified at the current revision"
                );
            }
            Event::ChangedAt(slot, changed_at) => {
                log!(
                    self,
                    "Dependency {} last changed at revision {}",
                    print_slot_as_function_call(slot),
                    changed_at,
                );
            }
            Event::PushActiveQuery => {
                self.push();
            }
            Event::PopActiveQuery => {
                self.pop();
            }
        };
    }

    fn push(&mut self) {
        self.indent += 1;
    }

    fn pop(&mut self) {
        self.indent -= 1;
    }

    const TAB: &'static str = "|  ";
}

fn print_memo(memo: &Memo) -> String {
    let mut dependencies = String::new();
    write!(&mut dependencies, "{{").unwrap();
    let mut first = true;
    for dependency in &memo.dependencies {
        if !first {
            write!(&mut dependencies, ", ").unwrap();
        }
        write!(
            &mut dependencies,
            "({}, {})",
            dependency.id,
            print_key(&dependency.key)
        )
        .unwrap();
        first = false;
    }
    write!(&mut dependencies, "}}").unwrap();
    format!(
        "(value: {}, verified_at: {}, changed_at: {}, dependencies: {})",
        memo.value, memo.verified_at, memo.changed_at, dependencies
    )
}

fn print_slot_as_function_call(slot: &Slot) -> String {
    let v = match slot.key {
        Key::Void => "".to_string(),
        Key::Int(x) => x.to_string(),
    };
    format!("{}({})", slot.id, v)
}

fn print_key(key: &Key) -> String {
    let v = match key {
        Key::Void => "()".to_string(),
        Key::Int(x) => x.to_string(),
    };
    format!("{}", v)
}
