Usually the moment I make this kind of list is the moment I never continue any actual work on a hobby project XD.
Let's see if that law holds once more.

<hr />

## Backlog

### Features

- Add concept of SyncGroup (opposing to system can only monitor one dir - we can monitor any amount of dirs separate
  from each other) -> this is a major refac that introduces a lot of meta challenges eg:
    - Find appropriate SyncGroup id for client&server (String?) - eg. SyncGroup1 = "obisidian_vault", SyncGroup2 = "
      ebooks", ...
    - Incorporate SyncGroup idea into server
        - History/EventSourcing
        - any Path resolving and endpoint (part of matchable-path or separate? probably better not to mingle and keep
          separate)
        - Add SyncGroup overview endpoint
    - Incorporate SyncGroup idea into client
        - rework config to support multiple directories / SyncGroups
          - rethink config -> WIP⚙: moving to server - remote conf of all clients + easier watchgroup-overview & assignment
        - add SyncGroup to client's file-watch loop
        - add SyncGroup to client's upload & sync loop
- add real DB for server (maybe ~~DuckDb?~~ or SQLite✅)
    - add more metrics (up/download per day etc.)
- implement backup strategy (currently mocked TODOs)
- switch to https (see [axum example](https://github.com/tokio-rs/axum/blob/main/examples/tls-rustls/src/main.rs)
  and [mkcert](https://github.com/FiloSottile/mkcert))

### Bugs

- cleanup of temp files (not every exit point of upload-endpoint cleans up the temp file - every ? and return has to be
  respected - probably best to add a custom transaction wrapper)
- improper panics for traversal attacks in `./shared/src/matchable_path.rs` should be caught and propagated with Error (
  TryFrom)
- no content-type header for /download responses ([needed?](https://www.relevance.com/wp-content/uploads/2014/11/Aint-nobody-got-time-for-that.jpg))

<hr />

## Open

- Filter by path/regex/gitignore-syntax? (eg. avoid syncing ".obsidian" when monitoring obsidian vault)
