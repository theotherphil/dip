//! This file contains the whole framework implementation, except for some logging code in events.rs.
//! It is intended to be readable from top to bottom.

use std::fmt::Debug;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

// The `event` module contains logging code only - it can safely be ignored when reading this file.
pub mod event;
use event::{Event, EventLogger};

// Salsa supports custom key and value types for queries.
// Dip does not - all keys must be of type `Key`, and all outputs must be of type `Value`.
pub type Value = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Void,
    Int(i32),
}

// Some From/Into impls for `Key`, as a minor concession to user ergonomics.
impl From<()> for Key {
    fn from(_: ()) -> Self {
        Key::Void
    }
}
impl From<i32> for Key {
    fn from(x: i32) -> Self {
        Key::Int(x)
    }
}
impl From<Key> for () {
    fn from(key: Key) -> () {
        match key {
            Key::Void => (),
            _ => panic!("Key type mismatch"),
        }
    }
}
impl From<Key> for i32 {
    fn from(key: Key) -> i32 {
        match key {
            Key::Int(x) => x,
            _ => panic!("Key type mismatch"),
        }
    }
}

/// A `Database` needs to know about all possible queries at the point where it is constructed.
///
/// Queries come in two varieties:
/// * Input queries, whose values are set explicitly by the user.
/// * Derived queries, whose values are computed from other queries.
///
/// Both kinds of queries are identified by `QueryIds`.
pub type QueryId = &'static str;

/// A `Slot` identifies a location in which to cache a query result.
/// Every query takes a `Key` as input, and to uniquely identify a query evaluation
/// you need to know both the id of the query and the inputs used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Slot {
    id: QueryId,
    key: Key,
}

impl Slot {
    fn new(id: QueryId, key: Key) -> Self {
        Self { id, key }
    }
}

/// The output of a query, together with the information needed to work out whether its value is still valid.
#[derive(Debug, Clone)]
struct Memo {
    /// The output of the query.
    value: Value,
    /// When the user sets the value for an input query the database revision increases.
    ///
    /// This field tells us the most recent revision at which we validated the contents of this memo.
    /// If `verified_at == db.revision` then we know that the `Memo` is valid.
    verified_at: usize,
    /// The last revision at which the value of the memo in this slot changed. When rerunning a query
    /// we only update `changed_at` if the output value has actually changed.
    changed_at: usize,
    /// The other queries (and keys used when calling those queries) we used to compute this value.
    ///
    /// The value of a memo is guaranteed to be valid if none of its dependencies have changed since it
    /// was last verified (as queries are required to be pure functions).
    ///
    /// If the values of any dependencies have changed since this memo was verified then the value in
    /// this memo is no longer valid and we need to recompute it to see if its value has changed.
    dependencies: HashSet<Slot>,
}

/// A query output, together with the latest revision at which the output of this query changed.
struct StampedValue {
    value: Value,
    changed_at: usize,
}

impl StampedValue {
    fn new(value: Value, changed_at: usize) -> Self {
        Self { value, changed_at }
    }
}

/// Where everything happens.
///
/// A `Database` tracks the dependencies between queries, caches results, and contains
/// the logic to determine when cached results need to be recomputed.
pub struct Database {
    /// The ids of inputs queries, i.e. those whose values will be set directly by the user
    /// rather than computed from the values of other queries.
    input_ids: Vec<QueryId>,
    /// The functions used to compute the values for derived queries.
    query_functions: HashMap<QueryId, Box<fn(&mut Database, Key) -> Value>>,
    /// Cached query results, for both input and derived queries.
    storage: HashMap<Slot, Memo>,
    /// The database revision is updated every time the user sets a value for an input query.
    revision: usize,
    /// When running queries (or when checking whether a cached result is still valid), the
    /// database will evaluate other queries.
    ///
    /// When evaluating a query we add this call (i.e. the (id, key) pair) to the top (i.e. last)
    /// element in the `active_queries` stack to record the dependency, and then push a fresh
    /// hash set onto the stack for the newly active query.
    active_queries: Vec<HashSet<Slot>>,
    /// Logs information about query execution to the console.
    /// Run `cargo run --example walkthrough` to see example output.
    logger: EventLogger,
}

// A helper macro to reduce the verbosity of event logging inside methods in `Database`.
// You can safely ignore this macro, as well as all uses of it inside `Database`.
macro_rules! event {
    ($self:expr, $event:path) => {{
        let event = $event;
        $self.logger.log_event(&event)
    }};
    ($self:expr, $event:path, $($arg:expr),*) => {{
        let event = $event($($arg.clone()),*);
        $self.logger.log_event(&event)
    }}
}

impl Database {
    /// `Database` needs to know about all the queries that it will be executing at construction.
    pub fn new(
        input_ids: Vec<QueryId>,
        query_functions: HashMap<QueryId, Box<fn(&mut Database, Key) -> Value>>,
    ) -> Database {
        Database {
            input_ids,
            query_functions,
            storage: HashMap::new(),
            revision: 0,
            active_queries: vec![],
            logger: EventLogger::new(),
        }
    }

