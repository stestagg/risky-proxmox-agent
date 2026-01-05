# Task 03: Server routes, launch flow, and UI

## Goal
Implement the web UI and server-side launch orchestration for mutually-exclusive VM launching.

## Steps
1. **Server routes**
   - Add routes:
     - `GET /` serves the HTML/JS UI.
     - `GET /api/vms` returns VM list from Proxmox.
     - `POST /api/launch` triggers the launch flow.
2. **Embed UI assets**
   - Create `assets/index.html` and `assets/app.js` (or embed with `askama`).
   - UI shows grid of VMs with name/tags/notes and status color.
3. **Launch orchestration**
   - Implement a server-side `LaunchManager` with a single-flight lock (e.g., `tokio::sync::Mutex`).
   - Sequence:
     1) If a VM is running and has `easy-kill`, terminate.
     2) Otherwise prompt the user for action (shutdown/hibernate/terminate/cancel).
     3) Execute the action and poll until stopped.
     4) Start the requested VM.
4. **Re-entrant behavior**
   - If a launch is running and a second request chooses `terminate`, update the current action to terminate.
   - Otherwise return an error indicating a launch is in progress.
5. **Client behavior**
   - JS submits launch intent and displays confirmation dialogs for current VM action.
   - UI shows progress/errors while server-side flow runs.

## Deliverables
- Route handlers in `src/server.rs` (or equivalent).
- UI assets and minimal CSS for grid layout and running status.
