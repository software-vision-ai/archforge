//! The identifier every object carries: a UUID, version 7 (a millisecond timestamp, then random
//! bits), so ids sort by creation and two branches never collide. The core makes one from the
//! parts the host supplies, since it has no clock and no randomness of its own.

use std::fmt;

/// Sixteen bytes; canonical text is the hyphenated lower-case form.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid(pub [u8; 16]);

impl Uuid {
    /// A version-7 UUID from a Unix time in milliseconds and 74 random bits (the low bits of
    /// `random` are used).
    pub fn v7(unix_ms: u64, random: [u8; 10]) -> Uuid {
        let mut b = [0u8; 16];
        let ms = unix_ms.min((1 << 48) - 1);
        b[..6].copy_from_slice(&ms.to_be_bytes()[2..]);
        b[6..].copy_from_slice(&random);
        b[6] = (b[6] & 0x0f) | 0x70;
        b[8] = (b[8] & 0x3f) | 0x80;
        Uuid(b)
    }

    /// The canonical text, or None for anything else: 36 characters, lower-case hex, hyphens at
    /// 8, 13, 18 and 23.
    pub fn parse(text: &str) -> Option<Uuid> {
        let bytes = text.as_bytes();
        if bytes.len() != 36 {
            return None;
        }
        let mut out = [0u8; 16];
        let mut n = 0;
        let mut i = 0;
        while i < 36 {
            if matches!(i, 8 | 13 | 18 | 23) {
                if bytes[i] != b'-' {
                    return None;
                }
                i += 1;
                continue;
            }
            let hex = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                _ => None,
            };
            let (hi, lo) = (hex(bytes[i])?, hex(*bytes.get(i + 1)?)?);
            *out.get_mut(n)? = (hi << 4) | lo;
            n += 1;
            i += 2;
        }
        Some(Uuid(out))
    }

    pub fn is_nil(&self) -> bool {
        self.0 == [0; 16]
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, b) in self.0.iter().enumerate() {
            if matches!(i, 4 | 6 | 8 | 10) {
                write!(f, "-")?;
            }
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Uuid({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v7_sorts_by_time_and_round_trips_through_text() {
        let a = Uuid::v7(1_761_670_800_000, [1; 10]);
        let b = Uuid::v7(1_761_670_800_001, [0; 10]);
        assert!(a < b);
        let text = a.to_string();
        assert_eq!(text.len(), 36);
        assert_eq!(&text[14..15], "7", "the version nibble");
        assert!(matches!(&text[19..20], "8" | "9" | "a" | "b"), "the variant bits");
        assert_eq!(Uuid::parse(&text), Some(a));
        for bad in ["", "x", &text.to_uppercase(), &text.replace('-', ""), &format!("{text} ")] {
            assert_eq!(Uuid::parse(bad), None, "{bad:?}");
        }
    }
}