    /// Sets the user-provided value for an input query.
    ///
    /// The `IntoKey` bound is just to make this slightly more ergonomic - users can pass
    /// `()` or an `i32` rather than needing to wrap these in a `Key` themselves.
    pub fn set<K: Into<Key>>(&mut self, id: QueryId, key: K, value: Value) {
        assert!(
            self.input_ids.contains(&id),
            "{} is not a valid input id",
            id
        );

        // Storage is indexed by slots - a query call is identified by a query id
        // and a key. Note that input queries also take a key, but a key of Key::Void
        // may be used for (input or derived) queries which logically take no key values.
        let slot = Slot::new(id, key.into());

        // As all query functions are pure, the only way for database state to change is
        // in response to this method being called. Each time an input is set we update
        // the database revision.
        self.revision = self.revision + 1;

        event!(self, Event::Set, slot, value, self.revision);

        // If a memo exists and the new value is the same as the old value then don't
        // update `changed_at`.
        let changed_at = self.read_memo(slot)
            .filter(|m| m.value == value)
            .map(|m| m.changed_at)
            .unwrap_or(self.revision);

        // Input queries do not depend on any other queries, so their dependency sets are
        // always empty.
        let memo = Memo {
            value,
            verified_at: self.revision,
            changed_at,
            dependencies: HashSet::new(),
        };

        // Helper method that stores the memo in `self.storage` and emits an Event reporting this.
        self.store_memo(slot, memo);
    }

    /// Computes or looks up the value for a query. This method is used for both input and derived queries.
    pub fn get<K: Into<Key>>(&mut self, id: QueryId, key: K) -> Value {
        self.get_with_timestamp(Slot::new(id, key.into())).value
    }

    /// Computes or looks up the value for a query and returns the value along with the database revision
    /// at which this value last changed.
    fn get_with_timestamp(&mut self, slot: Slot) -> StampedValue {
        event!(self, Event::Get, slot);

        // If we called into this method as part of computing or validating the output for another query
        // then record this call as a dependency of the parent query.
        //
        // When we store a `Memo` with the output of a query we read its dependencies from `active_queries`
        // and store them in the memo.
        if let Some(active) = self.active_queries.last_mut() {
            active.insert(slot);
        }

        // Make this the currently active query.
        self.push_active_query();

        // This `read` method could be inlined here. The only reason for not doing this is to remove the
        // need to call `pop_active_query` at each early return location from that method.
        let result = self.read(slot);

        // Remove the top element of `active_queries` now that we're done with it.
        self.pop_active_query();

        result
    }

