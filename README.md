# Archforge

Archforge helps you design software systems with diagrams. Draw a database with an ER diagram or
explore classes and their relationships with a UML class diagram. You can work on the model
yourself, or invite an AI agent to work alongside you and see its changes appear on screen.

[Download Archforge](https://github.com/software-vision-ai/archforge/releases) or follow the
[first steps in the user manual](docs/manual/README.md). Archforge runs on macOS, Windows and
Linux.

## Screenshots

From the latest release, in your appearance (light or dark); the [user manual](docs/manual/README.md)
walks through each screen.

**Start a project**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-home-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-home-light.png" alt="Home: start a new project or open one, with pinned folders and recent projects below." width="100%">
</picture>

**Plan a database**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-er-diagram-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-er-diagram-light.png" alt="Customer, Order and Account tables connected in a database diagram, with Order's details on the right." width="100%">
</picture>

**Explore classes**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-class-diagram-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-class-diagram-light.png" alt="A class diagram connecting Customer, Order and related classes, with Order's details on the right." width="100%">
</picture>

**Create an output with an agent**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-generate-dark.png">
  <img src="https://github.com/software-vision-ai/archforge/releases/latest/download/screenshot-generate-light.png" alt="The Generate dialog offers SQL or documentation and a request to copy to an external AI agent." width="100%">
</picture>

## Get started

1. [Install Dolt](https://docs.dolthub.com/introduction/installation), which Archforge uses to
   save your projects. Use version 2.6 or a later 2.x.
2. [Download Archforge](https://github.com/software-vision-ai/archforge/releases) for macOS,
   Windows or Linux.
3. Open Archforge and follow [Getting started](docs/manual/02-getting-started.md) to create your
   first project.

The current packages are not signed, so your system may ask for an extra confirmation the first
time you open Archforge. The [getting-started guide](docs/manual/02-getting-started.md) explains
what to expect.

## Work with an AI agent

An external AI agent can help you explore or change the project you have open. Its changes appear
in Archforge as they happen, and you can undo them. You can also ask an agent to write SQL or
documentation from your model. Agent access is optional and currently available on macOS and
Linux.

See [Working with an agent](docs/manual/07-agents.md) for setup steps and an example.

## Supporting Archforge

Archforge is free and stays free: a contribution unlocks nothing. If it helps your work, you can
support its development:

- [Ko-fi](https://ko-fi.com/dejanwho): a one-time tip to Dejan, who develops Archforge.
- GitHub Sponsors, one-time or monthly, for the software-vision-ai organization: the link appears
  here once its Sponsors profile is published.

The repository's Sponsor button and the app's About Archforge (the bar's Help button) offer the
same.

## Contributing

Read [Contributing](CONTRIBUTING.md) for how to propose a change and [Contributor rules](AGENTS.md)
for how we work. Everyone taking part agrees to the [Code of Conduct](CODE_OF_CONDUCT.md).
Report security problems as [Security](SECURITY.md) describes, not in a public issue.

<details>
<summary>Building and checking from source (for contributors)</summary>

### Building

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

### Checking

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

Releases are cut as [docs/releasing.md](docs/releasing.md) describes: bump the version with
`cargo xtask version`, push `main` to the `release` branch, and publish the draft it builds.

</details>

## Licence

The code is available under the MIT licence or the Apache License, Version 2.0, at your option
(`LICENSE-MIT`, `LICENSE-APACHE`). The artwork, the application icon included, is licensed like the
code; the name Archforge is a brand mark and is not covered by either licence (`NOTICE`).
Third-party assets are listed in `ATTRIBUTION.md`.
