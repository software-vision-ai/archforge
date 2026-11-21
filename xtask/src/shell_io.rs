//! The shell's part of the layer check: the interface crate (`ui-egui`) holds no process or network
//! code (agent-control-surfaces CS-17), so the window can never start a program or open a socket
//! itself; the host does, through `Services`.
//!
//! Two rules. A process or network crate is refused as a dependency of the shell ([`IO_CRATES`]).
//! And every Rust source under the shell's `src` is scanned: with its string and character literals
//! and its comments blanked, any path through `std::process`, `std::net` or `std::os::unix::net`
//! (written out, or inside a grouped import such as `use std::{io, net::TcpStream}`), and any of the
//! socket types by name ([`SOCKETS`]), is a finding with its file and line.

use std::path::{Path, PathBuf};

/// Crates that start processes or talk over a network, refused as dependencies of the shell.
pub const IO_CRATES: &[&str] = &[
    "async-std",
    "attohttpc",
    "command-group",
    "curl",
    "duct",
    "hyper",
    "interprocess",
    "isahc",
    "libc",
    "mio",
    "native-tls",
    "nix",
    "openssl",
    "reqwest",
    "rustix",
    "rustls",
    "smol",
    "socket2",
    "subprocess",
    "surf",
    "tokio",
    "tungstenite",
    "ureq",
];

/// The paths a shell source may not reach through.
const PATHS: [&[&str]; 3] = [&["std", "process"], &["std", "net"], &["std", "os", "unix", "net"]];

/// Socket types, refused by name wherever they come from.
pub const SOCKETS: [&str; 6] = ["TcpStream", "TcpListener", "UdpSocket", "UnixStream", "UnixListener", "UnixDatagram"];

/// Whether `name` is a process or network crate.
pub fn is_io_crate(name: &str) -> bool {
    IO_CRATES.contains(&name)
}

