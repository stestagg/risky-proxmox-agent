# Running Risky Proxmox Agent

## Environment Variables
Set the required Proxmox configuration:

```bash
export PVE_HOST="https://proxmox.example.com:8006"
export PVE_TOKEN_ID="root@pam!token-id"
export PVE_TOKEN_SECRET="your-secret"
export PVE_INSECURE_SSL="false"
```

## Run the Server
```bash
cargo run -- --bind 0.0.0.0 --port 8080
```

## Notes
- Replace the values above with your real Proxmox credentials.
- Update this document with production runbooks as needed.
