# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-08-16

### Added

- Ship as a Rust library alongside the CLI binary.
  - High-level `Manager` / `ManagerBuilder` facade for `add` / `list` / `remove` / `update`.
  - Pure-data request/outcome types; the library never prints or calls `process::exit`.
  - Low-level `core` primitives re-exported at the crate root.
- MIT OR Apache-2.0 dual licensing (`LICENSE-MIT`, `LICENSE-APACHE`).
- GitHub Actions CI (test / clippy / fmt + MSRV) and crates.io release workflows.
- `tests/lib_api.rs` proving the library works independently of the CLI.

### Changed

- Moved business logic out of `commands/` into the library (`core/` + `manager`).
- CLI `commands/` is now a thin rendering layer over the library.
- `list --json` now serializes the library's `ListedSkill` type (single source of truth).
