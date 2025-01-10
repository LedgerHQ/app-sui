# Developing an Alamgu ledger App

## Dependencies and development environments

### Background info

An Alamgu ledger app is mostly written in Rust, and efforts have been taken to ensure it is developed in a fairly standard way (for embedded Rust programs).

These dependencies are needed:

 - Rust toolchain with unstable features (including `rustc` and LLD the LLVM Linker).
 - C toolchain targeting freestanding ARM (including `newlib` as a rump libc)
 - ["linker wrapper" script from our fork of the SDK.](https://github.com/alamgu/ledger-nanos-sdk/blob/memory-fixes/scripts/link_wrap.sh)

Additionally for testing these are needed:

 - Python3
 - [Speculos] the official Ledger emulator
 - Ragger, a library to run tests on Speculos emulator

[Speculos]: https://github.com/ledgerHQ/speculos

### Getting a development environment with Nix

The easiest way to get all these dependencies is with Nix:

```bash
nix-shell -A $DEVICE.rustShell
cd rust-app/
cargo build --release --target=$TARGET_JSON
````
where `DEVICE` is one of
 - `nanox` for Nano X
 - `nanosplus` for Nano S+
 - `flex`, for Flex
 - `stax`, for Stax

### Getting a development environment without Nix

Exact instructions are not provided.
See the [`./docker`](./docker) subdirectory or [`GitHub Actions`](.github/workflows/rust.yml) secondary CI.

[main read-me]: ./README.md

## Updating the lock file (`Cargo.lock`)

Nix needs additional information not provided in the Cargo lock file for the most robust form of supply-chain integrity for git dependencies.
This information is contained in `crate-hashes.json` at the root of this repo.

After modifying `Cargo.lock`, please run
```
./update-crate-hashes.sh
```
in order to regenerate this file and keep it up to date.

## Running automated tests with Speculos

Using Nix, from the root level of this repo, run the following script to run tests for all devices

```bash
./run-ragger-tests.sh
```

## Deploying development builds to real hardware

The easiest thing to do is just run a Nix build as described in the [main read-me].
Nix will always track the latest changes, freshly rebuilding components as needed.

That said, it is also possible to use Cargo build.
This useful for the quickest "debug loop".

The [cargo-ledger](https://github.com/LedgerHQ/cargo-ledger.git) builds, outputs a `hex` file and a manifest file for `ledgerctl`, and loads it on a device in a single `cargo-ledger ledger -l nanosplus` command in the rust-app folder within app directory.

(You do not need to install `cargo-ledger` if you are using the nix-provided development shell, as it provides it.)

Before installing with either method, please ensure that your device is plugged, unlocked, and on the device home screen.

Note as described in the [main read-me](./README.md),
it is currently not possible to side-load apps on the on Nano X, so one can only test in the emulator.
