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

/// `server.yaml`: loopback only, a socket on unix, a fresh privileges file for this start, and no
/// file access from SQL.
pub(crate) fn server_yaml(s: &Settings<'_>) -> String {
    let sep = if cfg!(windows) { '\\' } else { '/' };
    let mut y = String::new();
    y.push_str("log_level: warning\nbehavior:\n  autocommit: true\nlistener:\n  host: \"127.0.0.1\"\n");
    y.push_str(&format!("  port: {}\n  max_connections: 16\n", s.port));
    if let Some(socket) = s.socket {
        y.push_str(&format!("  socket: {}\n", quote(socket)));
    }
    y.push_str(&format!("data_dir: {}\n", quote(s.data_dir)));
    y.push_str(&format!("cfg_dir: {}\n", quote(s.cfg_dir)));
    y.push_str(&format!("privilege_file: {}\n", quote(&format!("{}{sep}privileges.db", s.cfg_dir))));
    y.push_str(&format!("branch_control_file: {}\n", quote(&format!("{}{sep}branch_control.db", s.cfg_dir))));
    y.push_str(&format!("system_variables:\n  secure_file_priv: {}\n", quote(s.no_files)));
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
    }
}
