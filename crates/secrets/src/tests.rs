use super::keyring_store::{map_error, no_store};
use super::*;

/// A value no output may ever show.
const CANARY: &str = "sk-canary-5f0e2b7d91c4a3";

fn canary() -> Secret {
    Secret::new(CANARY.to_owned()).unwrap()
}

fn secret(value: &str) -> Secret {
    Secret::new(value.to_owned()).unwrap()
}

fn boxed(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.to_owned()))
}

/// Every way an error can be shown: debug, pretty debug, display.
fn shown(error: &SecretError) -> String {
    format!("{error:?}\n{error:#?}\n{error}")
}

#[test]
fn a_secret_comes_back_as_it_was_set() {
    let store = MemoryStore::new();
    store.set(&SecretName::ANTHROPIC_API_KEY, &secret("sk-ant-one")).unwrap();
    assert_eq!(store.get(&SecretName::ANTHROPIC_API_KEY).unwrap().expose(), "sk-ant-one");
}

#[test]
fn setting_again_replaces_the_secret() {
    let store = MemoryStore::new();
    store.set(&SecretName::OPENAI_API_KEY, &secret("first")).unwrap();
    store.set(&SecretName::OPENAI_API_KEY, &secret("second")).unwrap();
    assert_eq!(store.get(&SecretName::OPENAI_API_KEY).unwrap().expose(), "second");
}

#[test]
fn secrets_under_different_names_stay_apart() {
    let store = MemoryStore::new();
    store.set(&SecretName::ANTHROPIC_API_KEY, &secret("a")).unwrap();
    store.set(&SecretName::OPENAI_API_KEY, &secret("o")).unwrap();
    assert_eq!(store.get(&SecretName::ANTHROPIC_API_KEY).unwrap().expose(), "a");
    assert_eq!(store.get(&SecretName::OPENAI_API_KEY).unwrap().expose(), "o");
}

#[test]
fn a_deleted_secret_is_gone() {
    let store = MemoryStore::new();
    store.set(&SecretName::ANTHROPIC_API_KEY, &secret("gone soon")).unwrap();
    assert_eq!(store.delete(&SecretName::ANTHROPIC_API_KEY).unwrap(), Deleted::Removed);
    assert_eq!(store.get(&SecretName::ANTHROPIC_API_KEY).unwrap_err(), SecretError::NotFound { name: "anthropic-api-key".into() });
}

#[test]
fn deleting_a_missing_secret_says_it_was_absent() {
    let store = MemoryStore::new();
    assert_eq!(store.delete(&SecretName::OPENAI_API_KEY).unwrap(), Deleted::WasAbsent);
}

#[test]
fn a_missing_secret_is_not_found_by_name() {
    let error = MemoryStore::new().get(&SecretName::OPENAI_API_KEY).unwrap_err();
    assert_eq!(error, SecretError::NotFound { name: "openai-api-key".into() });
    assert!(error.to_string().contains("openai-api-key"), "{error}");
}

#[test]
fn a_secret_never_shows_its_value() {
    let secret = canary();
    for text in [format!("{secret:?}"), format!("{secret:#?}"), format!("{:?}", Some(&secret)), format!("{:?}", vec![&secret])] {
        assert!(!text.contains(CANARY), "{text}");
        assert!(text.contains(REDACTED), "{text}");
    }
}

#[test]
fn a_store_never_shows_its_secrets() {
    let store = MemoryStore::new();
    store.set(&SecretName::ANTHROPIC_API_KEY, &canary()).unwrap();
    let text = format!("{store:?} {store:#?}");
    assert!(!text.contains(CANARY), "{text}");
    assert!(text.contains("anthropic-api-key"), "{text}");
}

#[test]
fn no_error_carries_the_secret() {
    let name = SecretName::ANTHROPIC_API_KEY;
    let mut bad_text = CANARY.as_bytes().to_vec();
    bad_text.push(0xff);
    let errors = [
        map_error(keyring::Error::BadEncoding(bad_text), Operation::Get, &name, None),
        map_error(keyring::Error::BadDataFormat(CANARY.as_bytes().to_vec(), boxed(CANARY)), Operation::Get, &name, None),
        no_store(&keyring::Error::BadDataFormat(CANARY.as_bytes().to_vec(), boxed(CANARY))),
        no_store(&keyring::Error::BadEncoding(CANARY.as_bytes().to_vec())),
        // A platform that echoed the value it was given, scrubbed as `set` scrubs it.
        map_error(keyring::Error::PlatformFailure(boxed(&format!("rejected {CANARY}"))), Operation::Set, &name, Some(CANARY)),
        map_error(keyring::Error::NoStorageAccess(boxed(&format!("locked {CANARY}"))), Operation::Set, &name, Some(CANARY)),
        // An echo that straddles the length cap: scrubbed before the cut, so no part of it is left.
        map_error(
            keyring::Error::PlatformFailure(boxed(&format!("{}{CANARY}", "x".repeat(MAX_DETAIL_BYTES - 10)))),
            Operation::Set,
            &name,
            Some(CANARY),
        ),
        Secret::new(format!("{CANARY}{}", "x".repeat(MAX_SECRET_BYTES))).unwrap_err(),
        SecretName::new(&format!("{CANARY}{}", "x".repeat(MAX_NAME_BYTES))).unwrap_err(),
        // A short key pasted where a name belongs (with a capital, as keys have).
        SecretName::new(&format!("{CANARY}Q")).unwrap_err(),
    ];
    for error in &errors {
        let text = shown(error);
        // Not even the start of it, as a cut could leave.
        assert!(!text.contains(&CANARY[..10]), "{text}");
    }
    assert_eq!(errors[0], SecretError::NotText { name: "anthropic-api-key".into() });
}

