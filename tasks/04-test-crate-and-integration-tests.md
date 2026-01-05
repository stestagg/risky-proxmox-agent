# Task 04: Minimal Proxmox test crate and server tests

## Goal
Create a standalone dummy Proxmox API test crate to drive server tests without pulling in risky-proxmox-agent logic, then add server tests that use it.

## Steps
1. **Create standalone test crate**
   - Add a new crate (e.g., `crates/proxmox-dummy` or `tests/proxmox-dummy`) that exposes an HTTP API simulating a minimal Proxmox subset.
   - The crate must be independent (no references to risky-proxmox-agent modules).
2. **Dummy API surface**
   - Implement endpoints to:
     - List VMs (`GET /api2/json/nodes/{node}/qemu`).
     - Query VM status (`GET /api2/json/nodes/{node}/qemu/{vmid}/status/current`).
     - Start VM (`POST /api2/json/nodes/{node}/qemu/{vmid}/status/start`).
     - Shutdown VM (`POST /api2/json/nodes/{node}/qemu/{vmid}/status/shutdown`).
     - Stop/terminate VM (`POST /api2/json/nodes/{node}/qemu/{vmid}/status/stop`).
   - Use an in-memory state map for VM status and metadata (name, tags, notes).
3. **Minimal config/CLI**
   - Allow the dummy server to run on a random port for tests (e.g., `--bind 127.0.0.1:0`).
4. **Server crate tests**
   - Add integration tests under `crates/server/tests/` (or `tests/`) that:
     - Start the dummy API server.
     - Configure the real server to use the dummy API base URL.
     - Verify `/api/vms` returns expected data.
     - Verify launch flow: terminate easy-kill VM, start target, and status changes.

## Deliverables
- New dummy API crate with its own `Cargo.toml` and `src/main.rs` (or lib + bin).
- Integration tests for server using the dummy API.
