//! API keys for the agents Archforge talks to, kept in the system's credential store: the
//! keychain on macOS, the Credential Manager on Windows, the Secret Service on Linux. There is no
//! plaintext fallback: on a system without a secure store a key cannot be saved, and every call says
//! so with [`SecretError::NoSecureStore`].
//!
//! A [`Secret`] cannot be logged by accident: its `Debug` prints a fixed redaction, it has no
//! `Display`, no `Clone` and no serialisation, and its bytes are overwritten when it is dropped.
//! No error carries a secret.
//!
//! Every call on [`KeyringStore`] may block while the system answers, and on macOS may show the
//! system's own access prompt, so callers run them away from the interface thread.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

use std::borrow::Cow;
use std::fmt;

use zeroize::Zeroize;

mod keyring_store;
#[cfg(any(test, feature = "testing"))]
mod memory;

pub use keyring_store::KeyringStore;
#[cfg(any(test, feature = "testing"))]
pub use memory::MemoryStore;

/// The service every Archforge secret is kept under in the system's store. Fixed: a key saved by
/// one version is found by the next.
pub const SERVICE: &str = "Archforge";

/// The longest secret kept, in bytes: ample for an API key, and within every platform's limit (the
/// Windows Credential Manager's is 1280 characters), so a secret that one system takes every system
/// takes.
pub const MAX_SECRET_BYTES: usize = 1024;

/// The longest secret name, in bytes.
pub const MAX_NAME_BYTES: usize = 64;

/// The longest platform detail an error keeps, in bytes.
pub const MAX_DETAIL_BYTES: usize = 1024;

/// The name a secret is kept under: 1 to [`MAX_NAME_BYTES`] lowercase ASCII letters, digits and
/// hyphens, starting with a letter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretName(Cow<'static, str>);

impl SecretName {
    /// The API key for Anthropic's Claude.
    pub const ANTHROPIC_API_KEY: SecretName = SecretName(Cow::Borrowed("anthropic-api-key"));
    /// The API key for OpenAI's Codex.
    pub const OPENAI_API_KEY: SecretName = SecretName(Cow::Borrowed("openai-api-key"));

    /// A name, checked against the rules above.
    pub fn new(name: &str) -> Result<SecretName, SecretError> {
        match name_problem(name) {
            None => Ok(SecretName(Cow::Owned(name.to_owned()))),
            Some(reason) => Err(SecretError::InvalidName { bytes: name.len(), reason }),
        }
    }

    /// The name as the system store keys it, already checked against the rules.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretName {
    /// Writes the bare name; a name is never secret, so it may appear in messages.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The first rule `name` breaks, if any, said without repeating the name.
fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("it is empty".to_owned());
    }
    if name.len() > MAX_NAME_BYTES {
        return Some(format!("it is longer than {MAX_NAME_BYTES} bytes"));
    }
    if !name.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Some("it does not start with a lowercase letter".to_owned());
    }
    name.char_indices()
        .find(|(_, c)| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-'))
        .map(|(at, _)| format!("the character at byte {at} is not a lowercase letter, a digit or a hyphen"))
}

/// A secret value, such as an API key. Its `Debug` prints a fixed redaction; it has no `Display`,
/// no `Clone` and no serialisation, and its bytes are overwritten when it is dropped. Read it with
/// [`Secret::expose`], only where it is handed to the service that needs it.
///
/// ```compile_fail
/// let secret = archforge_secrets::Secret::new("sk-example".to_owned()).unwrap();
/// println!("{secret}"); // no Display: a secret cannot be formatted for people
/// ```
///
/// ```compile_fail
/// let secret = archforge_secrets::Secret::new("sk-example".to_owned()).unwrap();
/// let copy = secret.clone(); // no Clone: every copy is one more to overwrite
/// ```
pub struct Secret(String);

/// What a secret's `Debug` prints, whatever its value.
pub const REDACTED: &str = "Secret(<redacted>)";

impl Secret {
    /// Takes `value` without copying it. An empty value or one longer than [`MAX_SECRET_BYTES`] is
    /// refused, and overwritten before the error is returned.
    pub fn new(mut value: String) -> Result<Secret, SecretError> {
        if value.is_empty() {
            return Err(SecretError::EmptySecret);
        }
        if value.len() > MAX_SECRET_BYTES {
            let bytes = value.len();
            value.zeroize();
            return Err(SecretError::SecretTooLong { bytes, max: MAX_SECRET_BYTES });
        }
        Ok(Secret(value))
    }

