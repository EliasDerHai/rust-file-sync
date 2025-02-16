Usually the moment I make this kind of list is the moment I never continue any actual work on a hobby project XD.
Let's see if that law holds once more.

## Features:
 - !!NEEDED FOR CORE FUNCTIONALITY !! Filter by file type (eg. filter .DS_Store etc. - or maybe that should be a platform specific feature for macos?)
 - Filter by path (eg. avoid syncing ".obsidian" when monitoring obsidian vault)
 - Add concept of SyncGroup (opposing to system can only monitor one dir - we can monitor any amount of dirs separate from each other) -> this is a major refac that introduces a lot of meta challenges eg:
    - Find appropriate SyncGroup id for client&server (String?) - eg. SyncGroup1 = "obisidian_vault", SyncGroup2 = "ebooks", ...
    - Encorporate SyncGroup idea into server
      - History/EventSourcing
      - any Path resolving and endpoint (part of matchable-path or separate? probably better not to mingle and keep separate)
      - Add SyncGroup overview endpoint
    - Encorporate SyncGroup idea into client
      - rework config to support multiple directories / SyncGroups
      - add SyncGroup to client's file-watch loop
      - add SyncGroup to client's upload & sync loop
