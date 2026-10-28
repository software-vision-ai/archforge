//! The server's password, and random names. A secret never shows in a log or a message.

use std::fmt;

use crate::Error;

/// A password made for one start of the server.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// 32 random bytes, written as hex.
    pub(crate) fn generate() -> Result<Secret, Error> {
        let mut bytes = [0u8; 32];
        fill(&mut bytes)?;
        Ok(Secret(bytes.iter().map(|b| format!("{b:02x}")).collect()))
    }

    /// The password itself, for the server's environment and the connection only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<hidden>")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<hidden>")
    }
}

/// `len` random characters from `[a-z0-9]`.
pub(crate) fn random_name(len: usize) -> Result<String, Error> {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut bytes = vec![0u8; len];
    fill(&mut bytes)?;
    Ok(bytes.iter().map(|b| char::from(ALPHABET.get(usize::from(*b) % ALPHABET.len()).copied().unwrap_or(b'a'))).collect())
}

fn fill(bytes: &mut [u8]) -> Result<(), Error> {
    getrandom::fill(bytes).map_err(|e| Error::io("cannot read random bytes from", "the operating system", std::io::Error::other(e.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_shows_and_names_use_the_alphabet() {
        let s = Secret::generate().unwrap();
        assert_eq!(s.expose().len(), 64);
        assert_eq!(format!("{s} {s:?}"), "<hidden> <hidden>");
        let e = Error::StartFailed { status: "exit 1".into(), log_tail: String::new() };
        assert!(!format!("{e} {e:?}").contains(s.expose()));
        let n = random_name(8).unwrap();
        assert!(n.len() == 8 && n.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "{n}");
    }
}
