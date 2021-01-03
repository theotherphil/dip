//! In our very contrived example you own a company providing training services and need to quote a
//! subscription fee to your customers.
//!
//! The calculation is very simple: you have a fixed yearly base fee, but thanks to government
//! funding can provide a discounted price to school-aged customers.

// `salsa` generates a user-friendly API using procedural macros, but the users of `dip::Database`
// have to do most of the plumbing themselves.
//
// In our example we hide this plumbing from the "end-user" by exporting only a use-case-specific
// `CostsDatabase` trait from this nested module.
mod implementation {
    // The meaning of these types is explained in src/lib.rs, which is best read from top to bottom.
    use dip::{Database, Key, QueryId, Value};
    use std::collections::HashMap;

    // All dip queries have signature (&mut Database, Key) -> Value, but we define some type
    // aliases to make the example code easier to follow.
    type Dollars = i32;
    type Years = i32;

    // Identifiers for the queries (inputs or derived) used in our example.
    // We hide these from the end-user behind the `CostsDatabase` trait below.
    const BASE_FEE: QueryId = "base_fee";
    const DISCOUNT_AGE_LIMIT: QueryId = "discount_age_limit";
    const DISCOUNT_AMOUNT: QueryId = "discount_amount";
    const ONE_YEAR_FEE: QueryId = "one_year_fee";
    const TWO_YEAR_FEE: QueryId = "two_year_fee";

    // This trait allows for more ergonomic-looking code in this example's `main` function, but
    // has no special significance - we could equally well have written this example with
    // freestanding functions or by using the methods on `dip::Database` directly from `main`.
    //
    // The meaning of the various methods are explained in the `impl` block below.
    pub trait CostsDatabase {
        // Setting inputs
        fn set_discount_age_limit(&mut self, age_limit: Years);
        fn set_base_fee(&mut self, base_fee: Dollars);
        fn set_discount_amount(&mut self, discount_amount: Dollars);

        // Reading inputs
        fn discount_age_limit(&mut self) -> Years;
        fn base_fee(&mut self) -> Dollars;
        fn discount_amount(&mut self) -> Dollars;

        // Derived queries
        fn one_year_fee(&mut self, current_age: Years) -> Dollars;
        fn two_year_fee(&mut self, current_age: Years) -> Dollars;
    }

    impl CostsDatabase for Database {
        // The ids DISCOUNT_AGE_LIMIT, BASE_FEE and DISCOUNT_AMOUNT are registered as input ids
        // in the call to `dip::Database::new` in the `create_database` function below.
        //
        // Input queries take an `Into<Key>` as input. In our example they have no logical inputs,
        // so we use `()`.
        fn set_discount_age_limit(&mut self, age_limit: Years) {
            self.set(DISCOUNT_AGE_LIMIT, (), age_limit);
        }
        fn set_base_fee(&mut self, base_fee: Dollars) {
            self.set(BASE_FEE, (), base_fee);
        }
        fn set_discount_amount(&mut self, discount_amount: Dollars) {
            self.set(DISCOUNT_AMOUNT, (), discount_amount);
        }

        // The API for querying inputs is identical to non-input queries.
        fn discount_age_limit(&mut self) -> Years {
            self.get(DISCOUNT_AGE_LIMIT, ())
        }
        fn base_fee(&mut self) -> Dollars {
            self.get(BASE_FEE, ())
        }
        fn discount_amount(&mut self) -> Dollars {
            self.get(DISCOUNT_AMOUNT, ())
        }

        // Compute the one year membership fee for someone of the given age.
        fn one_year_fee(&mut self, current_age: Years) -> Dollars {
            self.get(ONE_YEAR_FEE, current_age)
        }

        // Compute the two year membership fee for someone of the given age.
        fn two_year_fee(&mut self, current_age: Years) -> Dollars {
            self.get(TWO_YEAR_FEE, current_age)
        }
    }

    // See comments in `create_database`.
    fn one_year_fee_query(db: &mut Database, current_age: Key) -> Dollars {
        let current_age: Years = current_age.into();

        // Customers receive a discount if they're <= the discount age limit.
        if current_age <= db.discount_age_limit() {
            db.base_fee() - db.discount_amount()
        } else {
            db.base_fee()
        }
    }

    // See comments in `create_database`.
    fn two_year_fee_query(db: &mut Database, current_age: Key) -> Dollars {
        let current_age: Years = current_age.into();

        // Compute the fees for this year and next year and add them (no loyalty discounts here).
        //
        // This is equal to `2 * one_year_fee` _unless_ you're currently at the age limit for a
        // young person's discount.
        let fee_this_year = db.one_year_fee(current_age);
        let fee_next_year = db.one_year_fee(current_age + 1);
        fee_this_year + fee_next_year
    }

