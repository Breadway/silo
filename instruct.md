# Silo - Project Overview

> Placeholder name, easy to rename later. Standalone repo: `Breadway/silo`.

## What this is

A two-part system for cataloging game archives on **Hestia** (home server) and pulling
them down to a laptop (**Hermes**) where they get extracted and registered as
non-Steam games, launched via Proton. The whole point is to remove the manual
"SCP the archive, untar it, hunt for the .exe, open Steam, add a non-steam game,
set launch options, set the compat tool, restart Steam" loop and replace it with
one command (or one click in a local web UI).

Two binaries, one repo (Cargo workspace):

1. **`silo-server`** - runs on Hestia. Owns the SQLite catalog DB and the
   archive storage directory. Exposes a small REST/JSON API over Tailscale only
   (no public exposure via Aegis - this never needs to leave the tailnet).
2. **`silo-client`** - runs on Hermes (or any laptop on the tailnet). Talks to
   `silo-server`'s API. Serves a local web UI (binds `127.0.0.1`) and opens it
   in the default browser on startup. Handles the actual pull → extract →
   register-with-Steam flow locally, since that part is inherently
   laptop-side (Steam lives there, not on the server).

## Why this design

- **Server process over SQLite-over-network**: avoids file-locking headaches,
  gives a clean API boundary, and lets the catalog be edited from the server
  side too (e.g. `silo-server` could eventually get a CLI for headless
  add/remove without needing the client running).
- **Local web UI over GTK4**: this is a single-user, localhost-only tool. A
  small web stack is faster to build well (more LLM training exposure, easier
  styling) and sidesteps GTK packaging/theming overhead for something this
  scoped-down. Bind to `127.0.0.1` only - this is not a service that needs to
  be reachable from elsewhere.
- **rsync over Tailscale for transfer**: resumable, only moves diffs if a pull
  is interrupted/retried, and reuses infra you already have (Tailscale mesh
  between Hestia and every laptop).
- **Standalone repo, not under `bread-ecosystem`**: this isn't part of the
  Hyprland/Wayland desktop tooling ecosystem and doesn't share the pywal
  theming / pywal-driven design system. Keep it independent.

## Hard constraint: Steam shortcut mechanics (read this before writing any Steam-integration code)

This was researched up front because it changes the design:

- Non-Steam game shortcuts live in a **binary VDF** file:
  `~/.steam/steam/userdata/<SteamID3>/config/shortcuts.vdf` (Linux).
- **Steam does not hot-reload this file.** Edits are only picked up after
  Steam fully restarts. There is no way around this - no API, no signal, no
  trick. Budget for "client kills Steam, writes the file, relaunches Steam" as
  a real step in the pull flow, not an edge case.
- **Do not write `shortcuts.vdf` while Steam is running.** If Steam is open
  when the file is modified externally, Steam can overwrite or corrupt it on
  next write/exit. The client must check if Steam is running, cleanly shut it
  down (`steam -shutdown`, which blocks until exit - don't SIGKILL), *then*
  write, *then* relaunch.
- **Compat tool (Proton) mapping is a separate file**: `config.vdf`, also
  under `userdata/<id>/config/`. The appid generated for a shortcut is a hash
  derived from the exe path + display name string - if that string isn't
  byte-identical between the entry in `shortcuts.vdf` and the corresponding
  entry written into `config.vdf`'s `CompatToolMapping`, the Proton version
  pin silently won't apply, and Steam will fall back to system defaults.
  **Store the exact literal strings used (exe path, app name) in the DB,
  don't reconstruct them on the fly**, so both files are always written from
  the same source string.
- Take a backup of both VDF files before every write (timestamped copy
  alongside the original is fine). These are hand-rolled binary formats with
  no validation; a malformed write can silently delete all existing shortcuts
  on next Steam launch.
- There are existing Rust crates and reference implementations for the binary
  VDF format (search crates.io for `vdf`/`keyvalues` parsers, and look at how
  SteamTinkerLaunch and CorporalQuesadilla/Steam-Shortcut-Manager structure
  writes) - don't hand-roll the binary format parser from scratch if a
  maintained crate covers it.

## Data model

### Archive metadata (sidecar file inside each `.tar.gz`, e.g. `silo.toml`)

Only three fields needed, per your call:

```toml
exe_path = "Game/Binaries/Win64/Game-Win64-Shipping.exe"  # relative to archive root after extraction
launch_args = "-windowed -nolauncher"                      # optional, empty string if none
proton_version = "proton_9"                                 # or "GE-Proton9-20" etc - exact compat tool string Steam expects
```

- If `silo.toml` is absent, `silo-client` falls back to scanning the extracted
  tree for `.exe` files. If exactly one is found, use it (with empty launch
  args and a default/configurable Proton version). If multiple are found,
  surface a picker in the web UI rather than guessing.
- `silo-server` doesn't need to parse this file at catalog time - it's
  opaque cargo the client reads after extraction. The *catalog* DB on the
  server only needs to track what's described below.

### Server-side catalog (SQLite, e.g. `catalog.db`)

A `games` table is enough to start:

| column           | type | notes                                                  |
|------------------|------|---------------------------------------------------------|
| id               | INTEGER PK | |
| name             | TEXT | display name |
| archive_filename | TEXT | filename within the server's archive storage dir |
| size_bytes       | INTEGER | for showing transfer estimates in the UI |
| added_at         | TEXT (ISO8601) | |
| notes            | TEXT, nullable | freeform |
| last_pulled_at   | TEXT, nullable | set by client via API after a successful pull, informational only |

Deliberately *not* duplicating `exe_path`/`launch_args`/`proton_version` here -
those live in `silo.toml` inside the archive, which is the source of truth, so
there's only one place to edit them. (Open question below if you want this
reconsidered.)

