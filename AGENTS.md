# AGENTS.md — RUZE (crate `meow`)

Discord ↔ Minecraft bridge in Rust (edition 2024) using `poise` + `serenity` on Tokio.
Tails the MC server `latest.log` + drives it via RCON, with zero mods on the server.

## Run / verify
- `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt` (run before committing), `cargo run`.
- No CI. Tests live in `src/storage.rs` (round-trip/corrupt-file for the JSON store) — `mc_log.rs` still has none; add table-driven tests for `parse_log_line` there.

## Layout & flow (read `main.rs` first)
- `main.rs` wires two mpsc channels and the RCON client:
  - `mc_log.rs`: tails the log; `parse_log_line` regex → `MinecraftEvent` → `FromMinecraftEvent` (mapping in `event_handler.rs`) → sends to bot.
  - `event_handler.rs`: `spawn_discord_to_mc_relay` takes `FromDiscordEvent` and sends `tellraw @a` (gold, "[Discord] <user>: msg") via RCON.
  - `discord_bot/dc_bot.rs` (`start_discord_bot`): builds framework, spawns the MC→Discord broadcast listener, connects to the gateway.
- Shared state is `discord_bot::Data` (`dc_bot.rs`): `target_channel_id_list: Arc<RwLock<Option<Vec<ChannelId>>>>` — bridging is **multi-channel**. RCON is an `Arc<Mutex<RconClient>>`. Bridged channels **persist across restarts** in a JSON file: `src/storage.rs` loads them at startup and saves on every `start_bridge`/`stop_bridge`. Path is `BRIDGE_STORAGE_PATH` or the default `storage.json` (gitignored).
- Commands live in `discord_bot/commands.rs` (prefix + slash): `ping`, `info`, `start_bridge`, `stop_bridge`, `help`. Slash commands are registered globally at startup. Prefixes: `~` plus literal `hey makima,` / `hey makima`.
- Only the bridge commands are owner/admin-gated (`is_owner_or_admin`): hard-coded `OWNER_ID` (dc_bot.rs:30) or ADMINISTRATOR permission.

## Env / secrets
- `.env` at repo root (gitignored). Required, no default: `LOG_PATH`, `DISCORD_TOKEN`, `RCON_PASSWORD`. `RCON_SERVER_ADDRESS` / `MC_SERVER_QUERY_ADDRESS` default to `localhost:25575` / `25565`. `BRIDGE_STORAGE_PATH` defaults to `storage.json`.
- The local `.env` holds a live Discord token + RCON password — never commit or log it.
- Startup fails if RCON is unreachable (`main.rs` connects before Discord); `LOG_PATH` file must exist.

## Gotchas
- `info` uses a **server-status ping** (`rust_mc_status`) + RCON `list`, not the Query protocol; the `msp`/`mc-query` deps are unused.
- Log-parser regexes assume current Vanilla log format; version changes silently break parsing. Death detection is a `DEATH_SENTENCES` substring list (`mc_log.rs`) — new death messages must be added there.
- `init_discord_client` sets only `MESSAGE_CONTENT` intent; the `GuildMemberAddition` welcome-embed path won't fire unless `GUILD_MEMBERS` intent is also enabled (README claims otherwise — stale).
- `stop_bridge` on a non-bridged channel succeeds silently.
- RCON `send_command` is blocking — hold the `Mutex` guard only for the call.

## Code conventions (keep short)
- Prefer `anyhow` with `.context()`; no panics/`unwrap` in non-test paths.
- Use `tokio` primitives for sharing state (already the pattern via `Data`).
- Keep parsers/formatting pure and testable; write table-driven unit tests for `parse_log_line`.