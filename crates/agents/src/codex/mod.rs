//! The OpenAI Codex adapter (AR-202, after the AR-196 spike in `docs/agent-runtimes/README.md`).
//!
//! - [`CodexAccount`] is a short-lived `codex app-server` for Settings: the account, ChatGPT sign-in
//!   in the browser or with a device code, an API key's check, sign-out, and the models.
//! - The conversation's copy ([`history`]) keeps a conversation's messages, as Codex's threads are
//!   ephemeral.
//!
//! Where the sign-in lives (owner decisions): an API key goes to Codex over its standard input in
//! `account/login/start`, with `cli_auth_credentials_store="ephemeral"`, so it is never written to a
//! file, the keyring or the command line, and every process signs in again. A ChatGPT sign-in is
//! kept in the system keyring (`"keyring"`) or, where no usable keyring exists, in Codex's own files
//! (`"file"`) inside Archforge's private `CODEX_HOME` ([`CodexHome`], a 0700 folder in the
//! workspace); [`CredentialStore`] offers both and the application chooses. Every process of a
//! person shares that `CODEX_HOME`, as the keyring entry is tied to its path.

pub mod account;
pub mod history;
pub mod rpc;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use archforge_secrets::Secret;
use zeroize::Zeroizing;

pub use account::{AccountEvent, AccountState, BrowserLogin, CancelStatus, CodexAccount, DeviceLogin, LoginId, LoginOutcome, ModelInfo};
pub use history::History;
pub use rpc::{EXPIRED_SIGN_IN_MESSAGES, RpcError};

pub use crate::claude::{Bridge, BridgeHome};
use crate::{AgentError, Workspace};

/// The folder in the workspace that is Archforge's private `CODEX_HOME`.
pub const HOME_FOLDER: &str = "codex-home";
/// The name of Archforge's tool server in Codex's configuration.
pub const SERVER_NAME: &str = "archforge";
/// The label of the API key in errors (the key never enters the environment).
pub const KEY_LABEL: &str = "OPENAI_API_KEY";
/// The shortest request timeout accepted.
pub const MIN_REQUEST_TIMEOUT: Duration = Duration::from_millis(10);
/// The longest request timeout accepted.
pub const MAX_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
/// The default request timeout.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The features `codex app-server` is started without (the spike's list): Archforge's bridge is the
/// model's only tool.
const DISABLED: [&str; 14] = [
    "shell_tool",
    "unified_exec",
    "apps",
    "plugins",
    "multi_agent",
    "view_image",
    "image_generation",
    "browser_use",
    "computer_use",
    "goals",
    "hooks",
    "skill_search",
    "tool_suggest",
    "sleep_tool",
];

/// Where Codex keeps a sign-in (`cli_auth_credentials_store`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialStore {
    /// In the process's memory only: for an API key, which every process is given again.
    Ephemeral,
    /// The system keyring: for a ChatGPT sign-in.
    Keyring,
    /// Codex's own files in Archforge's private `CODEX_HOME`: for a ChatGPT sign-in where no usable
    /// keyring exists.
    File,
}

impl CredentialStore {
    /// The value Codex takes.
    pub fn as_str(self) -> &'static str {
        match self {
            CredentialStore::Ephemeral => "ephemeral",
            CredentialStore::Keyring => "keyring",
            CredentialStore::File => "file",
        }
    }
}

/// Archforge's private `CODEX_HOME`: `codex-home` in the workspace, a real folder of this user
/// with mode 0700, shared by every Codex process of the person, as the ChatGPT sign-in is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexHome {
    path: PathBuf,
}

impl CodexHome {
    /// The folder in `workspace`, made with mode 0700 when missing and checked when present (a link,
    /// another user's folder or one others may enter is refused, never changed).
    pub fn open(workspace: &Workspace) -> Result<CodexHome, AgentError> {
        let path = workspace.root().join(HOME_FOLDER);
        crate::folder::ensure_private(&path, crate::sys::current_uid())?;
        Ok(CodexHome { path })
    }

