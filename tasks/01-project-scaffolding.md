# Task 01: Project scaffolding and configuration

## Goal
Establish the core Rust workspace/binary layout, configuration loading, and baseline build/run tooling for the risky-proxmox-agent service.

## Steps
1. **Create the Rust binary crate**
   - Initialize a binary crate for the server (e.g., `crates/server` or root `src/main.rs`).
   - Add `Cargo.toml` entries for dependencies: web framework (e.g., `axum`), async runtime (`tokio`), config (`dotenvy`), logging (`tracing`), HTTP client for Proxmox API (`reqwest`), and template embedding (`askama` or `include_str!`).
2. **Define configuration model**
   - Create a `config` module (e.g., `src/config.rs`).
   - Load `.env` values for `PVE_HOST`, `PVE_TOKEN_ID`, `PVE_TOKEN_SECRET`, and `PVE_INSECURE_SSL` (accept self-signed).
   - Add CLI args for `--bind` and `--port` (using `clap`).
3. **Wire logging/tracing**
   - Initialize `tracing_subscriber` in `main` with env-based log level.
4. **Add build/run docs placeholders**
   - Create `INSTALLING.md` and `RUNNING.md` with placeholder sections and commands to update later.
5. **Add systemd unit example**
   - Create `systemd/risky-proxmox-agent.service` with placeholders for binary path, environment file, and user.

## Deliverables
- `Cargo.toml` + `src/main.rs` (or crate layout).
- `src/config.rs` for env/CLI config.
- `INSTALLING.md`, `RUNNING.md`, `systemd/risky-proxmox-agent.service`.
