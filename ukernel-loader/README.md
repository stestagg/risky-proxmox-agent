# ukernel-loader

`ukernel-loader` is a Unikraft-based console binary that mirrors the core flow of the Haiku client:

1. On start, fetches VM inventory from a hard-coded API URL.
2. Prints VMs to the console.
3. Lets the user select a VM to launch via `/api/launch`.
4. Allows shutting down the Proxmox host via `/api/host-shutdown`.

If the API reports `needs_action` (another VM is currently active), `ukernel-loader` retries with `"action":"terminate"`, which matches the expected flow where this loader is the currently running VM.

## API URL

The loader currently uses:

- `http://10.0.2.2:3000`

Update `API_BASE_URL` in `src/main.c` for your environment.

## Build/run (KraftKit)

From this directory:

```bash
kraft build
kraft run
```

At runtime, use:

- `number` → launch VM
- `r` → refresh list
- `s` → request host shutdown
- `q` → quit

## Notes

- This app is intentionally console-only (no persistence and no filesystem requirements).
- Networking is required to reach the HTTP API.
