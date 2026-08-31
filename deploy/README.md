# Deployment

## Scripts

| Script | Target |
|---|---|
| `deploy_server.sh` | Raspberry Pi (aarch64 Linux) via SSH |
| `deploy_client_mac.sh` | macOS, managed via launchd |
| `deploy_client_linux.sh` | Linux, managed via systemd user service |
| `deploy_client_windows.sh` | Windows, managed via nssm |

Each client script handles first-time setup automatically (creates directories, copies the config template, registers the service).

### `deploy_server.sh`

Prompts for how to produce the server binary, then stops the remote service,
uploads, restarts, and polls `/ping`. All three paths target
`aarch64-unknown-linux-musl` (statically linked), so the binaries are
interchangeable:

1. **GitHub artifact (latest release)** — downloads the latest `server-linux-aarch64`
   release asset with `gh`. Fastest, needs no local toolchain.
   The frontend is already embedded in the artifact by CI (see below), so there
   is no local web build. Deploys a **released** version, so run `release.sh`
   first.
2. **Build locally** — plain `cargo build --target aarch64-unknown-linux-musl`,
   using the cross-linker configured in `.cargo/config.toml`
   (`aarch64-linux-musl-gcc`). Prompts for a version bump and deploys your 
   current working tree.
3. **Build with docker (cross)** — `cross` build (docker). Slowest but most
   hermetic. Prompts for a version bump and deploys your current working tree.

The frontend (`web/dist/`) is embedded into the server binary at compile time
via `rust-embed`, so `trunk build --release` runs before the server compiles on
the local/docker paths, and CI's release build embeds it into the published
artifact for the GitHub-artifact path.

## Server TLS

The server itself speaks plain HTTP on `127.0.0.1:<PORT>`.
I recommend tailscale with **`tailscale serve`** as:

**Prerequisite (one-time, per tailnet):** HTTPS Certificates must be enabled in the
[admin console → DNS tab](https://login.tailscale.com/admin/dns).

**One-time setup on the Pi:**
```bash
sudo tailscale set --operator=$USER   # optional, avoids sudo below
tailscale serve --bg --https=3000 http://127.0.0.1:3000
tailscale serve status                # confirm the mapping
```
This persists across reboots — `tailscaled` keeps the serve config, so it doesn't need to be
re-run after a Pi restart or a server redeploy.

To remove it or start over: `tailscale serve reset`.