## API surface (`silo-server`, axum, JSON, Tailscale-bound)

```
GET    /games                 -> list catalog entries
GET    /games/:id             -> single entry detail
POST   /games                 -> add entry (multipart upload of the .tar.gz, or
                                  a server-local path if you're dropping archives
                                  directly onto Hestia's disk - probably support both)
DELETE /games/:id              -> remove entry + delete archive from disk
PATCH  /games/:id              -> edit name/notes
POST   /games/:id/mark-pulled  -> client calls this after a successful pull, updates last_pulled_at
GET    /health                 -> trivial liveness check
```

Archive bytes themselves are fetched via **rsync**, not through the JSON API
(rsync daemon module or rsync-over-ssh against Hestia's tailnet IP, pointed at
the same archive storage directory the server manages) - the API is metadata-only.

## `silo-client` pull flow (the core user-facing loop)

1. User opens web UI (auto-launched on `silo-client` start), browses catalog
   (pulled from `silo-server` API).
2. User clicks "Pull" on a game.
3. Client runs `rsync` to fetch the archive into a local staging dir, showing
   progress in the UI.
4. Client extracts the `.tar.gz`.
5. Client looks for `silo.toml`; if absent, scans for `.exe` per the fallback
   rule above. May prompt the user via the UI if ambiguous.
6. Client checks if Steam is running; if so, shows a "closing Steam to add
   this game" notice and runs `steam -shutdown`, waiting for exit.
7. Client backs up `shortcuts.vdf` and `config.vdf`, then writes the new
   shortcut entry + compat tool mapping, using the exact stored strings.
8. Client relaunches Steam.
9. Client calls `POST /games/:id/mark-pulled` on the server.
10. UI shows success + the `steam://rungameid/...` link for convenience.

## Open questions for Claude Code to flag back rather than guess on

- Where exactly should extracted games live on Hermes - under Steam's own
  library folders (so Steam's disk-usage UI accounts for them) vs. a separate
  `~/Games/silo/` tree? Affects step 4/7 wiring.
- Multi-laptop scenario: if you ever pull the same game to more than one
  machine, should the server track *which* machines have pulled what, or is
  `last_pulled_at` (single timestamp, last writer wins) good enough? Leaning
  toward "good enough for now" given this is single-user.
- Should `silo-server` validate `silo.toml` contents at upload time (reject
  archives missing it / malformed) or stay fully opaque to archive internals
  as currently scoped? Current design says opaque - confirm that's still
  desired before building.

## Tech stack summary

- **Language**: Rust, workspace with `silo-server` and `silo-client` crates,
  matching the rest of the Breadway ecosystem's conventions (TOML config,
  systemd user service for `silo-server` on Hestia).
- **Server web framework**: axum.
- **Client web UI**: lean toward **Svelte** (small bundle, compiles away the
  framework overhead, plenty of LLM familiarity) served by the client binary
  itself (e.g. via `axum` or `tower-http` serving a built static bundle) -
  open to a different lightweight framework if there's a good reason, but
  avoid anything heavyweight (no full Next.js-style SSR stack needed for a
  single-user localhost tool).
- **DB**: SQLite (via `rusqlite` or `sqlx`).
- **Transfer**: `rsync` shelled out to, over Tailscale.
- **VDF read/write**: existing Rust crate if one adequately covers binary VDF;
  otherwise implement against the documented binary VDF spec (Valve Developer
  Community wiki + SteamTinkerLaunch's writeup are good references), with
  mandatory backup-before-write.