#[test]
fn a_platform_error_keeps_the_systems_words() {
    let error = map_error(keyring::Error::PlatformFailure(boxed("the daemon went away")), Operation::Delete, &SecretName::OPENAI_API_KEY, None);
    assert_eq!(error, SecretError::Platform { operation: Operation::Delete, name: "openai-api-key".into(), detail: "the daemon went away".into() });
    assert_eq!(error.to_string(), "the credential store could not remove 'openai-api-key': the daemon went away");
}

#[test]
fn a_long_platform_message_is_capped() {
    let long = "é".repeat(MAX_DETAIL_BYTES);
    let SecretError::Platform { detail, .. } =
        map_error(keyring::Error::PlatformFailure(boxed(&long)), Operation::Get, &SecretName::OPENAI_API_KEY, None)
    else {
        panic!("not a platform error");
    };
    assert!(detail.len() <= MAX_DETAIL_BYTES + '…'.len_utf8(), "{}", detail.len());
}

#[test]
fn the_store_errors_map_to_their_own_kinds() {
    let name = SecretName::OPENAI_API_KEY;
    assert_eq!(map_error(keyring::Error::NoEntry, Operation::Get, &name, None), SecretError::NotFound { name: "openai-api-key".into() });
    assert_eq!(
        map_error(keyring::Error::NoStorageAccess(boxed("user canceled")), Operation::Get, &name, None),
        SecretError::Denied { name: "openai-api-key".into(), detail: "user canceled".into() }
    );
}

#[test]
fn a_system_without_a_secure_store_says_so() {
    let error = no_store(&keyring::Error::PlatformFailure(boxed("org.freedesktop.secrets was not provided by any service")));
    assert!(matches!(&error, SecretError::NoSecureStore { reason } if reason.contains("org.freedesktop.secrets")), "{error:?}");
    assert!(error.to_string().contains("cannot be saved"), "{error}");
    // An entry asked for after the store failed to open.
    let error = map_error(keyring::Error::NoDefaultStore, Operation::Set, &SecretName::ANTHROPIC_API_KEY, None);
    assert!(matches!(error, SecretError::NoSecureStore { .. }), "{error:?}");
}

#[test]
fn a_store_that_is_not_there_refuses_every_call() {
    let store = MemoryStore::unavailable();
    let name = SecretName::ANTHROPIC_API_KEY;
    assert!(matches!(store.get(&name), Err(SecretError::NoSecureStore { .. })));
    assert!(matches!(store.set(&name, &secret("kept nowhere")), Err(SecretError::NoSecureStore { .. })));
    assert!(matches!(store.delete(&name), Err(SecretError::NoSecureStore { .. })));
}

#[test]
fn empty_and_oversized_secrets_are_refused() {
    assert_eq!(Secret::new(String::new()).unwrap_err(), SecretError::EmptySecret);
    let too_long = "k".repeat(MAX_SECRET_BYTES + 1);
    assert_eq!(Secret::new(too_long).unwrap_err(), SecretError::SecretTooLong { bytes: MAX_SECRET_BYTES + 1, max: MAX_SECRET_BYTES });
    assert!(Secret::new("k".repeat(MAX_SECRET_BYTES)).is_ok());
}

#[test]
fn the_known_names_follow_the_rules() {
    for name in [SecretName::ANTHROPIC_API_KEY, SecretName::OPENAI_API_KEY] {
        assert_eq!(SecretName::new(name.as_str()).unwrap(), name);
    }
}

#[test]
fn a_name_breaking_a_rule_is_refused_with_the_rule() {
    for (name, reason) in [
        ("", "it is empty"),
        ("Anthropic", "it does not start with a lowercase letter"),
        ("1key", "it does not start with a lowercase letter"),
        ("api key", "the character at byte 3 is not a lowercase letter, a digit or a hyphen"),
        ("api_key", "the character at byte 3 is not a lowercase letter, a digit or a hyphen"),
        ("apiké", "the character at byte 4 is not a lowercase letter, a digit or a hyphen"),
    ] {
        assert_eq!(SecretName::new(name).unwrap_err(), SecretError::InvalidName { bytes: name.len(), reason: reason.into() });
    }
    let long = "a".repeat(MAX_NAME_BYTES + 1);
    assert_eq!(SecretName::new(&long).unwrap_err(), SecretError::InvalidName { bytes: 65, reason: "it is longer than 64 bytes".into() });
    assert!(SecretName::new(&"a".repeat(MAX_NAME_BYTES)).is_ok());
}

/// Uses the real system store, which may prompt or be missing, so the gate never runs it. Run it
/// by hand with `cargo test -p archforge-secrets -- --ignored`; it works under its own service
/// and removes what it saved.
#[test]
#[ignore = "touches the system's credential store"]
fn the_system_store_round_trips_a_secret() {
    let store = KeyringStore::open_service("Archforge test").unwrap();
    let name = SecretName::new("archforge-secrets-selftest").unwrap();
    let _ = store.delete(&name);
    assert!(matches!(store.get(&name), Err(SecretError::NotFound { .. })));
    store.set(&name, &secret("first")).unwrap();
    store.set(&name, &secret("second")).unwrap();
    assert_eq!(store.get(&name).unwrap().expose(), "second");
    assert_eq!(store.delete(&name).unwrap(), Deleted::Removed);
    assert_eq!(store.delete(&name).unwrap(), Deleted::WasAbsent);
    assert!(matches!(store.get(&name), Err(SecretError::NotFound { .. })));
}
