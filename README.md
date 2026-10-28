# Archforge

Archforge is a desktop modelling application for macOS, Windows and Linux, built as one Rust
workspace. It is at its start: the window shows Home, the start screen of its design, in a light
and a dark appearance. The screens behind Home's controls come next; until then those controls
say they are not available yet.

## Building

```sh
cargo run -p archforge
```

You need stable Rust 1.95 or later (the `rust-version` in `Cargo.toml`). On Linux, install the
development packages for xkbcommon, Wayland, X11, Xrandr, Xi and Mesa GL first. The shared
crates come from [craft-libs](https://github.com/storytold/craft-libs), by the ArtCraft team, always
at its latest main.

The interface is drawn in Inter. The build embeds it from a checkout of
[craft-fonts](https://github.com/storytold/craft-fonts) next to this one (`CRAFT_FONTS_DIR` in
`.cargo/config.toml`); without that checkout it warns and draws with egui's bundled faces instead:

```sh
git clone https://github.com/storytold/craft-fonts ../craft-fonts
```

## Checking

```sh
cargo xtask ci
```

first moves craft-libs to the newest commit of its main branch (in `Cargo.lock`), then runs every
check the repository has, in order, and stops at the first failure: formatting, lints, tests, the
asset attribution, the agent skills in `skills/` against the files they name, the crate layering, a
build for `wasm32-unknown-unknown` and cargo-deny. It needs that target
(`rustup target add wasm32-unknown-unknown`) and cargo-deny 0.20.2
(`cargo install --locked cargo-deny@0.20.2`), which fetches the RustSec advisory database, so the
gate needs the network. Maintainers also run an optional trademark check; without its list
(`FORGE_BRANDS`) the gate skips it and says so. `cargo xtask shot` renders the window offscreen to
`plan/shots/`.

## Downloads

None yet.

## Contributing

Read `CONTRIBUTING.md` for how to propose a change and `AGENTS.md` for the rules every change
follows. Report security problems as `SECURITY.md` describes, not in a public issue.

## Licence

The code is available under the MIT licence or the Apache License, Version 2.0, at your option
(`LICENSE-MIT`, `LICENSE-APACHE`). The artwork, the application icon included, is licensed like the
code; the name Archforge is a brand mark and is not covered by either licence (`NOTICE`).
Third-party assets are listed in `ATTRIBUTION.md`.
