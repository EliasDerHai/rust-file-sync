# Deployment

## Scripts

| Script | Target |
|---|---|
| `deploy_server.sh` | Raspberry Pi (aarch64 Linux) via SSH |
| `deploy_client_mac.sh` | macOS, managed via launchd |
| `deploy_client_linux.sh` | Linux, managed via systemd user service |
| `deploy_client_windows.sh` | Windows, managed via nssm |

Each client script handles first-time setup automatically (creates directories, copies the config template, registers the service).

## Server TLS

The server uses TLS when `TLS_CERT_PATH` and `TLS_KEY_PATH` environment variables are set; otherwise it falls back to plain HTTP. The certs on the Pi were generated with [mkcert](https://github.com/FiloSottile/mkcert) and live at `~/certs/cert.pem` and `~/certs/key.pem`.

These env vars must be set in the server's systemd unit on the Pi:

```ini
# /etc/systemd/system/rust-file-sync_server.service
[Service]
Environment="TLS_CERT_PATH=/home/pi/certs/cert.pem"
Environment="TLS_KEY_PATH=/home/pi/certs/key.pem"
```

## Trusting the mkcert CA on client machines

The server cert is signed by a local mkcert CA, not a public CA. Each client machine needs to trust that CA once.

**Fetch the CA cert from the Pi:**
```bash
scp pi@raspberrypi.local:~/.local/share/mkcert/rootCA.pem /tmp/mkcert-ca.pem
```

**macOS:**
```bash
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain /tmp/mkcert-ca.pem
```

**Linux (Debian/Ubuntu):**
```bash
sudo cp /tmp/mkcert-ca.pem /usr/local/share/ca-certificates/mkcert-raspberrypi.crt
sudo update-ca-certificates
```

**Windows (run in an elevated PowerShell):**
```powershell
Import-Certificate -FilePath "$env:TEMP\mkcert-ca.pem" -CertStoreLocation Cert:\LocalMachine\Root
```

After trusting the CA, restart the client service so it picks up the updated trust store.
