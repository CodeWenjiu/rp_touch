# Repository Guidelines

## Project Structure & Module Organization
- `src/main.rs`: single embedded entrypoint (`#![no_std]`, `#![no_main]`) for RP235x + Embassy USB CDC logic.
- `.cargo/config.toml`: cross-compilation target and default runner (`picotool load ...`) used by `cargo run`.
- `memory.x`: linker memory layout for flash/RAM regions on the target board.
- `build.rs`: copies `memory.x` into build output and sets linker args (`link.x`, `defmt.x`).
- `term.bat`: Windows helper to flash an ELF and open serial (`tio COM12`).
- `target/`: build artifacts only; do not commit generated binaries from this directory.

## Build, Test, and Development Commands
- `cargo check` - fast compile validation without producing a final binary.
- `cargo build --release` - builds optimized firmware for `thumbv8m.main-none-eabihf`.
- `cargo run --release` - builds and flashes via configured runner (`picotool`).
- `cargo fmt` - applies standard Rust formatting.
- `cargo clippy --all-targets -- -D warnings` - lint gate before submitting changes.
- `term.bat target\\thumbv8m.main-none-eabihf\\release\\rp_touch` - flash + open serial monitor on Windows.

## Coding Style & Naming Conventions
- Follow idiomatic Rust: 4-space indentation, `snake_case` for functions/variables, `PascalCase` for types, `UPPER_SNAKE_CASE` for statics/constants.
- Keep async Embassy tasks focused and small; isolate hardware setup from loop logic where possible.
- Prefer `defmt` logging (`info!`, `error!`) for runtime diagnostics on-device.
- Run `cargo fmt` and fix Clippy warnings before creating a PR.

## Testing Guidelines
- There is no dedicated test suite yet; rely on compile/lint + hardware smoke tests.
- Minimum validation for each change:
  1) `cargo check`
  2) `cargo clippy --all-targets -- -D warnings`
  3) Flash and verify USB CDC behavior on device (connect, send, echo, disconnect).
- When adding pure logic modules, add unit tests in the same file with `#[cfg(test)]`.

## Commit & Pull Request Guidelines
- Current history uses short, lowercase commit subjects (for example: `init`, `chore`); keep subjects concise and imperative.
- Prefer one logical change per commit; include why in the body when touching linker, memory, or runner config.
- PRs should include: purpose, key files changed, hardware used (board + host OS), and validation evidence (command output or serial log snippets).