    /// The body of `get_with_timestamp` after recording this query as a dependency of the parent query (if any)
    /// and pushing a new entry onto the active query stack.
    fn read(&mut self, slot: Slot) -> StampedValue {
        // Helper method that queries `self.storage` for a memo in this slot and emits an Event reporting this.
        let memo = self.read_memo(slot);

        if self.is_input_query(slot.id) {
            // If this is an input query then we require the user to have provided a value via `.set(..)`.
            let memo = memo.expect("attempting to query an input slot that has not been set");

            event!(self, Event::MemoForInputQuery);

            // If this is the first read of this input at the current revision then update the memo to reflect this.
            // Note that memoised values for inputs are always valid - they can't be invalidated by changes to the
            // values of any other queries.
            //
            // Aside, short version:
            //      We could have chosen to handle input queries in any of several other basically equivalent ways.
            //
            // Aside, longer version:
            //      If you're wondering why we care about `verified_at` for inputs when we've just stated that input
            //      `Memo`s are always valid, the answer is that it doesn't really matter either way.
            //
            //      The only significance of updating `verified_at` here is that it avoids the recursive call into 
            //      `get_with_timestamp` inside the `has_changed_since` method below. This has no effect on the
            //      the set of query functions that get run, but saves a bit of pushing to and popping from the active
            //      query stack. We could also have chosen to special case inputs inside `has_changed_since`, or to
            //      update `verified_at` for all inputs whenever a new value is set for _any_ input query, or made the
            //      field optional and omitted it for inputs, or chosen from a range of yet other possibilities, without
            //      changing the algorithm or calculations performed in any material way.
            //
            //      We could also have noted that `any_inputs_have_changed` as defined below would always be false
            //      for inputs and that special casing inputs is not strictly necessary. But handling
            //      inputs separately seemed slightly clearer.
            //
            if memo.verified_at != self.revision {
                let new_memo = Memo {
                    verified_at: self.revision,
                    ..memo
                };
                self.store_memo(slot, new_memo);
            }

            return StampedValue::new(memo.value, memo.changed_at);
        }

        // If we have a memo and this isn't an input query then we need to check if the memoized value is still valid.
        if let Some(memo) = memo.clone() {
            // If we've verified the memo already at this revision then it must be usable.
            if memo.verified_at == self.revision {
                event!(self, Event::MemoVerifiedAtCurrentRevision);
                return StampedValue::new(memo.value, memo.changed_at);
            }

            // Otherwise, we need to check the dependencies of the memo to see if any of their values have changed
            // since the memo was last verified.
            event!(self, Event::StartedInputChecks, memo.verified_at);

            let any_inputs_have_changed = memo
                .dependencies
                .iter()
                .any(|&input| self.has_changed_since(input, memo.verified_at));

            event!(self, Event::CompletedInputChecks, any_inputs_have_changed);

            // If the values used by when computing this memo have not changed this the memo is still valid
            // and we can update the memo's `verified_at` field and return from this method.
            //
            // Otherwise we fall through to the code after this block that recomputes the memo using its query
            // function.
            if !any_inputs_have_changed {
                let new_memo = Memo {
                    verified_at: self.revision,
                    ..memo
                };
                self.store_memo(slot, new_memo);
                return StampedValue::new(memo.value, memo.changed_at);
            }
        }

        // If we got to this point then either we don't have a memoised value or it's out of date.
        // In either case we need to evaluate the query function.
        let new_value = self.run_query_function(slot);

        // Some logging.
        if let Some(memo) = memo.clone() {
            event!(self, Event::ValueComparison, memo.value, new_value, self.revision);
        }

        // If we had a memo before and the query's value hasn't actually changed then
        // we don't update `changed_at`.
        let changed_at = memo
            .filter(|m| m.value == new_value)
            .map(|m| m.changed_at)
            .unwrap_or(self.revision);

        // Store the new memo, recording its dependencies by reading from the top element of from `active_queries`.
        let memo = Memo {
            value: new_value,
            verified_at: self.revision,
            changed_at,
            dependencies: self.active_queries.last().unwrap().clone(),
        };

        self.store_memo(slot, memo);
        StampedValue::new(new_value, changed_at)
    }

    /// Checks whether the output for a query has changed since the specified revision.
    ///
    /// If we have an up to date memo for this (query, key) pair then we can use the `changed_at` field
    /// from the memo. Otherwise, we need to recurse into `get_with_timestamp` to get a `StampedValue`
    /// for this slot.
    ///
    /// (Reminder of the state of the call stack if that happens:
    ///     get_with_timestamp(query_one)
    ///     -> read(query_one)
    ///         -> has_changed_since(query_that_query_one_depends_on)
    ///             -> get_with_timestamp(query_that_query_one_depends_on)
    ///                 -> ...
    /// )
    fn has_changed_since(&mut self, slot: Slot, revision: usize) -> bool {
        let changed_at = {
            // If we _did_ have a mechanism for removing cached valued then we would return self.revision here if no memo existed.
            let memo = self.storage.get(&slot).expect(
                "previously queried values always exist as we never remove anything from our cache",
            );

            // If we've verified the memo this revision then we can trust its changed_at field.
            if memo.verified_at == self.revision {
                memo.changed_at
            // If we've not verified the memo this revision then we need to recurse.
            } else {
                self.get_with_timestamp(slot).changed_at
            }
        };
        event!(self, Event::ChangedAt, slot, changed_at);
        changed_at > revision
    }

    /// Find the query function with id `slot.id` and run it. 
    /// Recall that query functions have signature `fn(&mut Database, Key) -> Value`.
    /// See `one_year_fee_query` in examples/walkthrough.rs for an example.
    fn run_query_function(&mut self, slot: Slot) -> Value {
        event!(self, Event::StartedQueryEvaluation);
        let query = self
            .query_functions
            .get(slot.id)
            .expect("Missing query function")
            .clone();
        let new_value = query(self, slot.key);
        event!(self, Event::CompletedQueryEvaluation);
        new_value
    }

    fn push_active_query(&mut self) {
        event!(self, Event::PushActiveQuery);
        self.active_queries.push(HashSet::new());
    }

    fn pop_active_query(&mut self) -> Option<HashSet<Slot>> {
        event!(self, Event::PopActiveQuery);
        self.active_queries.pop()
    }

    fn store_memo(&mut self, slot: Slot, memo: Memo) {
        // The read of the existing memo value here is solely to let us generate more helpful logs.
        let old_memo = self.storage.get(&slot).cloned();
        event!(self, Event::StoreMemo, old_memo, memo);
        self.storage.insert(slot, memo);
    }

    fn read_memo(&mut self, slot: Slot) -> Option<Memo> {
        let value = self.storage.get(&slot).cloned();
        event!(self, Event::ReadMemo, value);
        value
    }

    fn is_input_query(&self, id: QueryId) -> bool {
        self.input_ids.contains(&id)
    }
}