    /// The folder.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The command line after `codex`: `app-server --strict-config`, the credential store, no history
/// file, no analytics, no web search, the spike's features switched off, and (for a session) the
/// bridge as the tool server `archforge` through `-c mcp_servers.archforge.*`: its command, its
/// arguments (`mcp --bridge <port> [--read-only] --origin <label>`), the person's `HOME` (and
/// `XDG_CONFIG_HOME` where set) for the control token, and the spike's timeouts. Refused: a relative
/// or non-UTF-8 path, a port of 0, an origin label `archforge-cli` would refuse, an unknown home, and
/// a control character in any value.
pub fn command_line(store: CredentialStore, bridge: Option<&Bridge>) -> Result<Vec<OsString>, AgentError> {
    let mut args: Vec<String> = vec!["app-server".into(), "--strict-config".into()];
    for value in [
        format!("cli_auth_credentials_store={}", toml_string("credential store", store.as_str())?),
        "history.persistence=\"none\"".to_owned(),
        "analytics.enabled=false".to_owned(),
        "web_search=\"disabled\"".to_owned(),
    ] {
        args.push("-c".into());
        args.push(value);
    }
    for feature in DISABLED {
        args.push("--disable".into());
        args.push(feature.into());
    }
    if let Some(bridge) = bridge {
        for value in bridge_overrides(bridge)? {
            args.push("-c".into());
            args.push(value);
        }
    }
    Ok(args.into_iter().map(OsString::from).collect())
}

/// The `-c` values that give Codex Archforge's tool server.
fn bridge_overrides(bridge: &Bridge) -> Result<Vec<String>, AgentError> {
    let path_text = |what: &'static str, path: &Path| -> Result<String, AgentError> {
        if !path.is_absolute() {
            return Err(AgentError::InvalidSetting { setting: what, bytes: path.as_os_str().len(), reason: "it is not an absolute path".to_owned() });
        }
        let text = path.to_str().ok_or_else(|| AgentError::InvalidSetting {
            setting: what,
            bytes: path.as_os_str().len(),
            reason: "it is not UTF-8 text".to_owned(),
        })?;
        toml_string(what, text)
    };
    let cli = path_text("archforge-cli path", &bridge.cli)?;
    if bridge.port == 0 {
        return Err(AgentError::InvalidSetting { setting: "bridge port", bytes: 1, reason: "the port is 1 to 65535".to_owned() });
    }
    if bridge.origin.starts_with('-') {
        return Err(setting("origin label", &bridge.origin, "a label may not start with -"));
    }
    archforge_control::check_origin(&bridge.origin).map_err(|e| setting("origin label", &bridge.origin, &e.to_string()))?;
    let mut bridge_args = vec!["mcp".to_owned(), "--bridge".to_owned(), bridge.port.to_string()];
    if !bridge.allow_changes {
        bridge_args.push("--read-only".to_owned());
    }
    bridge_args.push("--origin".to_owned());
    bridge_args.push(bridge.origin.clone());
    let mut quoted = Vec::new();
    for arg in &bridge_args {
        quoted.push(toml_string("bridge argument", arg)?);
    }
    let home = bridge.home.home.as_ref().ok_or_else(|| AgentError::InvalidSetting {
        setting: "home folder",
        bytes: 0,
        reason: "the person's home folder is unknown, so the tool server could not find the control token".to_owned(),
    })?;
    let mut env = vec![format!("HOME={}", path_text("home folder", home)?)];
    if let Some(xdg) = &bridge.home.xdg_config_home {
        env.push(format!("XDG_CONFIG_HOME={}", path_text("XDG_CONFIG_HOME", xdg)?));
    }
    Ok(vec![
        format!("mcp_servers.{SERVER_NAME}.command={cli}"),
        format!("mcp_servers.{SERVER_NAME}.args=[{}]", quoted.join(",")),
        format!("mcp_servers.{SERVER_NAME}.env={{{}}}", env.join(",")),
        format!("mcp_servers.{SERVER_NAME}.startup_timeout_sec=10"),
        format!("mcp_servers.{SERVER_NAME}.tool_timeout_sec=60"),
    ])
}

/// `text` as a TOML basic string; a control character is refused (TOML would need it escaped and
/// none belongs in a path, a label or a name).
fn toml_string(what: &'static str, text: &str) -> Result<String, AgentError> {
    if text.chars().any(char::is_control) {
        return Err(setting(what, text, "it holds a control character"));
    }
    // With no control character left, JSON's escaping (of `"` and `\` alone) is TOML's.
    serde_json::to_string(text).map_err(|e| setting(what, text, &e.to_string()))
}

