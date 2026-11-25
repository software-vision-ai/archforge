# Dependency updates

Every change to `Cargo.lock` that is not part of a feature (a security fix, the weekly craft-libs bump) gets a line here, in the same pull request: the date, what moved, and why. The gate never changes `Cargo.lock` (it runs `--locked`); craft-libs moves only through `cargo xtask bump`.

| Date | What moved | Why |
|---|---|---|
| 2026-11-22 | `url` 2.5.8 → 2.5.9, `form_urlencoded` 1.2.2 → 1.2.3 (`cargo update -p url --precise 2.5.9`) | [RUSTSEC-2026-0592](https://rustsec.org/advisories/RUSTSEC-2026-0592): stack overflow in `Url::origin()` on nested `blob:` URLs. `url` comes in through `mysql` (archforge-dolt) and `webbrowser` (eframe); cargo-deny refused 2.5.8, so every gate failed. AR-798 |
| 2026-11-25 | craft-libs `52fc6b5b` → `05b99ee5` (`cargo xtask bump`): `craft-atomic`, `craft-dialogs`, `craft-i18n`, `craft-mcp-server`, `craft-testkit`; no other package moved | The weekly craft-libs bump. Of the crates Archforge uses, only two changed upstream. `craft-mcp-server` ([craft-libs#2444](https://github.com/storytold/craft-libs/pull/2444)) now refuses a request whose `jsonrpc` is not `"2.0"` in its `Server` before dispatching; Archforge takes only `check_arguments` from it, which did not change, so Archforge's MCP server behaves as before. `craft-testkit` ([craft-libs#2106](https://github.com/storytold/craft-libs/pull/2106)) adds tests to `sample_damage` only. `craft-atomic`, `craft-dialogs` and `craft-i18n` did not change. No licence change (MIT OR Apache-2.0) |
