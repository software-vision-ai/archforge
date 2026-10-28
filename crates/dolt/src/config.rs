//! The server's configuration file. Every value is a double-quoted YAML string: `\` and `"` are
//! escaped, and `$` is written `$$`, since dolt expands `${VAR}` before it reads the file.

/// One value as a double-quoted YAML string.
pub(crate) fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '$' => out.push_str("$$"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Where the server's files go and how it listens.
pub(crate) struct Settings<'a> {
    pub port: u16,
    pub socket: Option<&'a str>,
    pub data_dir: &'a str,
    pub cfg_dir: &'a str,
    pub no_files: &'a str,
}

/// What a project folder from elsewhere must never make the server do: sync with a remote, which a
/// `config.json` planted in the data folder's `.dolt` turns on, or run scheduled events, which can
/// push (measured with dolt 2.6.1). System variables set here win over persisted ones.
const NO_REPLICATION: &[(&str, &str)] = &[
    ("dolt_replicate_to_remote", "\"\""),
    ("dolt_read_replica_remote", "\"\""),
    ("dolt_replicate_heads", "\"\""),
    ("dolt_replication_remote_url_template", "\"\""),
    ("dolt_replicate_all_heads", "0"),
    ("dolt_async_replication", "0"),
    ("dolt_skip_replication_errors", "0"),
    // `behavior.event_scheduler` alone loses to a planted `sqlserver.global.event_scheduler`.
    ("event_scheduler", "\"OFF\""),
];

/// `server.yaml`: loopback only, a socket on unix, a fresh privileges file for this start, no file
/// access from SQL, no scheduled events (an event can push to a remote) and no replication.
pub(crate) fn server_yaml(s: &Settings<'_>) -> String {
    let sep = if cfg!(windows) { '\\' } else { '/' };
    let mut y = String::new();
    y.push_str("log_level: warning\nbehavior:\n  autocommit: true\n  event_scheduler: \"OFF\"\nlistener:\n  host: \"127.0.0.1\"\n");
    y.push_str(&format!("  port: {}\n  max_connections: 16\n", s.port));
    if let Some(socket) = s.socket {
        y.push_str(&format!("  socket: {}\n", quote(socket)));
    }
    y.push_str(&format!("data_dir: {}\n", quote(s.data_dir)));
    y.push_str(&format!("cfg_dir: {}\n", quote(s.cfg_dir)));
    y.push_str(&format!("privilege_file: {}\n", quote(&format!("{}{sep}privileges.db", s.cfg_dir))));
    y.push_str(&format!("branch_control_file: {}\n", quote(&format!("{}{sep}branch_control.db", s.cfg_dir))));
    y.push_str(&format!("system_variables:\n  secure_file_priv: {}\n", quote(s.no_files)));
    for (name, value) in NO_REPLICATION {
        y.push_str(&format!("  {name}: {value}\n"));
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_quoted_and_dollars_doubled() {
        assert_eq!(quote(r#"C:\a "b" $HOME ${X} é"#), r#""C:\\a \"b\" $$HOME $${X} é""#);
        let y = server_yaml(&Settings {
            port: 52894,
            socket: Some("/t/af-x/db.sock"),
            data_dir: "/p/pro$ject",
            cfg_dir: "/t/af-x/doltcfg",
            no_files: "/t/af-x/nofiles",
        });
        assert!(y.contains("  port: 52894\n") && y.contains("  socket: \"/t/af-x/db.sock\"\n") && y.contains("data_dir: \"/p/pro$$ject\"\n"), "{y}");
        assert!(y.contains("secure_file_priv: \"/t/af-x/nofiles\""), "{y}");
        assert!(
            y.contains("  event_scheduler: \"OFF\"\n")
                && y.contains("  dolt_replicate_to_remote: \"\"\n")
                && y.contains("  dolt_async_replication: 0\n"),
            "{y}"
        );
    }
}
