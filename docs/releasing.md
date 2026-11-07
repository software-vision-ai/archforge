# Releasing Archforge

Every push to the `release` branch runs `.github/workflows/release.yml`. It builds every package
on GitHub's runners and creates or updates a **draft** GitHub release named `Archforge v<version>`.
Nobody sees a draft until it is published.

## Versions

The version lives in one place, `[workspace.package] version` in the root `Cargo.toml`, and moves
only through the version tool:

```sh
cargo xtask version              # prints it, e.g. 0.1.0
cargo xtask version 0.2.0        # or 0.2.0-rc.1; updates Cargo.toml and Cargo.lock
```

A release adds features as a minor version (0.2.0, 0.3.0); a release of fixes alone is a patch
(0.2.1). A version with a suffix (`0.2.0-rc.1`) is a pre-release. A version that is published
can't be released again, so the next release always bumps it first.

## Cutting a release

1. **Bump the version** on `main` with the version tool, through a pull request as any change.
2. **Push `main` to `release`:**

   ```sh
   git push origin main:release
   ```

   The workflow starts by itself.
3. **Wait for the draft.** After about 40 minutes the Releases page has the draft
   `Archforge v<version>`, with every package, `SHA256SUMS` and 56 screenshots: the README
   gallery's eight, which the workflow makes no draft without, and the forty-eight guide pictures
   of the wiki and the user manual (see [Screenshots](#screenshots)). Its notes are Archforge's own
   lines (install Dolt first, the packages are unsigned, what isn't available yet) above GitHub's
   notes from the pull requests merged since the last release: *What's Changed*, *New Contributors*
   and the *Full Changelog* link.
4. **Check it.** Download a package or two, and follow the
   [release checklist](release-checklist.md) in the running app.
5. **Publish** the draft on GitHub. Publishing creates the tag `v<version>` on the commit that was
   built.
6. **Look at the pictures.** A release becomes the latest once it is published, and the README's
   gallery then shows its screenshots: open the repository's front page in a light and in a dark
   appearance and see that all four load. A pre-release never becomes the latest, so the README
   keeps showing the previous release's pictures; check the pre-release's own instead, at
   `releases/download/v<version>/screenshot-<name>-<light|dark>.png`.

Pushing to `release` again before publishing rebuilds the same draft: its packages and notes are
replaced. Once the draft is published, the workflow refuses that version, so bump it first.

**Dry runs:** a pull request that touches `packaging/` or the workflow, and a run started by hand
(*Actions → Release → Run workflow*), build and check every package without releasing anything;
the packages stay on the run as artefacts.

## What gets built

| System | Packages | Built on |
|---|---|---|
| macOS 11 or later, universal (Apple silicon and Intel) | `archforge-<v>-macos-universal.dmg` | `macos-15` |
| Windows 10 or later, x64 | `archforge-<v>-windows-x64.msi`, `archforge-<v>-windows-x64.zip` | `windows-latest` |
| Windows 11 on ARM, ARM64 | `archforge-<v>-windows-arm64.msi`, `archforge-<v>-windows-arm64.zip` | `windows-11-arm` |
| Linux x86_64 and aarch64, glibc 2.35 or later | `archforge-<v>-linux-<arch>.{AppImage,AppImage.zsync,deb,rpm,tar.gz}` | `ubuntu-22.04`, `ubuntu-22.04-arm` |

The macOS and Linux packages carry `archforge-cli`, the command-line tool agents run
(`archforge-cli mcp --bridge <port>`), which the in-app chat is to start. It is built in release
mode with the application and placed beside its executable, where `archforge_control::cli_path`
finds it:

| Package | The application | `archforge-cli` |
|---|---|---|
| macOS dmg | `Archforge.app/Contents/MacOS/archforge` | `Archforge.app/Contents/MacOS/archforge-cli`, universal too |
| Linux tar.gz (and the Flatpak made from it) | `bin/archforge` | `bin/archforge-cli` |
| Linux deb and rpm | `/usr/bin/archforge` | `/usr/bin/archforge-cli` |
| Linux AppImage | `usr/bin/archforge` inside the image | `usr/bin/archforge-cli` inside the image |

Inside the macOS bundle it is covered when the bundle is signed and notarised: the signing step
signs it first, with the same identity and the hardened runtime, then the bundle. The workflow
checks that every package holds it, and that the dmg's and the AppImage's report the release's
version (`archforge-cli --version`). The Windows packages don't carry it yet.

Each comes from its script under `packaging/`. Every package reports its version, commit and date
(`archforge --version`): the workflow sets `ARCHFORGE_BUILD_SHA` and `ARCHFORGE_BUILD_DATE`, which
`apps/archforge/build.rs` reads; a plain `cargo build` reports a dev build. Where a system takes
only numbers (the macOS bundle, the Windows installer and version resource), a pre-release carries
its numbers alone, `0.2.0` of `0.2.0-rc.1`; file names keep the whole version.

## Screenshots

The README's gallery shows four screens in the visitor's appearance: Home, an ER diagram, a UML
class diagram and File → Generate. The pictures are never committed. The workflow renders them
from the release's own commit with `cargo xtask screens --gallery <dir>` (the screens and their
file names are `GALLERY` in `xtask/src/screens.rs`) and attaches them to the release as
`screenshot-<name>-<light|dark>.png`. The same command renders twenty-four guide screens beside
them (`WIKI_GALLERY`, published as `screenshot-guide-<name>-<light|dark>.png`), which the README
doesn't show: the wiki shows them, and the user manual's
[Going further](manual/05-uml-class-model.md#going-further) shows those of its sections. The
README, the wiki and the manual load them from `releases/latest/download/`, so every published
release refreshes them, and they show the latest release. A pre-release carries its own
pictures too, but `latest` skips it.

The gallery stays whole in two ways:

- **The workflow refuses a release without it.** After rendering, the Screenshots job checks that
  every `screenshot-*.png` the README names is among the pictures. The draft needs that job, so a
  missing picture stops the release before anything is published. A test in `xtask/src/screens.rs`
  keeps `GALLERY` and the README's pictures in step, another that every `WIKI_GALLERY` screen
  is one the snapshot example draws and that no two published pictures share a name, and a third
  that every guide picture the manual loads is one `WIKI_GALLERY` publishes.
- **Look after publishing**, as step 6 above says: the front page for a release, the
  pre-release's own `releases/download/v<version>/` files for a pre-release. A draft can't be
  checked through `latest`.

A release published without the pictures (v0.1.0 was, before the Screenshots job existed) shows
broken pictures on the front page until they are attached. Render them from that release's tag
into a clean `dist/screenshots`:

```sh
git switch --detach v<version>
rm -rf dist/screenshots && mkdir -p dist/screenshots
cargo xtask screens --gallery dist/screenshots
```

A tag older than the `--gallery` option (v0.1.0 is one) ignores it and writes every screen to
`plan/screens/<screen>-<light|dark>.png` instead. Copy the four screens `GALLERY` names to their
gallery names there (a tag older than `WIKI_GALLERY` has no guide screens to attach):

```sh
rm -rf dist/screenshots && mkdir -p dist/screenshots
cargo xtask screens
for pair in home:home workspace-cross-schema:er-diagram workspace-class-members:class-diagram workspace-generate:generate; do
  for theme in light dark; do
    cp "plan/screens/${pair%%:*}-$theme.png" "dist/screenshots/screenshot-${pair##*:}-$theme.png"
  done
done
```

Look at them before uploading: rendered from the tag, they show only what that release has. Then
attach them with a `SHA256SUMS` that lists them, made from the release's own files (on macOS,
`shasum -a 256` in place of `sha256sum`):

```sh
assets=$(mktemp -d)
gh release download v<version> --dir "$assets"
cp dist/screenshots/*.png "$assets"/
rm "$assets/SHA256SUMS"
(cd "$assets" && sha256sum -- * >SHA256SUMS)
gh release upload v<version> dist/screenshots/*.png "$assets/SHA256SUMS" --clobber
```

The packages themselves stay as they were; only the pictures and the checksum list change.

To change what the gallery shows, edit `GALLERY` and the README's Screenshots section together (a
test keeps them in step), and render them locally with the same command to look at them first. To
change the guide screens, edit `WIKI_GALLERY` together with the wiki pages and the manual's
sections that load them.

Nothing is signed yet. The macOS and Windows scripts have their signing steps, which wait for their
secrets: until then macOS and Windows ask before they first open Archforge, as the README says.
