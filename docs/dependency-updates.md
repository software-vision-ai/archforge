# Dependency updates

Every change to `Cargo.lock` that is not part of a feature (a security fix, the weekly craft-libs bump) gets a line here, in the same pull request: the date, what moved, and why. The gate never changes `Cargo.lock` (it runs `--locked`); craft-libs moves only through `cargo xtask bump`.

| Date | What moved | Why |
|---|---|---|
| 2026-11-22 | `url` 2.5.8 → 2.5.9, `form_urlencoded` 1.2.2 → 1.2.3 (`cargo update -p url --precise 2.5.9`) | [RUSTSEC-2026-0592](https://rustsec.org/advisories/RUSTSEC-2026-0592): stack overflow in `Url::origin()` on nested `blob:` URLs. `url` comes in through `mysql` (archforge-dolt) and `webbrowser` (eframe); cargo-deny refused 2.5.8, so every gate failed. AR-798 |
