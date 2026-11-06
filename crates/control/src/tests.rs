use super::*;

/// A fresh folder for one test's token file.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("archforge-control-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A server whose handler echoes its method and parameters, and its origin when there is one, a
/// panic for `boom`.
fn echo(name: &str) -> (Server, PathBuf) {
    let dir = folder(name);
    let handler: Handler = Arc::new(|method, params, origin| match method {
        "boom" => panic!("a handler that fails"),
        "refuse" => Err("refused on purpose".into()),
        _ if origin.is_some() => Ok(json!({ "method": method, "params": params, "origin": origin })),
        _ => Ok(json!({ "method": method, "params": params })),
    });
    (Server::start(0, &dir, handler).unwrap(), dir)
}

/// Sends raw `lines` on a fresh connection and gives what came back before it closed.
fn raw(port: u16, lines: &[&[u8]]) -> Vec<u8> {
    let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    for l in lines {
        let _ = s.write_all(l);
    }
    let _ = s.shutdown(std::net::Shutdown::Write);
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    out
}

#[test]
fn a_token_is_256_random_bits_compared_in_constant_time() {
    let (a, b) = (mint_token().unwrap(), mint_token().unwrap());
    assert!(is_token(&a) && is_token(&b) && a != b);
    assert!(same_token(&a, &a.clone()) && !same_token(&a, &b) && !same_token(&a, &a[..63]));
}

#[cfg(unix)]
#[test]
fn the_token_file_is_trusted_only_when_it_is_ours_alone() {
    use std::os::unix::fs::PermissionsExt;
    let dir = folder("file");
    let path = token_path(&dir, 4711);
    let token = mint_token().unwrap();
    write_token(&path, &token).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(read_token(&path).unwrap(), token);
    // An earlier start's file is replaced.
    let next = mint_token().unwrap();
    write_token(&path, &next).unwrap();
    assert_eq!(read_token(&path).unwrap(), next);
    // Readable by others: not trusted.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(read_token(&path).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    // A link to it: not trusted.
    let link = token_path(&dir, 4712);
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(read_token(&link).is_err());
    // Not a token: refused; a folder in its place: never removed.
    std::fs::write(token_path(&dir, 4713), "short").unwrap();
    std::fs::set_permissions(token_path(&dir, 4713), std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(read_token(&token_path(&dir, 4713)).is_err());
    std::fs::create_dir(token_path(&dir, 4714)).unwrap();
    assert!(write_token(&token_path(&dir, 4714), &token).is_err());
    assert!(token_path(&dir, 4714).is_dir());
}

#[cfg(unix)]
#[test]
fn a_settings_folder_others_may_write_to_is_closed_to_them() {
    use std::os::unix::fs::PermissionsExt;
    let dir = folder("umask");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o775)).unwrap();
    let handler: Handler = Arc::new(|_, _, _| Ok(json!({})));
    let server = Server::start(0, &dir, handler).unwrap();
    assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o755);
    drop(server);
}

#[test]
fn a_line_is_read_up_to_its_cap() {
    let mut r = BufReader::new(&b"one\r\ntwo\nthree"[..]);
    assert_eq!(read_line(&mut r, 10).unwrap().as_deref(), Some(&b"one"[..]));
    assert_eq!(read_line(&mut r, 10).unwrap().as_deref(), Some(&b"two"[..]));
    assert_eq!(read_line(&mut r, 10).unwrap().as_deref(), Some(&b"three"[..]));
    assert_eq!(read_line(&mut r, 10).unwrap(), None);
    let mut r = BufReader::with_capacity(4, &b"0123456789abc\n"[..]);
    assert!(read_line(&mut r, 10).is_err());
}

