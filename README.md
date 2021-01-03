# Dip

A toy incremental computation framework, intended as an executable introduction to the approach used by [salsa]. The basic setup is introduced in the [salsa book]:

>The key idea of salsa is that you define your program as a set of queries. Every query is used like a function `K -> V` that maps from
>some key of type `K` to a value of type `V`. Queries come in two basic varieties:
>
>* Inputs: the base inputs to your system. You can change these whenever you like.
>* Functions: pure functions (no side effects) that transform your inputs into other values. The results of queries are memoized to
>  avoid recomputing them a lot. When you make changes to the inputs, we'll figure out (fairly intelligently) when we can re-use these
>  memoized values and when we have to recompute them.

This library implements enough of the memoization strategy from salsa to hopefully give a useful introduction to the approach used, without having to worry about all the other details that would be  required in a real framework. In particular, we make (at least) the following simplifications:
* Salsa queries can specify their own key and values types, but Dip uses a concrete enum `Key` for all query keys, and a type alias `Value = i32` for outputs.
* Salsa is thread-safe and supports query cancellation. Dip always runs queries to completion on a single thread.
* Salsa supports a range of caching and cache eviction policies. Dip caches all query outputs and never evicts anything.
* Salsa works hard to give good performance. Dip does not.
* Salsa uses procedural macros to provide a user-friendly API. Dip requires the user to do a lot of manual plumbing themselves.

The best starting point is probably to run the `walkthrough` example.

```
cargo run --example walkthrough
```

This dumps a fairly detailed trace from a series of query executions to the terminal, along with some explanatory notes.

The implementation lives entirely within `src/lib.rs`, except for some code in `src/event.rs` that is used solely for logging. `src/lib.rs` is intended to make sense when read from top to bottom.

Example output from a query evaluation (taken from the output of running the example above):

<pre>
Query one_year_fee(17)
|  Existing memo: (value: 100, verified_at: 3, changed_at: 3, dependencies: {(discount_age_limit, ()), (base_fee, ())})
|  Checking inputs to see if any have changed since revision 3, when this memo was last verified
|  |  Query discount_age_limit()
|  |  |  Existing memo: (value: 16, verified_at: 3, changed_at: 3, dependencies: {})
|  |  |  Memo is valid as this is an input query
|  |  |  Updating stored memo to: (value: 16, verified_at: 4, changed_at: 3, dependencies: {})
|  |  Dependency discount_age_limit() last changed at revision 3
|  |  Query base_fee()
|  |  |  Existing memo: (value: 100, verified_at: 3, changed_at: 1, dependencies: {})
|  |  |  Memo is valid as this is an input query
|  |  |  Updating stored memo to: (value: 100, verified_at: 4, changed_at: 1, dependencies: {})
|  |  Dependency base_fee() last changed at revision 1
|  Memo is valid as no inputs have changed
|  Updating stored memo to: (value: 100, verified_at: 4, changed_at: 3, dependencies: {(discount_age_limit, ()), (base_fee, ())})
</pre>

[salsa]: https://github.com/salsa-rs/salsa
[salsa book]: https://salsa-rs.github.io/salsa/how_salsa_works.html
