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
    let handler: Handler = Arc::new(|call: Call<'_>| match call.method {
        "boom" => panic!("a handler that fails"),
        "refuse" => Err("refused on purpose".into()),
        method if call.origin.is_some() => Ok(json!({ "method": method, "params": call.params, "origin": call.origin })),
        method => Ok(json!({ "method": method, "params": call.params })),
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
    let handler: Handler = Arc::new(|_: Call<'_>| Ok(json!({})));
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

/// A handler that answers each call with its role and whether its token is live, and holds a call
/// to `wait` until the token is revoked (as a request queued for the window waits).
fn roles() -> Handler {
    Arc::new(|call: Call<'_>| {
        if call.method == "wait" {
            let started = Instant::now();
            while call.live.load(Ordering::SeqCst) && started.elapsed() < Duration::from_secs(10) {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        Ok(json!({ "role": format!("{:?}", call.role), "live": call.live.load(Ordering::SeqCst) }))
    })
}

/// Opens a connection, sends `first` and reads its reply, keeping the connection.
fn held(port: u16, first: &str) -> (TcpStream, BufReader<TcpStream>, Value) {
    let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(first.as_bytes()).unwrap();
    let mut r = BufReader::new(s.try_clone().unwrap());
    let reply = read_line(&mut r, MAX_REPLY).unwrap().map(|l| serde_json::from_slice(&l).unwrap()).unwrap_or(Value::Null);
    (s, r, reply)
}

/// Whether anything listens on `port` of the loopback interface.
fn listens(port: u16) -> bool {
    TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), Duration::from_millis(300)).is_ok()
}

/// Each grant has its own private file and role; revoking one answers its held connection and its waiting request, removes its file and leaves the others.
#[cfg(unix)]
#[test]
fn each_grant_has_its_own_file_and_role_and_a_revoked_one_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let dir = folder("grants");
    let server = Server::open(&dir, roles()).unwrap();
    let port = server.port();
    assert_eq!(server.grants(), 0, "an opening starts with no token");
    let read = server.grant(Role::ReadOnly).unwrap();
    let full = server.grant(Role::Full).unwrap();
    for g in [&read, &full] {
        let name = g.path().file_name().unwrap().to_str().unwrap().to_string();
        let hex = name.strip_prefix(&format!("control-{port}-")).and_then(|r| r.strip_suffix(".token")).unwrap_or_else(|| panic!("{name}"));
        assert!(hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()), "{name}");
        assert_eq!(std::fs::metadata(g.path()).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(g.path().is_absolute());
    }
    assert_ne!(read.path(), full.path());
    let reader = Client::from_token_file(read.path(), port).unwrap();
    let writer = Client::from_token_file(full.path(), port).unwrap();
    assert_eq!(reader.call("m", json!({})).unwrap(), json!({ "role": "ReadOnly", "live": true }));
    assert_eq!(writer.call("m", json!({})).unwrap(), json!({ "role": "Full", "live": true }));
    // A connection holding the read-only token: revoking it answers REVOKED and closes it, and a
    // request waiting in the handler sees its liveness go false.
    let token = read_token(read.path()).unwrap();
    let (mut s, mut r, first) = held(port, &format!("{{\"id\":1,\"method\":\"m\",\"token\":\"{token}\"}}\n"));
    assert_eq!(first["result"]["role"], "ReadOnly");
    s.write_all(b"{\"id\":2,\"method\":\"wait\"}\n").unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let path = read.path().to_path_buf();
    drop(read);
    let waited: Value = serde_json::from_slice(&read_line(&mut r, MAX_REPLY).unwrap().unwrap()).unwrap();
    assert_eq!((waited["id"].clone(), waited["result"]["live"].clone()), (json!(2), json!(false)), "{waited}");
    let last: Value = serde_json::from_slice(&read_line(&mut r, MAX_REPLY).unwrap().unwrap()).unwrap();
    assert_eq!(last["error"], REVOKED, "{last}");
    assert_eq!(read_line(&mut r, MAX_REPLY).unwrap(), None, "then closed");
    assert!(!path.exists(), "its file is gone");
    assert!(matches!(Client::with_token(port, token).call("m", json!({})), Err(CallError::Unreachable(_))), "the revoked token opens nothing");
    // The other grant goes on.
    assert_eq!(server.grants(), 1);
    assert!(writer.call("m", json!({})).is_ok());
    drop(full);
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Closing an opening revokes its grants, answers a held connection "revoked", removes the files and stops listening, within a bounded time.
#[cfg(unix)]
#[test]
fn closing_a_server_revokes_every_grant_answers_held_connections_and_stops_listening() {
    let dir = folder("close");
    let server = Server::open(&dir, roles()).unwrap();
    let port = server.port();
    let grant = server.grant(Role::Full).unwrap();
    let path = grant.path().to_path_buf();
    let token = read_token(&path).unwrap();
    let (_s, mut r, first) = held(port, &format!("{{\"id\":1,\"method\":\"m\",\"token\":\"{token}\"}}\n"));
    assert_eq!(first["ok"], true);
    // The grant outlives nothing: the close revokes it.
    assert!(grant.live());
    let started = Instant::now();
    drop(server);
    assert!(!grant.live(), "a grant of a closed opening opens nothing");
    assert!(started.elapsed() < Duration::from_secs(3), "the close is bounded: {:?}", started.elapsed());
    let last: Value = serde_json::from_slice(&read_line(&mut r, MAX_REPLY).unwrap().unwrap()).unwrap();
    assert_eq!(last["error"], REVOKED, "{last}");
    assert!(!path.exists(), "the file went with the close");
    assert!(!listens(port), "nothing listens after the close");
    drop(grant);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A token file is never created over another, and removed only while its name holds the very file written.
#[cfg(unix)]
#[test]
fn a_token_file_is_never_made_over_another_and_removed_only_while_it_is_the_same_file() {
    let dir = folder("identity");
    let path = dir.join("control-1-0000000000000000.token");
    std::fs::write(&path, "someone else's").unwrap();
    assert!(matches!(create_token_file(&path, &mint_token().unwrap()), Err(Created::Taken)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "someone else's", "never replaced");
    let mine = dir.join("control-1-1111111111111111.token");
    let id = create_token_file(&mine, &mint_token().unwrap()).ok().unwrap();
    // Replaced by another file under the same name: left alone.
    std::fs::remove_file(&mine).unwrap();
    std::fs::write(&mine, "another").unwrap();
    remove_token_file(&mine, id);
    assert!(mine.exists(), "a file that is not the one written is left");
    // The very file: removed.
    std::fs::remove_file(&mine).unwrap();
    let id = create_token_file(&mine, &mint_token().unwrap()).ok().unwrap();
    remove_token_file(&mine, id);
    assert!(!mine.exists());
    let _ = std::fs::remove_dir_all(&dir);
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

/// The packaged layout: the application's executable and the command-line tool in one folder.
fn layout(name: &str, with_cli: bool) -> (PathBuf, PathBuf) {
    let dir = folder(name);
    let exe = dir.join(if cfg!(windows) { "archforge.exe" } else { "archforge" });
    std::fs::write(&exe, b"app").unwrap();
    if with_cli {
        std::fs::write(dir.join(CLI_FILE), b"cli").unwrap();
    }
    (dir, exe)
}

#[test]
fn the_command_line_tool_is_found_beside_the_executable_before_any_development_folder() {
    let (dir, exe) = layout("cli-beside", true);
    let (dev, _) = layout("cli-beside-dev", true);
    // The executable's own folder, links followed (the temporary folder may be one).
    let beside = std::fs::canonicalize(&dir).unwrap().join(CLI_FILE);
    assert_eq!(find_cli(&exe, std::slice::from_ref(&dev)), Ok(beside));
    // A development folder is looked in only when nothing lies beside the executable.
    let (_, lone) = layout("cli-beside-lone", false);
    assert_eq!(find_cli(&lone, std::slice::from_ref(&dev)), Ok(dev.join(CLI_FILE)));
}

#[test]
fn a_missing_command_line_tool_is_a_typed_error_naming_every_place_looked_at() {
    let (dir, exe) = layout("cli-missing", false);
    let dev = folder("cli-missing-dev");
    let Err(CliError::NotFound { looked }) = find_cli(&exe, std::slice::from_ref(&dev)) else {
        panic!("a missing archforge-cli must be NotFound");
    };
    let real = std::fs::canonicalize(&dir).unwrap().join(CLI_FILE);
    let mut expected = vec![real.clone()];
    if dir.join(CLI_FILE) != real {
        expected.push(dir.join(CLI_FILE));
    }
    expected.push(dev.join(CLI_FILE));
    assert_eq!(looked, expected);
    let message = CliError::NotFound { looked }.to_string();
    assert!(message.contains(&dir.join(CLI_FILE).display().to_string()), "{message}");
    // A folder named like the tool is not the tool.
    std::fs::create_dir(dir.join(CLI_FILE)).unwrap();
    assert!(matches!(find_cli(&exe, &[]), Err(CliError::NotFound { .. })));
}

#[test]
fn only_a_debug_build_looks_in_the_cargo_target_folder() {
    assert!(dev_cli_dirs(false).is_empty());
    let dirs = dev_cli_dirs(true);
    assert!(dirs.last().is_some_and(|d| d.ends_with("target/debug")), "{dirs:?}");
}

#[cfg(unix)]
#[test]
fn a_linked_executable_finds_the_command_line_tool_beside_the_file_it_links_to() {
    let (dir, exe) = layout("cli-linked", true);
    let elsewhere = folder("cli-linked-bin");
    let link = elsewhere.join("archforge");
    std::os::unix::fs::symlink(&exe, &link).unwrap();
    // Another install's tool beside the link is not this application's.
    std::fs::write(elsewhere.join(CLI_FILE), b"another install").unwrap();
    let found = find_cli(&link, &[]).unwrap();
    assert_eq!(found, std::fs::canonicalize(&dir).unwrap().join(CLI_FILE));
}

/// A server's listener watch says closed only once its listening socket is closed (AR-229): open
/// while it serves, closed once it is dropped, whatever binds the freed port since; and never
/// closed for a server left running, so a check by it fails on a listener really left open.
#[test]
fn a_listener_watch_says_closed_only_once_the_socket_is_closed() {
    let (server, _dir) = echo("listener-watch");
    let port = server.port();
    let watch = server.listener_watch();
    assert!(!watch.closed(), "open while it serves");
    drop(server);
    assert!(watch.closed(), "closed once dropped");
    // Another socket on the freed port changes nothing.
    let squatter = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
    assert!(watch.closed(), "still closed with the port taken again: {:?}", squatter.as_ref().map(|_| ()));
    // A server never closed: its watch never says closed.
    let (left, _dir) = echo("listener-watch-left");
    let left_watch = left.listener_watch();
    std::mem::forget(left);
    assert!(!left_watch.closed(), "a listener left open is never said closed");
}