    pub fn create_database() -> impl CostsDatabase {
        // Unlike in salsa, our users need to wire up all the queries themselves.
        //
        // First, we define the set of input ids. These are queries whose values must be provided
        // directly by the user.
        let input_ids = vec![BASE_FEE, DISCOUNT_AGE_LIMIT, DISCOUNT_AMOUNT];

        // Dependency tracking and memoisation is defined in terms of QueryIds. If dip determines
        // that it needs to (re)compute some value then it needs to be able to look up the
        // appropriate query function from its id. This lookup is provided directly in the
        // constructor to `Database`.
        let mut query_functions = HashMap::<QueryId, Box<fn(&mut Database, Key) -> Value>>::new();

        // Note that we only need to register functions for derived queries - no user-provided code
        // is executed when reading input queries as we just read their cached values directly.
        query_functions.insert(ONE_YEAR_FEE, Box::new(one_year_fee_query));
        query_functions.insert(TWO_YEAR_FEE, Box::new(two_year_fee_query));

        // Return a configured database and hide the plumbing from the end-users behind a trait.
        Database::new(input_ids, query_functions)
    }
}

use implementation::{create_database, CostsDatabase};

fn note(message: &str) {
    println!("\n\n****");
    for line in message.lines() {
        println!("**  {}", line.trim());
    }
    println!("**");
    println!();
}

fn main() {
    let mut db = create_database();

    note(
        r#"Contrived setup: you own a company that provides training services, and need to quote
        a subscription fee to potential customers.

        The calculation is very simple: you have a fixed yearly base fee, but thanks to government funding
        can provide a discounted price to school-aged customers.

        The Database used has three inputs:
            * base_fee()
            * discount_amount()
            * discount_age_limit()

        And two derived queries:
            * one_year_fee(age: Years) -> Dollars
            * two_year_fee(age: Years) -> Dollars

        Pseudo-code for the two derived queries:
            * one_year_fee(age) = if age <= discount_age_limit { base_fee - discount_amount } else { base_fee }
            * two_year_fee(age) = one_year_fee(age) + one_year_fee(age + 1)

        In the execution below we:
            * Set values for all of the database inputs
            * Run the derived queries for a few inputs, noting where existing results are being reused
            * Change some of the input values, rerun some derived queries and note where and why cached values require recalculation

        Output without leading '*'s is from Dip - the Database type emits Events and these are written to the terminal."#,
    );

    note(r#"Before we can query fees we need to set the input values."#);
    db.set_base_fee(100);
    db.set_discount_amount(30);
    db.set_discount_age_limit(16);

    note(
        r#"16 is the maximum age for a young person's discount, so the one year fee for a 16 year old is base_fee - discount_amount."#,
    );
    assert_eq!(db.one_year_fee(16), 70);

    note(
        r#"17 is greater than the maximum age for a young person's discount, so the one year fee for a 17 year old is base_fee."#,
    );
    assert_eq!(db.one_year_fee(17), 100);

    note(
        r#"To compute the two year fee for a 17 year old we need to know the one year fee for a 17 year old and the one year fee for
        an 18 year old. We have already computed the first of these, so will re-use the cached value for one_year_fee(17) and
        compute one_year_fee(18)."#,
    );
    assert_eq!(db.two_year_fee(17), 200);

    note(r#"Update the discount provided to people under the discount age limit."#);
    db.set_discount_amount(40);

    note(
        r#"The memo for one_year_fee(17) is out of date, as the database revision has increased since it was last verified.
        However, as neither the age limit threshold nor the base fees have changed its value is still valid."#,
    );
    assert_eq!(db.one_year_fee(17), 100);

    note(
        r#"As 16 <= discount_age_limit we will spot that one of the inputs to one_year_fee(16) has changed and have to recompute."#,
    );
    assert_eq!(db.one_year_fee(16), 60);

    note(
        r#"Government funding criteria have changed - we can now also provide discounts to 17 year olds."#,
    );
    db.set_discount_age_limit(17);

    note(
        r#"Both one_year_fee(17) and one_year_fee(18) query the age limit, so both have potentially changed - we will need
        to rerun queries to tell. The value of one_year_fee(18) does not change, but the value of one_year_fee(17) does
        and so two_year_fee(17) also needs to be recomputed."#,
    );
    assert_eq!(db.two_year_fee(17), 160);
}
