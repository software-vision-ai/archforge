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
   `Archforge v<version>`, with every package, `SHA256SUMS` and the README gallery's eight
   screenshots: the workflow makes no draft without them (see [Screenshots](#screenshots)). Its
   notes are Archforge's own lines
   (install Dolt first, the packages are unsigned, what isn't available yet) above GitHub's notes
   from the pull requests merged since the last release: *What's Changed*, *New Contributors* and the
   *Full Changelog* link.
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
`screenshot-<name>-<light|dark>.png`. The README and the wiki load them from
`releases/latest/download/`, so every published release refreshes them, and they show the latest
release. A pre-release carries its own pictures too, but `latest` skips it.

The gallery stays whole in two ways:

- **The workflow refuses a release without it.** After rendering, the Screenshots job checks that
  every `screenshot-*.png` the README names is among the pictures. The draft needs that job, so a
  missing picture stops the release before anything is published. A test in `xtask/src/screens.rs`
  keeps `GALLERY` and the README's pictures in step.
- **Look after publishing**, as step 6 above says: the front page for a release, the
  pre-release's own `releases/download/v<version>/` files for a pre-release. A draft can't be
  checked through `latest`.

A release published without the pictures (v0.1.0 was, before the Screenshots job existed) shows
broken pictures on the front page until they are attached. Render them from that release's tag,
then upload them to it:

```sh
git switch --detach v<version>
cargo xtask screens --gallery dist/screenshots
gh release upload v<version> dist/screenshots/*.png --clobber
```

Uploading adds files to a published release and leaves its packages and `SHA256SUMS` as they
are. A tag older than the `--gallery` option (v0.1.0 is one) renders with plain
`cargo xtask screens`, which writes every screen to `plan/screens/<screen>-<light|dark>.png`: copy
the four screens `GALLERY` names to their gallery names, for example `workspace-cross-schema-dark.png`
to `screenshot-er-diagram-dark.png`, and look at them before uploading. Rendered from the tag, they
show only what that release has.

To change what the gallery shows, edit `GALLERY` and the README's Screenshots section together (a
test keeps them in step), and render them locally with the same command to look at them first.

Nothing is signed yet. The macOS and Windows scripts have their signing steps, which wait for their
secrets: until then macOS and Windows ask before they first open Archforge, as the README says.
