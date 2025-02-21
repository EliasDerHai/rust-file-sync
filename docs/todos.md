Usually the moment I make this kind of list is the moment I never continue any actual work on a hobby project XD.
Let's see if that law holds once more.

<hr />

## Backlog

### Features
 - Add concept of SyncGroup (opposing to system can only monitor one dir - we can monitor any amount of dirs separate from each other) -> this is a major refac that introduces a lot of meta challenges eg:
    - Find appropriate SyncGroup id for client&server (String?) - eg. SyncGroup1 = "obisidian_vault", SyncGroup2 = "ebooks", ...
    - Incorporate SyncGroup idea into server
      - History/EventSourcing
      - any Path resolving and endpoint (part of matchable-path or separate? probably better not to mingle and keep separate)
      - Add SyncGroup overview endpoint
    - Incorporate SyncGroup idea into client
      - rework config to support multiple directories / SyncGroups
      - add SyncGroup to client's file-watch loop
      - add SyncGroup to client's upload & sync loop
  - add real DB for server (maybe DuckDb? or SQLite)
      - add more metrics (up/download per day etc.)
  - implement backup strategy (currently mocked TODOs)

### Bugs
 - improper panics for traversal attacks in `./shared/src/matchable_path.rs` should be caught and propagated with Error (TryFrom)
 - wrong content-type header for /download responses (fixed "text; charset=utf-8")

<hr />

## Open
 - Filter by path/regex/gitignore-syntax? (eg. avoid syncing ".obsidian" when monitoring obsidian vault)
