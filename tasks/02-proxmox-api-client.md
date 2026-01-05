# Task 02: Proxmox API client and VM model

## Goal
Implement a minimal Proxmox API client layer to list VMs, query status, and start/stop/halt VMs.

## Steps
1. **Create API client module**
   - Add `src/proxmox/mod.rs` with a `ProxmoxClient` struct that stores base URL and auth token.
   - Use `reqwest` with `rustls` and an option to accept invalid certs when `PVE_INSECURE_SSL=true`.
2. **Define VM data model**
   - Create `src/proxmox/types.rs` with a `VmInfo` struct: VM ID, name, tags, status, and notes.
   - Parse tags into a `Vec<String>` and normalize status (`running`, `stopped`, `unknown`).
3. **List VMs**
   - Implement `list_vms()` that calls Proxmox API endpoints (node → qemu list) and maps to `VmInfo`.
4. **VM actions**
   - Implement methods: `start_vm(vmid)`, `stop_vm(vmid)` (graceful), `shutdown_vm(vmid)` (alias if needed), `hibernate_vm(vmid)` if supported, and `terminate_vm(vmid)` (stop/kill).
   - Add `vm_status(vmid)` that returns `running/stopped`.
5. **Error handling**
   - Define an error enum in `src/proxmox/error.rs` to wrap HTTP and parse errors.

## Deliverables
- `src/proxmox/mod.rs`, `src/proxmox/types.rs`, `src/proxmox/error.rs`.
- Unit tests for parsing VM tags/notes if possible.
