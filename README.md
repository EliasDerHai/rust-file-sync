## Release

### Server
Got a little bash script for deployment `./deploy/deploy_server.sh` or manually
1. cross_compile (eg. [cross](https://github.com/cross-rs/cross)) `cross build -p server --release --target=aarch64-unknown-linux-gnu`
2. stop current service on pi (`systemctl stop ...`)
3. upload & overwrite old binary ...
4. start service (`systemctl start ...`) 
5. profit (or new bugs)

### Client


## Idea

file sync for obsidian or exchange for files?
server should be on pi
and in rust (obviously)

Maybe something like [Syncthing](https://github.com/syncthing/syncthing) - but definitly not in go 😆

Server:

- Axum
- SqLite or maybe MongoDb - what about just a plain .txt with event-sourcing entries?

Clients:

- Tauri
- SolidJs

clients can't be pure web-apps, bc I want to automatically sink files and full access over the filesystem
maybe finally my chance to wipe out [Tauri](https://tauri.app/) ?

hm on second thought - why would I need a second frontend...

obv. a lot of similar community plugins exist - eg.:

- [live-sync](https://github.com/vrtmrz/obsidian-livesync)
- [remotely-save](https://github.com/remotely-save/remotely-save)
- or probably the best option [git](https://github.com/Vinzent03/obsidian-git)

for the UI probably writing a similar plugin would make the best UX (see [obsidians docs](https://docs.obsidian.md/))

## Details

### Client's Requirements

- watches files under tracking
- diffs them (possibly file-size, last-updated date, etc.)
- recognizes change and communicates the change as precise as possible to the server (for create and update this
  includes an upload of the new data)

### Server's Requirements

- owns master data (meaning he has the data that is considered origin)
- keeps record of every change across the system (event-sourcing)
- comes up with a strategy on how to flash the current "true" state onto a device that is out of sync (*the tricky
  part*)

