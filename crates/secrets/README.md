# archforge-secrets

**Purpose.** The API keys for the agents Archforge talks to, kept in the system's credential store:
the keychain on macOS, the Credential Manager on Windows, the Secret Service on Linux. There is no
plaintext fallback anywhere: on a system without a secure store a key cannot be saved, and every
call answers `SecretError::NoSecureStore`, which the interface shows. Nothing is written to a file.

**Layer.** 9, a host service beside `control` and `dolt`: it talks to the operating system, so the
shell never depends on it and it is never built for the portable target.

**Public API.**

| Item | Signature |
|---|---|
| `SecretStore` | `get(&SecretName) -> Result<Secret, _>`, `set(&SecretName, &Secret) -> Result<(), _>` (replaces), `delete(&SecretName) -> Result<Deleted, _>`; `Send + Sync` |
| `KeyringStore::open` | `pub fn open() -> Result<KeyringStore, SecretError>`: the system's store under the service `SERVICE` (`Archforge`); `NoSecureStore` when there is none; its answer holds until the next start |
| `MemoryStore` | `new()`, `unavailable()` (answers `NoSecureStore`, as a system without a store); feature `testing`, never shipped |
| `Secret` | `new(String) -> Result<Secret, _>` (takes the string without copying; empty or over `MAX_SECRET_BYTES` refused), `expose(&self) -> &str` |
| `SecretName` | `new(&str) -> Result<SecretName, _>`: 1 to 64 lowercase letters, digits and hyphens, starting with a letter; `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`; a refused name is not repeated in its error, only its length and the rule |
| `held_key_in` | `pub fn held_key_in(store: &dyn SecretStore, text: &str) -> Result<Option<SecretName>, SecretError>`: the first kept API key `text` holds, by name (a key not kept, or no secure store, holds none; any other read failure is an error, so the text is not taken as checked); `held_names()` lists the names it looks under |
| `Deleted` | `Removed`, `WasAbsent` |
| `SecretError` | `NoSecureStore`, `NotFound`, `Denied` (locked or prompt declined), `InvalidName`, `EmptySecret`, `SecretTooLong`, `NotText`, `Platform { operation, name, detail }` |
| `SERVICE`, `MAX_SECRET_BYTES`, `MAX_NAME_BYTES`, `MAX_DETAIL_BYTES`, `REDACTED` | `Archforge`, 1 KiB (within every platform's limit), 64, 1 KiB, `Secret(<redacted>)` |

**A secret cannot be logged by accident.** `Secret`'s `Debug` prints `Secret(<redacted>)` whatever
its value; it has no `Display`, no `Clone` and no serialisation (two `compile_fail` doc tests hold
that), and its bytes are overwritten with `zeroize` when it is dropped. No error carries a secret:
the store's errors that hold the stored bytes are overwritten and reduced to a name, a platform
message is scrubbed of the value a `set` gave it and only then capped at 1 KiB (so no cut leaves part
of it behind), a refused name is not repeated (it may be a key pasted into the wrong place), and a
kind of error the `keyring` crate adds later is named, never shown. Copies the platform libraries make inside their
own calls are outside this crate's reach.

**Calls block, and run on a worker.** `KeyringStore::open` and every call on the store may wait for
the system (on Linux, the session bus), and on macOS may show the system's own access prompt, so
callers run them away from the interface thread. The Linux store crate unwraps its own lock and
would panic after an earlier panic inside it, so that worker turns a panic into an error.

**Opened once.** `keyring` opens the system's store on first use and keeps that answer for the
process: on a Linux system whose Secret Service starts after Archforge, keys can be saved after the
next start, and the message the interface shows says so.

**Why these `keyring` features.** `keyring` 4.2.0 is pinned exactly, with its default features off
and its `v1` feature named, which is the one way to build its library and switches on exactly three
backends, each only on its own platform:

- the macOS keychain (`apple-native-keyring-store` with `keychain`, through `security-framework`,
  which binds the system framework and builds no C);
- the Windows Credential Manager (`windows-native-keyring-store`, through `windows-sys`);
- the Linux Secret Service over D-Bus, through `zbus-secret-service-keyring-store` with its
  pure-Rust crypto (`crypto-rust`: RustCrypto's `aes`, `cbc`, `hkdf`, `sha2`). It speaks D-Bus with
  `zbus`, already in the lockfile for the interface's accessibility bridge.

Left off: `cli`, which brings every store `keyring` knows, among them
`dbus-secret-service-keyring-store` (libdbus, built from vendored C source), the Linux kernel
keyutils store (keys there do not outlive the session), `db-keystore` (a database file, which the
owner ruled out) and the Android store. So the default build compiles no OpenSSL and no libdbus, and
every crate it adds passes `deny.toml`'s licence allowlist and sources as they stand. The Secret
Service path runs over the zbus 5 family, whose unmaintained advisories `deny.toml` already ignored
for the accessibility bridge; their reasons now name this second parent, so they end only when both
reach zbus 6.

`keyring` is pinned exactly; the code that handles the keys lives in its store crates
(`keyring-core` 1.0.1, `apple-native-keyring-store` 1.0.2, `windows-native-keyring-store` 1.1.0,
`zbus-secret-service-keyring-store` 1.0.1), held by `Cargo.lock`, so they move only in a reviewed
lockfile change.

**Tests.** `cargo test -p archforge-secrets` runs every test against `MemoryStore` and the error
mapping, with a canary value that no `Debug`, display or error may show. The one test of the real
store is `#[ignore]`d, so the gate never prompts or depends on a keychain; run it by hand with
`cargo test -p archforge-secrets -- --ignored`. It works under the service `Archforge test` and
removes what it saved.

**Non-goals.** Windows support is built but not yet checked by hand (macOS and Linux come first).
The settings screen that asks for a key, and the agents that read one, come with their own changes.
