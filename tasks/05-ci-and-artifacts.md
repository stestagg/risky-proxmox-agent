# Task 05: CI workflow for tests and x86_64 Linux artifacts

## Goal
Add a GitHub Actions workflow that runs tests and produces build artifacts for x86_64 Linux.

## Steps
1. **Workflow setup**
   - Create `.github/workflows/ci.yml` to trigger on `push` and `pull_request`.
2. **Test job**
   - Use `actions/checkout` and `actions-rs`/`dtolnay/rust-toolchain` to install stable Rust.
   - Run `cargo test --all`.
3. **Build artifact job**
   - Build release binary for x86_64 Linux (`cargo build --release --target x86_64-unknown-linux-gnu`).
   - Upload the built binary as an artifact (`actions/upload-artifact`).
4. **Optional caching**
   - Add `Swatinem/rust-cache` or equivalent to speed up builds.

## Deliverables
- `.github/workflows/ci.yml` with test + artifact steps.