/// A refused setting, named by its length only: the value is never repeated.
fn setting(what: &'static str, value: &str, reason: &str) -> AgentError {
    AgentError::InvalidSetting { setting: what, bytes: value.len(), reason: reason.to_owned() }
}

/// Refuses a request timeout outside [`MIN_REQUEST_TIMEOUT`] to [`MAX_REQUEST_TIMEOUT`].
pub(crate) fn timeout_check(timeout: Duration) -> Result<(), AgentError> {
    if timeout < MIN_REQUEST_TIMEOUT || timeout > MAX_REQUEST_TIMEOUT {
        return Err(AgentError::InvalidLimit {
            limit: "request timeout (ms)",
            value: timeout.as_millis(),
            accepted: format!("{} to {}", MIN_REQUEST_TIMEOUT.as_millis(), MAX_REQUEST_TIMEOUT.as_millis()),
        });
    }
    Ok(())
}

/// Refuses an API key that holds anything but printable ASCII other than `"` and `\`, so it goes
/// into its JSON line exactly as it is, with no escaping and no copy; the key is never repeated.
pub(crate) fn checked_api_key(key: &Secret) -> Result<(), AgentError> {
    let fits = key.expose().bytes().all(|b| (0x21..=0x7e).contains(&b) && b != b'"' && b != b'\\');
    if fits {
        Ok(())
    } else {
        Err(AgentError::InvalidSetting {
            setting: "Codex API key",
            bytes: key.expose().len(),
            reason: "a key is printable ASCII without quotes or backslashes".to_owned(),
        })
    }
}

/// The API key's `account/login/start` line for request `id`, in a buffer of its final size that is
/// overwritten when dropped. The key must have passed [`checked_api_key`].
pub(crate) fn api_key_line(id: u64, key: &Secret) -> Zeroizing<String> {
    let mut line = Zeroizing::new(String::with_capacity(account::key_line_len(id, key.expose())));
    account::write_key_line(&mut line, id, key.expose());
    line
}

/// The handshake's `initialize`.
pub(crate) mod initialize {
    use serde_json::{Value, json};

    use crate::{AgentError, ErrorKind, Fit, RuntimeKind, Version};

    /// `initialize`'s parameters: Archforge's name and version, no experimental API.
    pub(crate) fn params() -> Value {
        json!({
            "clientInfo": { "name": "archforge", "title": "Archforge", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "experimentalApi": false, "requestAttestation": false },
        })
    }

