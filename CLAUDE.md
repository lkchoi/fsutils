# CLAUDE.md

## Project structure

Cargo workspace of macOS file utility CLI tools. Each tool is its own crate with `lib.rs` (logic) and `main.rs` (CLI). The `fsutils` crate re-exports all tools as subcommands.

Crates: mvsum, ddup, dss, ssim, fix-ext, fsutils.

## Conventions

- Rust 2024 edition.
- All CLIs use clap derive. The `run()` function in `lib.rs` returns an `i32` error count; `main.rs` calls `exit(1)` on nonzero.
- Tests live in `lib.rs` (`#[cfg(test)] mod tests`), not separate test files. Use `tempfile` for integration tests.

## Adding a new crate

When adding a new tool, update all of:
1. `Cargo.toml` workspace members
2. `install.sh` tool loop
3. `fsutils/src/main.rs` subcommand wiring
4. `README.md` tool table

## Building

`cargo build --release` must be run to update the binaries in `target/release/`.

`install.sh` builds release and symlinks all tool binaries into `~/bin`.

## Platform

macOS-only. Tools rely on xattr, Finder metadata, `sips`, `file`, and `cwebp`.
