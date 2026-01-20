# Ledger Log

Ledger Log is a library matching the interface of the venerable Rust [Log](https://crates.io/crates/log) library.

Log already has support for pluggable backends, but not via a zero-cost abstraction.
The indirection used interferes with the Ledger's bare-bones ROPI / RWPI linking model.

Ledger Log bakes in the specific ledger IO functionality, but in doing so keeps the run time operation dead simple.