    /// The value, for the one place that sends it on.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// A second secret with the same value, for a store that keeps its own.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn duplicate(&self) -> Secret {
        Secret(self.0.clone())
    }
}

impl Drop for Secret {
    /// Overwrites the value's bytes, spare capacity included, before the memory is freed.
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    /// Writes [`REDACTED`] whatever the value, so no log or panic message can show it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

/// What a delete found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deleted {
    /// The secret was there and is gone.
    Removed,
    /// There was no secret by that name; nothing changed.
    WasAbsent,
}

/// The operation a platform error happened in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Get,
    Set,
    Delete,
}

impl fmt::Display for Operation {
    /// Writes the operation as a verb for an error message: read, save or remove.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Operation::Get => "read",
            Operation::Set => "save",
            Operation::Delete => "remove",
        })
    }
}

/// Why a secret could not be read, saved or removed. No variant carries a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretError {
    /// This system has no secure credential store (on Linux, no Secret Service answers), so
    /// secrets cannot be kept. Nothing falls back to a file.
    NoSecureStore { reason: String },
    /// No secret is kept under this name.
    NotFound { name: String },
    /// The store is locked, or its access prompt was declined.
    Denied { name: String, detail: String },
    /// The name breaks a rule of [`SecretName`]. The name itself is not kept, as it may be a key
    /// pasted into the wrong place; `bytes` is its length.
    InvalidName { bytes: usize, reason: String },
    /// An empty secret was given, or found in the store.
    EmptySecret,
    /// A secret longer than [`MAX_SECRET_BYTES`] was given, or found in the store.
    SecretTooLong { bytes: usize, max: usize },
    /// The value kept under this name is not UTF-8 text.
    NotText { name: String },
    /// The store failed for another reason, which `detail` gives in the system's words.
    Platform { operation: Operation, name: String, detail: String },
}

impl fmt::Display for SecretError {
    /// Writes one sentence naming the secret and the cause, built only from fields that hold no
    /// secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecretError::NoSecureStore { reason } => {
                write!(
                    f,
                    "this system has no secure credential store, so API keys cannot be saved ({reason}); set one up, such as a Secret Service on Linux, and restart Archforge"
                )
            }
            SecretError::NotFound { name } => write!(f, "no secret named '{name}' is in the credential store"),
            SecretError::Denied { name, detail } => {
                write!(f, "the credential store refused access to '{name}' (it is locked, or the request was declined): {detail}")
            }
            SecretError::InvalidName { bytes, reason } => write!(
                f,
                "a secret name of {bytes} bytes was refused because {reason}; a name is 1 to {MAX_NAME_BYTES} lowercase letters, digits and hyphens, starting with a letter"
            ),
            SecretError::EmptySecret => f.write_str("a secret cannot be empty"),
            SecretError::SecretTooLong { bytes, max } => write!(f, "a secret of {bytes} bytes is longer than the {max} bytes allowed"),
            SecretError::NotText { name } => write!(f, "the secret named '{name}' is not UTF-8 text"),
            SecretError::Platform { operation, name, detail } => {
                write!(f, "the credential store could not {operation} '{name}': {detail}")
            }
        }
    }
}

impl std::error::Error for SecretError {}

/// A platform message as an error keeps it: every occurrence of `secret` replaced first, so a
/// message that echoed the value it was given cannot carry it on, then cut to
/// [`MAX_DETAIL_BYTES`], so no cut can leave part of the value behind.
pub(crate) fn detail(mut text: String, secret: Option<&str>) -> String {
    if let Some(secret) = secret.filter(|s| !s.is_empty() && text.contains(*s)) {
        let scrubbed = text.replace(secret, "<redacted>");
        text.zeroize();
        text = scrubbed;
    }
    capped(text)
}

/// A platform message cut to [`MAX_DETAIL_BYTES`] at a character boundary.
fn capped(mut text: String) -> String {
    if text.len() > MAX_DETAIL_BYTES {
        let mut end = MAX_DETAIL_BYTES;
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        text.truncate(end);
        text.push('…');
    }
    text
}

/// A place secrets are kept, by name.
pub trait SecretStore: Send + Sync {
    /// The secret kept under `name`; [`SecretError::NotFound`] when there is none.
    fn get(&self, name: &SecretName) -> Result<Secret, SecretError>;
    /// Keeps `secret` under `name`, replacing any secret kept there before.
    fn set(&self, name: &SecretName, secret: &Secret) -> Result<(), SecretError>;
    /// Removes the secret kept under `name`, and says whether there was one.
    fn delete(&self, name: &SecretName) -> Result<Deleted, SecretError>;
}

#[cfg(test)]
mod tests;