    /// Checks `initialize`'s answer: its `userAgent` (`archforge/<codex version> …`) must be text;
    /// the version it names must be in Codex's accepted range (a version newer than the tested one
    /// is logged, one outside the range refused as a missing runtime). A version that cannot be
    /// read is logged; `find` checked the program's own version already.
    pub(crate) fn check(answer: &Value) -> Result<(), AgentError> {
        let agent = answer.get("userAgent").and_then(Value::as_str).ok_or_else(|| AgentError::Unreadable {
            runtime: RuntimeKind::Codex,
            request: "initialize",
            reason: "its userAgent is missing or not text".to_owned(),
        })?;
        let version = agent.split_once('/').and_then(|(_, rest)| rest.split(' ').next()).and_then(Version::parse);
        let Some(version) = version else {
            log::warn!("codex's handshake names no version Archforge can read");
            return Ok(());
        };
        let range = RuntimeKind::Codex.range();
        match range.fit(version) {
            Fit::Tested => Ok(()),
            Fit::NewerThanTested => {
                log::warn!("codex {version} is newer than {}, the version Archforge was tested with", range.tested);
                Ok(())
            }
            Fit::TooOld | Fit::TooNew => Err(AgentError::reported(
                ErrorKind::MissingRuntime,
                &format!("codex says it is version {version}, outside the versions Archforge works with ({} up to {})", range.floor, range.below),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ErrorKind;

    /// The command line carries the store, the switches and the bridge, read-only unless changes
    /// are allowed, and refuses values TOML would need escaped.
    #[test]
    fn the_command_line_carries_the_store_and_the_bridge() {
        let bridge = Bridge {
            cli: PathBuf::from("/opt/archforge/bin/archforge-cli"),
            port: 47000,
            origin: "Chat 1".to_owned(),
            allow_changes: false,
            home: BridgeHome { home: Some(PathBuf::from("/Users/person")), xdg_config_home: None },
        };
        let args: Vec<String> =
            command_line(CredentialStore::Keyring, Some(&bridge)).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(&args[..4], ["app-server", "--strict-config", "-c", "cli_auth_credentials_store=\"keyring\""]);
        for feature in DISABLED {
            assert!(args.windows(2).any(|w| w[0] == "--disable" && w[1] == feature), "{feature}");
        }
        assert!(args.contains(&"mcp_servers.archforge.command=\"/opt/archforge/bin/archforge-cli\"".to_owned()), "{args:?}");
        assert!(
            args.contains(&"mcp_servers.archforge.args=[\"mcp\",\"--bridge\",\"47000\",\"--read-only\",\"--origin\",\"Chat 1\"]".to_owned()),
            "{args:?}"
        );
        assert!(args.contains(&"mcp_servers.archforge.env={HOME=\"/Users/person\"}".to_owned()), "{args:?}");
        let allowed = Bridge { allow_changes: true, ..bridge.clone() };
        let args: Vec<String> =
            command_line(CredentialStore::Ephemeral, Some(&allowed)).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert!(!args.iter().any(|a| a.contains("--read-only")));
        assert!(args.contains(&"cli_auth_credentials_store=\"ephemeral\"".to_owned()));
        let account: Vec<String> = command_line(CredentialStore::File, None).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert!(!account.iter().any(|a| a.contains("mcp_servers")));
        for bad in [
            Bridge { cli: PathBuf::from("archforge-cli"), ..bridge.clone() },
            Bridge { port: 0, ..bridge.clone() },
            Bridge { origin: "-x".into(), ..bridge.clone() },
            Bridge { home: BridgeHome::default(), ..bridge.clone() },
            Bridge { cli: PathBuf::from("/opt/a\u{7}b"), ..bridge.clone() },
        ] {
            assert!(matches!(command_line(CredentialStore::Keyring, Some(&bad)), Err(AgentError::InvalidSetting { .. })), "{bad:?}");
        }
        assert_eq!(toml_string("x", "a\"b\\c").unwrap(), "\"a\\\"b\\\\c\"");
    }

    /// Request timeouts out of range and keys JSON would escape are refused, never repeated.
    #[test]
    fn timeouts_and_keys_are_checked() {
        assert!(timeout_check(DEFAULT_REQUEST_TIMEOUT).is_ok());
        assert!(timeout_check(Duration::from_millis(1)).is_err());
        assert!(timeout_check(MAX_REQUEST_TIMEOUT + Duration::from_millis(1)).is_err());
        assert!(checked_api_key(&Secret::new("sk-proj-AbC_123-xyz".into()).unwrap()).is_ok());
        for bad in ["sk \"quoted\"", "sk\\x", "sk\nline", "sk é"] {
            let error = checked_api_key(&Secret::new(bad.into()).unwrap()).unwrap_err();
            assert!(!format!("{error:?}{error}").contains(bad), "{error:?}");
        }
    }

    /// The handshake's version check accepts the tested and newer versions and refuses others.
    #[test]
    fn the_handshake_version_is_checked() {
        let agent = |v: &str| json!({ "userAgent": format!("archforge/{v} (Mac OS 27.2.0; arm64) unknown (archforge; 0.13.0)") });
        assert!(initialize::check(&agent("0.166.0")).is_ok());
        assert!(initialize::check(&agent("0.170.2")).is_ok());
        assert_eq!(initialize::check(&agent("0.165.9")).unwrap_err().kind(), ErrorKind::MissingRuntime);
        assert_eq!(initialize::check(&agent("1.0.0")).unwrap_err().kind(), ErrorKind::MissingRuntime);
        assert!(initialize::check(&json!({ "userAgent": "something else" })).is_ok());
        assert!(matches!(initialize::check(&json!({})), Err(AgentError::Unreadable { .. })));
    }
}