/// `source` with every string, byte string, raw string and character literal and every comment
/// replaced by spaces, line breaks kept, so a finding's line is its line in the file. A lifetime
/// (`'a`) is left as it is.
pub fn blank(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    // Writes the characters from `from` to `to` as blanks, keeping line breaks.
    let blanked = |out: &mut String, from: usize, to: usize| {
        for c in chars.get(from..to.min(chars.len())).unwrap_or_default() {
            out.push(if *c == '\n' { '\n' } else { ' ' });
        }
    };
    while let Some(&c) = chars.get(i) {
        let next = chars.get(i + 1).copied();
        let prev_ident = i > 0 && chars.get(i - 1).is_some_and(|p| p.is_alphanumeric() || *p == '_');
        if c == '/' && next == Some('/') {
            let end = chars.iter().skip(i).position(|c| *c == '\n').map_or(chars.len(), |n| i + n);
            blanked(&mut out, i, end);
            i = end;
        } else if c == '/' && next == Some('*') {
            // Block comments nest.
            let (mut depth, mut j) = (0usize, i);
            while j < chars.len() {
                match (chars.get(j), chars.get(j + 1)) {
                    (Some('/'), Some('*')) => {
                        depth += 1;
                        j += 2;
                    }
                    (Some('*'), Some('/')) => {
                        depth = depth.saturating_sub(1);
                        j += 2;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => j += 1,
                }
            }
            blanked(&mut out, i, j);
            i = j;
        } else if !prev_ident && (c == 'r' || (c == 'b' && next == Some('r'))) && raw_start(&chars, i).is_some() {
            let (open, hashes) = raw_start(&chars, i).unwrap_or((i, 0));
            // The closing quote followed by as many hashes.
            let mut j = open + 1;
            while j < chars.len() {
                if chars.get(j) == Some(&'"') && (1..=hashes).all(|h| chars.get(j + h) == Some(&'#')) {
                    j += hashes + 1;
                    break;
                }
                j += 1;
            }
            blanked(&mut out, i, j);
            i = j;
        } else if c == '"' || (c == 'b' && next == Some('"') && !prev_ident) {
            let mut j = if c == 'b' { i + 2 } else { i + 1 };
            while j < chars.len() {
                match chars.get(j) {
                    Some('\\') => j += 2,
                    Some('"') => {
                        j += 1;
                        break;
                    }
                    _ => j += 1,
                }
            }
            blanked(&mut out, i, j);
            i = j;
        } else if c == '\'' || (c == 'b' && next == Some('\'') && !prev_ident) {
            let start = if c == 'b' { i + 1 } else { i };
            match char_end(&chars, start) {
                Some(end) => {
                    blanked(&mut out, i, end);
                    i = end;
                }
                // A lifetime or a label.
                None => {
                    out.push(c);
                    i += 1;
                }
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Where a raw string starting at `i` (`r"`, `r#"`, `br"`, …) opens its quote, and its hashes.
fn raw_start(chars: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = if chars.get(i) == Some(&'b') { i + 2 } else { i + 1 };
    let mut hashes = 0;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    (chars.get(j) == Some(&'"')).then_some((j, hashes))
}

/// The end of a character literal opening at `i` (its quote), or none for a lifetime.
fn char_end(chars: &[char], i: usize) -> Option<usize> {
    match chars.get(i + 1)? {
        '\\' => {
            // A unicode escape runs to its closing brace, a byte escape (`\x7f`) takes two digits,
            // and the others one character; anything else is no character literal.
            let close = match chars.get(i + 2)? {
                'u' if chars.get(i + 3) == Some(&'{') => (i + 4..i + 12).find(|j| chars.get(*j) == Some(&'}'))?.saturating_add(1),
                'x' => i + 5,
                'n' | 'r' | 't' | '\\' | '0' | '\'' | '"' => i + 3,
                _ => return None,
            };
            (chars.get(close) == Some(&'\'')).then_some(close + 1)
        }
        _ => (chars.get(i + 2) == Some(&'\'')).then_some(i + 3),
    }
}

/// A token of blanked source: a name, `::`, a brace, a comma, a `*` or anything else, with its line.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Name(String),
    Path,
    Open,
    Close,
    Comma,
    Star,
    Other,
}

/// The tokens of `blanked`, each with its line (from 1).
fn tokens(blanked: &str) -> Vec<(Token, usize)> {
    let chars: Vec<char> = blanked.chars().collect();
    let (mut out, mut line, mut i) = (Vec::new(), 1usize, 0usize);
    while let Some(&c) = chars.get(i) {
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while chars.get(i).is_some_and(|c| c.is_alphanumeric() || *c == '_') {
                i += 1;
            }
            out.push((Token::Name(chars.get(start..i).unwrap_or_default().iter().collect()), line));
        } else if c == ':' && chars.get(i + 1) == Some(&':') {
            out.push((Token::Path, line));
            i += 2;
        } else {
            out.push((
                match c {
                    '{' => Token::Open,
                    '}' => Token::Close,
                    ',' => Token::Comma,
                    '*' => Token::Star,
                    _ => Token::Other,
                },
                line,
            ));
            i += 1;
        }
    }
    out
}

/// How deep groups may nest in one path tree before the scan stops following it.
const MAX_DEPTH: usize = 32;

/// The full paths of the path tree starting at `at` (a name), expanding groups; gives them and
/// where the tree ends. `prefix` is the path so far, `depth` how many groups it is inside; `cut`
/// is set when the budget or the nesting cap stopped the tree, so the scan fails closed.
fn tree(
    tokens: &[(Token, usize)],
    mut at: usize,
    prefix: Vec<String>,
    depth: usize,
    budget: &mut usize,
    cut: &mut bool,
) -> (Vec<Vec<String>>, usize) {
    let mut path = prefix;
    loop {
        // Every step costs, and nesting is capped: a damaged file cannot make the scan loop for
        // long or recurse deep.
        if *budget == 0 || depth > MAX_DEPTH {
            *cut = true;
            return (vec![path], at);
        }
        *budget -= 1;
        match tokens.get(at).map(|(t, _)| t) {
            Some(Token::Name(n)) => {
                path.push(n.clone());
                at += 1;
            }
            Some(Token::Open) => {
                let mut all = Vec::new();
                at += 1;
                loop {
                    match tokens.get(at).map(|(t, _)| t) {
                        Some(Token::Close) => return (all, at + 1),
                        Some(Token::Comma) => at += 1,
                        None => return (all, at),
                        Some(_) => {
                            let (paths, end) = tree(tokens, at, path.clone(), depth + 1, budget, cut);
                            all.extend(paths);
                            at = end.max(at + 1);
                        }
                    }
                }
            }
            // A glob keeps its `*`, so what it brings in is judged by its stem.
            Some(Token::Star) => {
                path.push("*".into());
                return (vec![path], at + 1);
            }
            _ => return (vec![path], at),
        }
        match tokens.get(at).map(|(t, _)| t) {
            Some(Token::Path) => at += 1,
            // A rename keeps an `as` mark; the new name is skipped.
            Some(Token::Name(a)) if a == "as" => {
                path.push("as".into());
                return (vec![path], at + 2);
            }
            _ => return (vec![path], at),
        }
    }
}

/// How a path of a `std` tree brings things in: written out, through a glob, or renamed.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Brings {
    Path,
    Glob,
    Renamed,
}

/// A path's stem (without its glob or rename mark and a trailing `self`) and how it brings things in.
fn stem(path: &[String]) -> (&[String], Brings) {
    let (rest, brings) = match path.split_last() {
        Some((last, rest)) if last == "*" => (rest, Brings::Glob),
        Some((last, rest)) if last == "as" => (rest, Brings::Renamed),
        _ => (path, Brings::Path),
    };
    match rest.split_last() {
        Some((last, before)) if last == "self" => (before, brings),
        _ => (rest, brings),
    }
}

/// Whether `stem` starts with a refused path.
fn refused(stem: &[String]) -> bool {
    PATHS.iter().any(|f| stem.len() >= f.len() && stem.iter().zip(f.iter()).all(|(a, b)| a == b))
}

/// The refused path `stem` leads to, when it is the start of one (`std`, `std::os`, `std::os::unix`).
fn leads_to(stem: &[String]) -> Option<String> {
    PATHS.iter().find(|f| stem.len() < f.len() && stem.iter().zip(f.iter()).all(|(a, b)| a == b)).map(|f| f.join("::"))
}

/// The finding for one path of a `std` tree, if it reaches a refused path: written out, through a
/// glob, or by bringing in (or renaming) a module on the way to one.
fn judge(path: &[String]) -> Option<String> {
    let (stem, brings) = stem(path);
    let shown = path.iter().filter(|s| *s != "as").cloned().collect::<Vec<_>>().join("::");
    if refused(stem) {
        return Some(shown);
    }
    let reached = leads_to(stem)?;
    Some(match brings {
        Brings::Renamed => format!("{} renamed, which hides its paths from this check", stem.join("::")),
        Brings::Glob => format!("{shown}, a glob that reaches {reached}"),
        Brings::Path => format!("{shown} brought in by name, which reaches {reached}"),
    })
}

/// The process and network uses in one source file's text, each `line N: …`.
pub fn findings(source: &str) -> Vec<String> {
    let tokens = tokens(&blank(source));
    let mut out = Vec::new();
    let mut budget = tokens.len().saturating_mul(4).saturating_add(64);
    let mut i = 0;
    while let Some((token, line)) = tokens.get(i) {
        match token {
            Token::Name(n) if SOCKETS.contains(&n.as_str()) => out.push(format!("line {line}: the socket type {n}")),
            // Renaming the standard library would hide its paths from the scan.
            Token::Name(n) if n == "std" && matches!(tokens.get(i + 1), Some((Token::Name(a), _)) if a == "as") => {
                out.push(format!("line {line}: std renamed, which hides its paths from this check"));
            }
            Token::Name(n) if n == "std" && tokens.get(i + 1).map(|(t, _)| t) == Some(&Token::Path) => {
                let mut cut = false;
                let (paths, end) = tree(&tokens, i, Vec::new(), 0, &mut budget, &mut cut);
                out.extend(paths.iter().filter_map(|p| judge(p)).map(|f| format!("line {line}: {f}")));
                if cut {
                    out.push(format!("line {line}: a path too long or too deeply nested to follow"));
                }
                i = end.max(i + 1);
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Every `.rs` file under `dir`, sorted.
fn sources(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))? {
            let path = entry.map_err(|e| format!("{}: {e}", d.display()))?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|x| x == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The process and network uses in every source under `dir`, each named with its file.
pub fn scan(dir: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for file in sources(dir)? {
        let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let rel = file.strip_prefix(dir).unwrap_or(&file).display().to_string();
        out.extend(
            findings(&text).into_iter().map(|f| format!("{rel} {f}: the shell holds no process or network code; the host does it through Services")),
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Written-out paths and socket types are found, each with its line.
    #[test]
    fn written_out_paths_and_socket_types_are_found_with_their_lines() {
        let source = "fn a() {\n    let _ = std::process::Command::new(\"ls\");\n}\nfn b(s: TcpStream) {}\nuse std :: net :: SocketAddr;\nuse std::os::unix::net::UnixStream as U;\n";
        let found = findings(source);
        assert_eq!(
            found,
            vec![
                "line 2: std::process::Command::new",
                "line 4: the socket type TcpStream",
                "line 5: std::net::SocketAddr",
                "line 6: std::os::unix::net::UnixStream",
            ]
        );
    }

    /// Grouped imports are expanded to their full paths.
    #[test]
    fn grouped_imports_are_expanded() {
        assert_eq!(findings("use std::{io, net::Ipv4Addr};"), vec!["line 1: std::net::Ipv4Addr"]);
        assert_eq!(findings("use std::{\n    fmt,\n    process,\n};"), vec!["line 1: std::process"]);
        assert_eq!(findings("use std::{os::{unix::{net::{self}}}, sync::Arc};"), vec!["line 1: std::os::unix::net::self"]);
        assert_eq!(findings("use std::{io::{Read, Write}, sync::{Arc, Mutex}};"), Vec::<String>::new());
        assert_eq!(findings("use std::process::*;"), vec!["line 1: std::process::*"]);
    }

    /// A glob, a module brought in by name or renamed on the way to a refused path, in any group,
    /// is a finding; a glob or a module elsewhere in std is not.
    #[test]
    fn globs_and_modules_on_the_way_to_a_refused_path_are_found() {
        assert_eq!(findings("use std::net::*;"), vec!["line 1: std::net::*"]);
        assert_eq!(findings("use std::{io, net::*};"), vec!["line 1: std::net::*"]);
        assert_eq!(findings("use std::*;"), vec!["line 1: std::*, a glob that reaches std::process"]);
        assert_eq!(findings("use std::{*};"), vec!["line 1: std::*, a glob that reaches std::process"]);
        assert_eq!(
            findings("use std::os::*; fn f(_: unix::net::UnixListener) {}"),
            vec!["line 1: std::os::*, a glob that reaches std::os::unix::net", "line 1: the socket type UnixListener"]
        );
        assert_eq!(findings("use std::{os::{unix::*}};"), vec!["line 1: std::os::unix::*, a glob that reaches std::os::unix::net"]);
        assert_eq!(findings("use std::os; os::unix::net::X;"), vec!["line 1: std::os brought in by name, which reaches std::os::unix::net"]);
        assert_eq!(findings("use std::os::{self};"), vec!["line 1: std::os::self brought in by name, which reaches std::os::unix::net"]);
        assert_eq!(findings("use std::os as o;"), vec!["line 1: std::os renamed, which hides its paths from this check"]);
        assert_eq!(findings("use std::os::unix::{self as u};"), vec!["line 1: std::os::unix renamed, which hides its paths from this check"]);
        assert_eq!(findings("use std::{net as n};"), vec!["line 1: std::net"]);
        // Elsewhere in std a glob, a module or a rename reaches nothing refused.
        assert_eq!(findings("use std::collections::*; use std::os::unix::fs::PermissionsExt; use std::{fmt, io::*};"), Vec::<String>::new());
    }

    /// Strings, characters and comments never count, and lifetimes do not throw the blanking off.
    #[test]
    fn strings_characters_and_comments_are_blanked_and_lifetimes_kept() {
        let source = r##"
            // std::process::Command in a comment
            /* std::net /* nested */ TcpStream */
            /// TcpListener in a doc comment
            fn f<'a>(x: &'a str) -> &'a str { let _ = ('"', '\'', b'x', '\u{1F600}'); "std::net::TcpStream" }
            const R: &str = r#"std::process "quoted" UnixStream"#;
            const B: &[u8] = b"UdpSocket";
            const BR: &[u8] = br"UdpSocket";
        "##;
        assert_eq!(findings(source), Vec::<String>::new());
        let blanked = blank("let s = \"a\nb\"; std::net::X");
        assert_eq!(blanked.lines().count(), 2, "line breaks are kept");
        assert_eq!(findings("let s = \"a\nb\"; std::net::X"), vec!["line 2: std::net::X"]);
        // A name that merely ends in r or b before a quote is not a raw or byte string.
        assert_eq!(findings("let br = 1; let r = br; std::process::exit(0)"), vec!["line 1: std::process::exit"]);
    }

    /// An escaped quote ends its character literal where it should, and a renamed std is a finding.
    #[test]
    fn an_escaped_quote_ends_its_character_and_a_renamed_std_is_found() {
        // Read short, the escaped quote would leave a stray quote that opens a string swallowing the
        // code after it.
        assert_eq!(findings("let q = ['\\'','\"']; std::process::exit(0); let s = \"x\";"), vec!["line 1: std::process::exit"]);
        assert_eq!(findings("let c = '\\u{1F600}'; let s = \"std::net\"; std::net::X"), vec!["line 1: std::net::X"]);
        assert_eq!(findings("use std as s; s::process::exit(0);"), vec!["line 1: std renamed, which hides its paths from this check"]);
        assert_eq!(findings("extern crate std as x;"), vec!["line 1: std renamed, which hides its paths from this check"]);
        assert_eq!(findings("use std::{self as s, io}; s::process::exit(0);"), vec!["line 1: std renamed, which hides its paths from this check"]);
        // Renaming something that leads nowhere refused is no finding.
        assert_eq!(findings("use std::fmt::{self as f, Result as FmtResult};"), Vec::<String>::new());
        // A byte escape is four characters long, and the scan reads past it as the compiler does.
        assert_eq!(findings("let q = ['\\x41','\"']; std::process::exit(0); let s = \"x\";"), vec!["line 1: std::process::exit"]);
        assert_eq!(findings("let b = b'\\x7f'; let s = \"std::net\"; std::net::X"), vec!["line 1: std::net::X"]);
    }

    /// A damaged file never hangs the scan, and what it cannot follow is a finding.
    #[test]
    fn a_damaged_file_never_hangs_the_scan_and_fails_closed() {
        let deep = format!("use std::{}", "{a::".repeat(10_000));
        assert!(findings(&deep).iter().any(|f| f.contains("too deeply nested")), "a path the scan cannot follow is a finding");
        let _ = findings("use std::{{{{{{{{{{");
        let _ = findings("'");
        let _ = findings("r#\"never closed");
        let _ = findings("/* never closed");
    }

    /// The process and network crates are known, the shell's own are not among them.
    #[test]
    fn process_and_network_crates_are_known() {
        for name in ["tokio", "reqwest", "libc", "rustix", "socket2", "mio", "nix"] {
            assert!(is_io_crate(name), "{name}");
        }
        for name in ["egui", "serde_json", "log", "zeroize"] {
            assert!(!is_io_crate(name), "{name}");
        }
    }

    /// The shell as it stands holds no process or network code.
    #[test]
    fn the_shell_holds_no_process_or_network_code() {
        let shell = crate::util::root().join("crates").join(crate::layers::SHELL).join("src");
        assert_eq!(scan(&shell).unwrap(), Vec::<String>::new());
    }
}
