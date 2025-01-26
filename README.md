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