#[cfg(unix)]
#[test]
fn a_call_with_the_token_is_answered_and_anything_else_closes_the_connection() {
    let (server, dir) = echo("serve");
    let port = server.port();
    let client = Client::new(&dir, port).unwrap();
    assert_eq!(
        client.call("engine.commands", json!({ "filter": "er" })).unwrap(),
        json!({ "method": "engine.commands", "params": { "filter": "er" } })
    );
    assert_eq!(client.call("refuse", json!({})), Err(CallError::Refused("refused on purpose".into())));
    // A handler that panics answers with an error, and the channel goes on.
    assert!(matches!(client.call("boom", json!({})), Err(CallError::Refused(_))));
    assert!(client.call("again", json!({})).is_ok());
    // A wrong token, no token, a malformed or a huge first line: closed without a word.
    let wrong = Client::with_token(port, mint_token().unwrap());
    assert!(matches!(wrong.call("engine.commands", json!({})), Err(CallError::Unreachable(_))));
    assert!(raw(port, &[b"{\"id\":1,\"method\":\"x\"}\n"]).is_empty());
    assert!(raw(port, &[b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n"]).is_empty());
    let huge = vec![b'x'; MAX_LINE + 10];
    assert!(raw(port, &[&huge, b"\n"]).is_empty());
    // After the first line: an unknown key or a bad line is an error reply, the connection stays.
    let token = read_token(&token_path(&dir, port)).unwrap();
    let first = format!("{{\"id\":1,\"method\":\"a\",\"token\":\"{token}\"}}\n");
    let out =
        raw(port, &[first.as_bytes(), b"{\"id\":2,\"method\":\"b\",\"extra\":1}\n", b"not json\n", b"{\"id\":3,\"method\":\"c\",\"params\":[]}\n"]);
    let replies: Vec<Value> = out.split(|b| *b == b'\n').filter(|l| !l.is_empty()).map(|l| serde_json::from_slice(l).unwrap()).collect();
    assert_eq!(replies.len(), 4);
    assert_eq!(replies[0]["ok"], true);
    assert!(replies[1]["error"].as_str().unwrap().contains("unknown key \"extra\""));
    assert_eq!(replies[2]["ok"], false);
    assert!(replies[3]["error"].as_str().unwrap().contains("params"));
    // Dropping the server removes its token file.
    drop(server);
    assert!(!token_path(&dir, port).exists());
}

#[cfg(unix)]
#[test]
fn past_the_connection_cap_a_connection_is_closed_at_once() {
    let (server, _dir) = echo("cap");
    let port = server.port();
    let idle: Vec<TcpStream> = (0..MAX_CONNECTIONS).map(|_| TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap()).collect();
    std::thread::sleep(Duration::from_millis(200));
    let mut extra = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    extra.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = [0u8; 1];
    assert_eq!(extra.read(&mut buf).unwrap_or(0), 0, "closed at once");
    drop(idle);
}

#[cfg(unix)]
#[test]
fn a_peer_that_drips_its_first_line_is_closed_on_time() {
    let (server, _dir) = echo("drip");
    let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port())).unwrap();
    s.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let started = Instant::now();
    let mut closed = false;
    while started.elapsed() < Duration::from_secs(4) {
        // A byte now and then, never a line break.
        if s.write_all(b"{").is_err() {
            closed = true;
            break;
        }
        let mut buf = [0u8; 1];
        if let Ok(0) = s.read(&mut buf) {
            closed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    assert!(closed, "a first line dripped in is cut off at its deadline");
    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
}

#[test]
fn an_origin_is_short_printable_text_and_anything_else_is_refused_by_name() {
    for good in ["Chat 1", "Chat · Orders", "Bestellungen prüfen", "注文", "a", &"x".repeat(MAX_ORIGIN)] {
        assert_eq!(check_origin(good), Ok(()), "{good:?}");
    }
    assert_eq!(check_origin(""), Err(OriginError::Empty));
    assert_eq!(check_origin("   "), Err(OriginError::Empty));
    assert_eq!(check_origin(&"x".repeat(MAX_ORIGIN + 1)), Err(OriginError::TooLong { chars: MAX_ORIGIN + 1 }));
    // Counted in characters, not bytes.
    assert_eq!(check_origin(&"é".repeat(MAX_ORIGIN)), Ok(()));
    assert_eq!(check_origin(" Chat"), Err(OriginError::Padded));
    assert_eq!(check_origin("Chat "), Err(OriginError::Padded));
    for (bad, at, code) in [
        ("Chat\n1", 5, 0x0A),
        ("Chat\t1", 5, 0x09),
        ("\u{1b}[31mChat", 1, 0x1B),
        ("Chat\u{202E}1", 5, 0x202E),
        ("Chat\u{200B}1", 5, 0x200B),
        ("Chat\u{2028}1", 5, 0x2028),
        ("Chat\u{00A0}1", 5, 0xA0),
        ("\u{FEFF}Chat", 1, 0xFEFF),
        ("Chat\u{E0041}", 5, 0xE0041),
        ("Chat\u{180E}", 5, 0x180E),
        ("Chat\u{1BCA0}", 5, 0x1BCA0),
        ("Chat\u{1D173}", 5, 0x1D173),
    ] {
        assert_eq!(check_origin(bad), Err(OriginError::NotPrintable { at, code }), "{bad:?}");
    }
    // Every refusal says what was wrong.
    let said = check_origin("Chat\u{202E}1").unwrap_err().to_string();
    assert!(said.contains("U+202E") && said.contains("printable"), "{said}");
    assert!(check_origin(&"x".repeat(65)).unwrap_err().to_string().contains("at most 64"));
    // A client is never made with an origin the server would refuse.
    let client = Client::with_token(1, mint_token().unwrap());
    assert!(client.with_origin(Some("Chat\n2".into())).is_err());
}

#[cfg(unix)]
#[test]
fn the_origin_travels_beside_the_params_and_a_bad_one_is_refused() {
    let (server, dir) = echo("origin");
    let port = server.port();
    let chat = Client::new(&dir, port).unwrap().with_origin(Some("Chat 1".into())).unwrap();
    assert_eq!(
        chat.call("engine.execute", json!({ "command": "x" })).unwrap(),
        json!({ "method": "engine.execute", "params": { "command": "x" }, "origin": "Chat 1" })
    );
    // Without one, the handler gets none.
    let plain = Client::new(&dir, port).unwrap();
    assert_eq!(plain.call("m", json!({})).unwrap(), json!({ "method": "m", "params": {} }));
    // A bad origin on the wire is an error reply naming it, never passed on.
    let token = read_token(&token_path(&dir, port)).unwrap();
    let first = format!("{{\"id\":1,\"method\":\"a\",\"token\":\"{token}\"}}\n");
    let long = format!("{{\"id\":3,\"method\":\"c\",\"origin\":\"{}\"}}\n", "x".repeat(MAX_ORIGIN + 1));
    let out = raw(
        port,
        &[
            first.as_bytes(),
            b"{\"id\":2,\"method\":\"b\",\"origin\":\"Chat\\n1\"}\n",
            long.as_bytes(),
            b"{\"id\":4,\"method\":\"d\",\"origin\":7}\n",
            b"{\"id\":5,\"method\":\"e\",\"origin\":\"Chat 2\"}\n",
        ],
    );
    let replies: Vec<Value> = out.split(|b| *b == b'\n').filter(|l| !l.is_empty()).map(|l| serde_json::from_slice(l).unwrap()).collect();
    assert_eq!(replies.len(), 5);
    assert!(replies[1]["error"].as_str().unwrap().contains("U+000A"), "{}", replies[1]);
    assert!(replies[2]["error"].as_str().unwrap().contains("at most 64"), "{}", replies[2]);
    assert!(replies[3]["error"].as_str().unwrap().contains("origin must be a string"), "{}", replies[3]);
    assert_eq!(replies[4]["result"]["origin"], "Chat 2");
}

#[test]
fn a_request_too_large_is_refused_before_it_is_sent() {
    let client = Client::with_token(1, mint_token().unwrap());
    let big = json!({ "text": "x".repeat(MAX_REQUEST) });
    assert!(matches!(client.call("engine.execute", big), Err(CallError::Refused(e)) if e.contains("larger")));
}

/// A small deterministic random source for the fuzz test.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[cfg(unix)]
#[test]
fn hostile_lines_never_bring_the_channel_down() {
    let (server, dir) = echo("fuzz");
    let port = server.port();
    let token = read_token(&token_path(&dir, port)).unwrap();
    let first = format!("{{\"id\":0,\"method\":\"hello\",\"token\":\"{token}\"}}\n");
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let shapes: [&[u8]; 9] = [
        b"{}",
        b"[]",
        b"null",
        b"{\"id\":{\"deep\":[1,2,{}]},\"method\":7}",
        b"{\"method\":\"x\",\"params\":\"text\"}",
        b"{\"method\":\"x\",\"token\":\"\"}",
        b"\xff\xfe\x00garbage",
        b"{\"method\":\"\\u0000\"}",
        b"{\"id\":1e999,\"method\":\"x\"}",
    ];
    for round in 0..40 {
        let mut lines: Vec<Vec<u8>> = vec![first.clone().into_bytes()];
        for _ in 0..20 {
            let line: Vec<u8> = match rng.next() % 4 {
                0 => (0..(rng.next() % 300)).map(|_| (rng.next() % 256) as u8).filter(|b| *b != b'\n').collect(),
                1 => shapes[(rng.next() % shapes.len() as u64) as usize].to_vec(),
                2 => {
                    let depth = (rng.next() % 5000) as usize;
                    let mut v = vec![b'['; depth];
                    v.extend(std::iter::repeat_n(b']', depth));
                    v
                }
                _ => format!("{{\"id\":{},\"method\":\"m{}\",\"params\":{{\"k\":{}}}}}", rng.next(), rng.next() % 9, rng.next()).into_bytes(),
            };
            let mut line = line;
            line.push(b'\n');
            lines.push(line);
        }
        // Half the rounds start without the token: those connections close at once.
        if round % 2 == 1 {
            lines.remove(0);
        }
        let refs: Vec<&[u8]> = lines.iter().map(Vec::as_slice).collect();
        let _ = raw(port, &refs);
    }
    // The channel still answers.
    let client = Client::new(&dir, port).unwrap();
    assert!(client.call("still.there", json!({})).is_ok());
}
