//! The store in the system's credential store, through the `keyring` crate: the keychain on
//! macOS, the Credential Manager on Windows, the Secret Service over D-Bus on Linux.

use keyring::{Entry, Error};
use zeroize::Zeroize;

use crate::{Deleted, Operation, SERVICE, Secret, SecretError, SecretName, SecretStore, detail};

/// Secrets in the system's credential store, under [`SERVICE`].
#[derive(Debug)]
pub struct KeyringStore {
    service: &'static str,
}

impl KeyringStore {
    /// Opens the system's store; [`SecretError::NoSecureStore`] when it has none, as a Linux
    /// system without a Secret Service. Nothing falls back to a file.
    ///
    /// The first call opens the store and may block (on Linux it connects to the session bus). Its
    /// answer holds until Archforge restarts: a Secret Service that starts later is found at the
    /// next start.
    pub fn open() -> Result<KeyringStore, SecretError> {
        KeyringStore::open_service(SERVICE)
    }

    /// The store under another service, so a test of the real store leaves Archforge's keys alone.
    pub(crate) fn open_service(service: &'static str) -> Result<KeyringStore, SecretError> {
        store_ready()?;
        Ok(KeyringStore { service })
    }

    /// The system store's entry for `name` under this store's service; a failure is mapped as the
    /// call `operation` maps one.
    fn entry(&self, name: &SecretName, operation: Operation) -> Result<Entry, SecretError> {
        Entry::new(self.service, name.as_str()).map_err(|e| map_error(e, operation, name, None))
    }
}

impl SecretStore for KeyringStore {
    /// Reads the value as text; one that is empty or longer than [`crate::MAX_SECRET_BYTES`] is
    /// refused and overwritten.
    fn get(&self, name: &SecretName) -> Result<Secret, SecretError> {
        let value = self.entry(name, Operation::Get)?.get_password().map_err(|e| map_error(e, Operation::Get, name, None))?;
        Secret::new(value)
    }

    /// Saves through the system store, replacing an earlier value; a failure's platform message is
    /// scrubbed of the value before it is kept.
    fn set(&self, name: &SecretName, secret: &Secret) -> Result<(), SecretError> {
        let entry = self.entry(name, Operation::Set)?;
        entry.set_password(secret.expose()).map_err(|e| map_error(e, Operation::Set, name, Some(secret.expose())))
    }

    /// Removes the entry: a missing one answers `WasAbsent` rather than an error, and any other failure
    /// is mapped without the value.
    fn delete(&self, name: &SecretName) -> Result<Deleted, SecretError> {
        match self.entry(name, Operation::Delete)?.delete_credential() {
            Ok(()) => Ok(Deleted::Removed),
            Err(Error::NoEntry) => Ok(Deleted::WasAbsent),
            Err(e) => Err(map_error(e, Operation::Delete, name, None)),
        }
    }
}

/// Whether the system's store could be opened, the first call opening it.
fn store_ready() -> Result<(), SecretError> {
    match Entry::store_status() {
        Ok(()) => Ok(()),
        Err(e) => Err(no_store(e)),
    }
}

/// The error for a store that could not be opened, whatever the reason.
pub(crate) fn no_store(error: &Error) -> SecretError {
    let reason = match error {
        Error::PlatformFailure(_)
        | Error::NoStorageAccess(_)
        | Error::NoDefaultStore
        | Error::Invalid(..)
        | Error::NotSupportedByStore(_)
        | Error::BadStoreFormat(_) => detail(error.to_string(), None),
        // The variants that hold data, and any kind this version does not know, are named, never
        // shown.
        Error::BadEncoding(_) | Error::BadDataFormat(..) => "the store answered with malformed data".to_owned(),
        _ => "the store could not be opened, for a reason this version of Archforge does not recognise".to_owned(),
    };
    SecretError::NoSecureStore { reason }
}

/// The error for a failed call on the secret `name`. The variants that hold the stored value's
/// bytes are overwritten and reduced to a name; no error keeps them. `secret`, the value a `set`
/// was given, is scrubbed from every platform message before it is cut to length.
pub(crate) fn map_error(error: Error, operation: Operation, name: &SecretName, secret: Option<&str>) -> SecretError {
    let name_text = name.as_str().to_owned();
    let platform = |detail: String| SecretError::Platform { operation, name: name.as_str().to_owned(), detail };
    match error {
        Error::NoEntry => SecretError::NotFound { name: name_text },
        Error::NoDefaultStore => match Entry::store_status() {
            Err(e) => no_store(e),
            Ok(()) => SecretError::NoSecureStore { reason: "no credential store is set".to_owned() },
        },
        Error::NoStorageAccess(e) => SecretError::Denied { name: name_text, detail: detail(e.to_string(), secret) },
        Error::PlatformFailure(e) => platform(detail(e.to_string(), secret)),
        Error::BadEncoding(mut bytes) => {
            bytes.zeroize();
            SecretError::NotText { name: name_text }
        }
        Error::BadDataFormat(mut bytes, _) => {
            bytes.zeroize();
            platform("the stored value is malformed".to_owned())
        }
        Error::BadStoreFormat(reason) => platform(detail(format!("the store is malformed: {reason}"), secret)),
        Error::TooLong(attribute, limit) => platform(detail(format!("'{attribute}' is longer than the store's limit of {limit}"), secret)),
        Error::Invalid(attribute, reason) => platform(detail(format!("'{attribute}' is invalid: {reason}"), secret)),
        Error::Ambiguous(entries) => platform(format!("{} entries match the name", entries.len())),
        Error::NotSupportedByStore(issue) => platform(detail(format!("the store does not support it: {issue}"), secret)),
        // A kind of error this version of the crate does not know: named, never shown, as it may
        // hold the value.
        _ => platform("the store failed in a way this version of Archforge does not recognise".to_owned()),
    }
}
