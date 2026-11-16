# Silo

A two-part system for cataloging game archives on **Hestia** (home server) and pulling
them down to a laptop (**Hermes**), where they get extracted and registered as
non-Steam games launched via Proton. Replaces the manual "SCP the archive, untar it,
hunt for the .exe, open Steam, add a non-Steam game, set launch options, set the
compat tool, restart Steam" loop with one command (or one click in a local web UI).

Cargo workspace, two binaries:

- **silo-server** - runs on Hestia, holds the catalog
- **silo-client** - runs on Hermes, pulls archives and registers them with Steam

See `instruct.md` for the full design notes.

## Building

```bash
cargo build --release
```
