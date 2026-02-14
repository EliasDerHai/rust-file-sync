<img src="./docs/logo.png" width="200" />

Cross-platform file sync via Raspberry Pi. Written in Rust.

Tested on Arch, openSUSE, Windows, macOS and Raspbian.

## About

Allows tracking of "watch-groups" across several devices - most recent version will be pushed to all clients.

Example watchgroup:
 - obsidian-vault
Example clients:
 - desktop (arch) : obsidian-vault
 - macbook (home) : obsidian-vault
 - macbook (work) : obsidian-vault

-> background service syncs file changes from eg. home mac to his peers.

https://github.com/user-attachments/assets/7ea8d0f1-770a-4b86-ad06-4c39bf338c8c

+ some additional features for monitoring, local link-share incl. pwa with mobile support (local first) & whatever else I might come up with :D 

## Run

Server:
```bash
# one time
./setup.sh 
./run_server.sh
```

Client:
```bash
cp ./config.yaml.template config.yaml
```

```bash
cargo run -p client
```

## Deploy

check `./deploy/` there are scripts for clien & server. 
make sure to adapt them to your needs (eg. server deployment to pi assumes ssh setup)

Run as OS service: systemctl (Linux), nssm (Windows), launchctl (macOS).
