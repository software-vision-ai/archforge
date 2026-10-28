//! Path checks: plain text only, and the socket path's length limit.

use std::path::Path;

use crate::Error;

/// The path as text, refused when it is not UTF-8 or holds a control character, so it can go
/// into the server's configuration and be compared exactly.
pub(crate) fn text<'a>(what: &'static str, path: &'a Path) -> Result<&'a str, Error> {
    match path.to_str() {
        Some(s) if !s.chars().any(char::is_control) => Ok(s),
        _ => Err(Error::BadPath { what, path: path.to_string_lossy().into_owned() }),
    }
}

/// The longest socket path the platform takes: `sun_path` less its terminator.
pub(crate) const SOCKET_MAX: usize = if cfg!(target_os = "macos") { 103 } else { 107 };

/// Refuses a socket path the platform would cut, which dolt skips without failing.
pub(crate) fn socket_fits(path: &Path) -> Result<(), Error> {
    let len = path.as_os_str().len();
    if len > SOCKET_MAX { Err(Error::SocketPathTooLong { path: path.to_path_buf(), len, max: SOCKET_MAX }) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_are_refused_and_the_socket_limit_holds_at_its_edge() {
        assert_eq!(text("data folder", Path::new("/a/pro$ject \"x\" é")).unwrap(), "/a/pro$ject \"x\" é");
        assert!(matches!(text("data folder", Path::new("/a/b\nc")), Err(Error::BadPath { .. })));
        let at = format!("/{}", "s".repeat(SOCKET_MAX - 1));
        assert!(socket_fits(Path::new(&at)).is_ok());
        let over = format!("{at}s");
        assert!(matches!(socket_fits(Path::new(&over)), Err(Error::SocketPathTooLong { len, max, .. }) if len == max + 1));
    }
}
