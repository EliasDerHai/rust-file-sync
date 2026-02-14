I love taking notes.<br/>
I dislike having everything in the cloud.<br/>
I tend to forget commiting pushing changes when I'm working on notes rather than code.<br/>
I enjoy tinkering on my own solutions.<br/>

That's why I Implemented this cross-plattform file sync via raspberry pi for my differen devices (tested on openSUSE, windows, mac, raspbian).<br/>

In action:

https://github.com/user-attachments/assets/7ea8d0f1-770a-4b86-ad06-4c39bf338c8c

## Run

For running dev env just:
* server: run server via `cargo run -p server`
* client: copy config.yaml.template to config.yaml with according properties & run via `cargo run -p client`

## Deploy 

Assuming setup with services on linux (pi) via systemctl, windows (nssm) and mac (launchctl).

### Server
Got a little bash script for deployment `./deploy/deploy_server.sh` or manually
1. cross_compile (eg. [cross](https://github.com/cross-rs/cross)) `cross build -p server --release --target=aarch64-unknown-linux-gnu`
2. stop current service on pi (`systemctl stop ...`)
3. upload & overwrite old binary ...
4. start service (`systemctl start ...`) 
5. profit (or new bugs)

### Client
Basically just `cargo build -p client --release` and run it as os-service.

* **for windows** use nssm (`./deploy/deploy_client_windows.sh`)
* **for mac** use launchctl (`./deploy/deploy_client_windows.sh`)

## Background / Ideas

file sync for obsidian or exchange for files?
server should be on pi
and in rust (obviously)

Maybe something like [Syncthing](https://github.com/syncthing/syncthing)?

For obsidian my problem could have been solved the easy way with:

- [live-sync](https://github.com/vrtmrz/obsidian-livesync)
- [remotely-save](https://github.com/remotely-save/remotely-save)
- or probably the best option [git](https://github.com/Vinzent03/obsidian-git)

for the UI probably writing a similar plugin would make the best UX (see [obsidians docs](https://docs.obsidian.md/))

But let's roll with this :D

Server:
- Axum
- SqLite or maybe MongoDb - what about just a plain .txt with event-sourcing?

Clients:
- thin background service to sync with server

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

