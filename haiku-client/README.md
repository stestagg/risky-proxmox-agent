# Haiku Client

This folder contains a minimal native Haiku/BeOS desktop client for the Risky Proxmox server API.

## Features

- Shows VMs from `GET /api/vms` in a native `BListView`
- Refresh button to reload inventory
- Launch selected VM via `POST /api/launch`
- Handles launch conflicts using a native `BAlert` action chooser (`shutdown`, `hibernate`, `terminate`)

## Build on Haiku

```sh
cd haiku-client
make
```

This creates `risky_proxmox_haiku`.

## Run

```sh
./risky_proxmox_haiku
```

Set the server field to your backend URL, e.g. `http://127.0.0.1:3000`.

## Notes

- Networking is intentionally simple (`curl` shell calls) to keep this starter client small.
- The parser expects JSON shapes returned by this project's `/api` endpoints.
