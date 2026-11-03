# Archforge

Archforge is a desktop modelling application for macOS, Windows and Linux, built as one Rust
workspace, where people and AI agents work on the same model. Today it designs relational
databases on ER diagrams (schemas with tables, columns, keys, relationships, checks, indexes,
domains, enum types, subtypes, sequences and views) and class models on UML class diagrams
(packages, classes, attributes, operations and their relationships), edited on the diagrams and in
the Properties panel, with skills to guide people and agents, undo and redo for every change, in a
light and a dark appearance. A project is stored in [Dolt](https://github.com/dolthub/dolt) 2.6 or a
later 2.x, found on the `PATH` or through `ARCHFORGE_DOLT`.

The [user manual](docs/manual/README.md) takes a first-time user from starting Archforge to a
small ER and UML model, and shows how an agent works with it.

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

## Agent access

An agent can work in the open project while you watch: start Archforge with a control port, and
let the agent run the MCP server of `archforge-cli`, bridged to that port.

```sh
cargo run -p archforge -- --control 47000
cargo build -p archforge-cli
```

`.mcp.json` registers `target/debug/archforge-cli mcp --bridge 47000` for agents that read it. The
agent gets one tool per command (`er_entity_add`, `er_relationship_add`, `diagram_place`, ...) with
its parameters, plus `batch`, `undo`, `redo`, `inspect_document` and `save_project`, which saves the project as
the Save button does; `--compact` lists the general
tools and `search_tools` instead, for agents short of context. Every
change the agent makes runs through Archforge's commands, so it shows in the window at once and
undo takes it back. The port listens on the loopback interface only, and a token minted for each
start, in a file only you can read in Archforge's settings folder, guards it. Without
`--control` nothing listens. macOS and Linux for now. `archforge-cli app <method> [<json>] --port
47000` sends one request by hand; `archforge-cli commands` lists the commands.

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
gate needs the network. The project database's tests need dolt 2.6 or later within major version 2 on
the PATH, or its absolute path in `ARCHFORGE_DOLT`. Maintainers also run an optional trademark check; without its list
(`FORGE_BRANDS`) the gate skips it and says so. `cargo xtask shot` renders the window offscreen to
`plan/shots/`; `cargo xtask screens` renders every screen in both appearances to `plan/screens/`,
with a contact sheet for each (`light.html`, `dark.html`), the evidence to look over before a
release.

The gate drives every journey end to end in a test harness, but not the operating system: before a
release, follow the [release checklist](docs/release-checklist.md) once in the running app.

## Downloads

None yet.

## Supporting Archforge

Archforge is free and stays free: a contribution unlocks nothing. If it helps your work, you can
support its development:

- [Ko-fi](https://ko-fi.com/dejanwho): a one-time tip to Dejan, who develops Archforge.
- GitHub Sponsors, one-time or monthly, for the software-vision-ai organization: the link appears
  here once its Sponsors profile is published.

The repository's Sponsor button and the app's About Archforge (the bar's Help button) offer the
same.

## Contributing

Read `CONTRIBUTING.md` for how to propose a change and `AGENTS.md` for the rules every change
follows. Report security problems as `SECURITY.md` describes, not in a public issue.

## Licence

The code is available under the MIT licence or the Apache License, Version 2.0, at your option
(`LICENSE-MIT`, `LICENSE-APACHE`). The artwork, the application icon included, is licensed like the
code; the name Archforge is a brand mark and is not covered by either licence (`NOTICE`).
Third-party assets are listed in `ATTRIBUTION.md`.
