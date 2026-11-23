//! STP-39: `stapel lsp`, the language server on standard input and output.

mod common;

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// A running `stapel lsp` with the framed JSON-RPC helpers of a client.
struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl Client {
    fn start() -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_stapel"))
            .arg("lsp")
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Client {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    fn send(&mut self, msg: &Value) {
        let body = msg.to_string();
        let stdin = self.stdin.as_mut().unwrap();
        write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        stdin.flush().unwrap();
    }

    /// Reads one framed message; panics at the end of the output.
    fn read(&mut self) -> Value {
        let mut len = None;
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).unwrap();
            assert!(n > 0, "the server closed its output");
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length:") {
                len = Some(v.trim().parse::<usize>().unwrap());
            }
        }
        let mut buf = vec![0u8; len.expect("no Content-Length header")];
        self.stdout.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Sends a request and returns its response, skipping notifications of the server.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let msg = self.read();
            if msg.get("id") == Some(&json!(id)) && msg.get("method").is_none() {
                return msg;
            }
        }
    }

    fn initialize(&mut self) -> Value {
        let resp = self.request(
            "initialize",
            json!({"processId": null, "rootUri": null, "capabilities": {}}),
        );
        self.notify("initialized", json!({}));
        resp
    }

    fn close_stdin(&mut self) {
        self.stdin = None;
    }

    fn exit_code(&mut self) -> Option<i32> {
        self.child.wait().unwrap().code()
    }
}

// AC-1: capabilities, MethodNotFound, InvalidRequest after shutdown, exit 0.
#[test]
fn initialize_and_shutdown() {
    let mut c = Client::start();
    let resp = c.initialize();
    let caps = &resp["result"]["capabilities"];
    let sync = &caps["textDocumentSync"];
    assert!(
        sync == &json!(1) || sync["change"] == json!(1),
        "full text sync expected, got {sync}"
    );
    assert!(
        caps["codeActionProvider"].as_bool() == Some(true)
            || caps["codeActionProvider"].is_object(),
        "no code action provider: {caps}"
    );
    let mut commands: Vec<String> = caps["executeCommandProvider"]["commands"]
        .as_array()
        .expect("no execute command provider")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    commands.sort();
    assert_eq!(
        commands,
        ["stapel.next", "stapel.ok", "stapel.page", "stapel.status"]
    );

    let hover = c.request(
        "textDocument/hover",
        json!({
            "textDocument": {"uri": "file:///x/README.md"},
            "position": {"line": 0, "character": 0}
        }),
    );
    assert_eq!(hover["error"]["code"], json!(-32601), "{hover}");

    let shut = c.request("shutdown", Value::Null);
    assert!(shut.get("error").is_none(), "{shut}");
    assert_eq!(shut["result"], Value::Null);

    let late = c.request(
        "textDocument/hover",
        json!({
            "textDocument": {"uri": "file:///x/README.md"},
            "position": {"line": 0, "character": 0}
        }),
    );
    assert_eq!(late["error"]["code"], json!(-32600), "{late}");

    c.notify("exit", Value::Null);
    assert_eq!(c.exit_code(), Some(0));
}

// AC-1: `exit` without `shutdown`, and the end of standard input, exit 1.
#[test]
fn exit_without_shutdown() {
    let mut c = Client::start();
    c.initialize();
    c.notify("exit", Value::Null);
    assert_eq!(c.exit_code(), Some(1));

    let mut c = Client::start();
    c.initialize();
    c.close_stdin();
    assert_eq!(c.exit_code(), Some(1));
}

impl Client {
    /// Like `request`, and returns the notifications that came before the response.
    fn request_with_notes(&mut self, method: &str, params: Value) -> (Value, Vec<Value>) {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut notes = Vec::new();
        loop {
            let msg = self.read();
            if msg.get("id") == Some(&json!(id)) && msg.get("method").is_none() {
                return (msg, notes);
            }
            notes.push(msg);
        }
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "markdown", "version": 1, "text": text}}),
        );
    }

    fn code_action(&mut self, uri: &str, line: u32) -> (Value, Vec<Value>) {
        self.request_with_notes(
            "textDocument/codeAction",
            json!({
                "textDocument": {"uri": uri},
                "range": {"start": {"line": line, "character": 0}, "end": {"line": line, "character": 0}},
                "context": {"diagnostics": []}
            }),
        )
    }
}

/// A repository folder with `stapel.toml` and a ticket folder with `state.json`.
fn repo_with(root: &std::path::Path, ticket: &str) {
    let tdir = root.join(".stapel/tickets").join(ticket);
    std::fs::create_dir_all(&tdir).unwrap();
    std::fs::write(
        root.join(".stapel/stapel.toml"),
        stapel_core::config::default_toml("STP"),
    )
    .unwrap();
    std::fs::write(tdir.join("state.json"), "{}").unwrap();
    std::fs::write(tdir.join("ticket.md"), "# T\n\nline\nlast\n").unwrap();
    std::fs::write(tdir.join("page.md"), "page\n").unwrap();
}

fn file_uri(path: &std::path::Path) -> String {
    format!("file://{}", path.to_str().unwrap().replace(' ', "%20"))
}

fn titles(resp: &Value) -> Vec<String> {
    assert!(resp.get("error").is_none(), "{resp}");
    resp["result"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| x["title"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

// AC-2: the three buttons, in order, at any range; the argument is the URI.
#[test]
fn buttons_on_ticket_documents() {
    let dir = tempfile::tempdir().unwrap();
    repo_with(dir.path(), "STP-1");
    let mut c = Client::start();
    c.initialize();
    for doc in ["ticket.md", "page.md"] {
        let path = dir.path().join(".stapel/tickets/STP-1").join(doc);
        let uri = file_uri(&path);
        c.open(&uri, &std::fs::read_to_string(&path).unwrap());
        for line in [0, 3] {
            let (resp, _) = c.code_action(&uri, line);
            let got = titles(&resp);
            assert!(got.len() >= 3, "{doc} line {line}: {resp}");
            assert_eq!(
                got[..3],
                ["stapel status STP-1", "stapel next", "stapel page STP-1"]
            );
            let first = &resp["result"][0];
            assert!(first.get("kind").is_none_or(|k| k.is_null()), "{first}");
            assert_eq!(first["command"]["command"], json!("stapel.status"));
            assert_eq!(first["command"]["arguments"], json!([uri]));
            assert_eq!(resp["result"][1]["command"]["command"], json!("stapel.next"));
            assert_eq!(resp["result"][2]["command"]["command"], json!("stapel.page"));
            assert_eq!(resp["result"][2]["command"]["arguments"], json!([uri]));
        }
    }
}

// AC-3: no action and no diagnostic for a document that is not an open ticket document.
#[test]
fn nothing_for_other_documents() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("my root");
    repo_with(&root, "STP-1");
    repo_with(&root, "-x");
    repo_with(&root, "stp-039");
    std::fs::write(root.join("README.md"), "x\n").unwrap();
    std::fs::write(root.join("ticket.md"), "x\n").unwrap();
    std::fs::create_dir_all(root.join(".stapel/tickets/STP-2")).unwrap();
    std::fs::write(root.join(".stapel/tickets/STP-2/ticket.md"), "x\n").unwrap();
    let bare = dir.path().join("bare");
    std::fs::create_dir_all(bare.join(".stapel/tickets/STP-1")).unwrap();
    std::fs::write(bare.join(".stapel/tickets/STP-1/ticket.md"), "x\n").unwrap();
    std::fs::write(bare.join(".stapel/tickets/STP-1/state.json"), "{}").unwrap();
    let broken = dir.path().join("broken");
    repo_with(&broken, "STP-1");
    std::fs::write(broken.join(".stapel/stapel.toml"), "this is = [not valid").unwrap();
    let never = root.join(".stapel/tickets/STP-1/page.md");

    let mut c = Client::start();
    c.initialize();
    let t = |r: &std::path::Path, rel: &str| file_uri(&r.join(rel));
    let others = [
        t(&root, "README.md"),
        t(&root, "ticket.md"),
        t(&root, ".stapel/tickets/STP-2/ticket.md"),
        t(&bare, ".stapel/tickets/STP-1/ticket.md"),
        t(&root, ".stapel/tickets/-x/ticket.md"),
        t(&broken, ".stapel/tickets/STP-1/ticket.md"),
        "untitled:Untitled-1".to_string(),
    ];
    for uri in &others {
        c.open(uri, "# T\n\nline\n");
        let (resp, notes) = c.code_action(uri, 0);
        assert!(titles(&resp).is_empty(), "{uri}: {resp}");
        assert!(
            notes
                .iter()
                .all(|n| n["method"] != json!("textDocument/publishDiagnostics")),
            "{uri}: {notes:?}"
        );
    }
    // never opened
    let (resp, notes) = c.code_action(&file_uri(&never), 0);
    assert!(titles(&resp).is_empty(), "{resp}");
    assert!(notes.is_empty(), "{notes:?}");

    // a folder `stp-039` is a ticket document, and a root with a space is one
    let uri = t(&root, ".stapel/tickets/stp-039/ticket.md");
    c.open(&uri, "# T\n\nline\n");
    let (resp, _) = c.code_action(&uri, 0);
    assert_eq!(
        titles(&resp)[..3],
        ["stapel status stp-039", "stapel next", "stapel page stp-039"]
    );
}

impl Client {
    /// Runs a button command and returns its response and the notifications before it.
    fn execute(&mut self, command: &str, args: Value) -> (Value, Vec<Value>) {
        self.request_with_notes(
            "workspace/executeCommand",
            json!({"command": command, "arguments": args}),
        )
    }
}

/// The `window/showMessage` notifications as (type, message).
fn shown(notes: &[Value]) -> Vec<(i64, String)> {
    notes
        .iter()
        .filter(|n| n["method"] == json!("window/showMessage"))
        .map(|n| {
            (
                n["params"]["type"].as_i64().unwrap(),
                n["params"]["message"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// The text of the `window/logMessage` notifications, joined.
fn logged(notes: &[Value]) -> String {
    notes
        .iter()
        .filter(|n| n["method"] == json!("window/logMessage"))
        .map(|n| n["params"]["message"].as_str().unwrap().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

// AC-4: a button runs the executable in the root; Info message, log and `null`.
#[test]
fn runs_a_button() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    let tdir = dir.join(".stapel/tickets/ABC-1");
    let ticket = tdir.join("ticket.md");
    let uri = file_uri(&ticket);
    let mut c = Client::start();
    c.initialize();
    c.open(&uri, &std::fs::read_to_string(&ticket).unwrap());

    let (resp, notes) = c.execute("stapel.status", json!([uri]));
    assert_eq!(resp["result"], Value::Null, "{resp}");
    assert!(resp.get("error").is_none(), "{resp}");
    let msgs = shown(&notes);
    assert_eq!(msgs.len(), 1, "{notes:?}");
    assert_eq!(msgs[0].0, 3);
    assert!(
        msgs[0].1.starts_with("stapel status ABC-1: ticket: ABC-1"),
        "{msgs:?}"
    );
    assert!(logged(&notes).contains("ticket: ABC-1"), "{notes:?}");

    let (resp, notes) = c.execute("stapel.next", json!([uri]));
    assert_eq!(resp["result"], Value::Null, "{resp}");
    let msgs = shown(&notes);
    assert_eq!(msgs.len(), 1, "{notes:?}");
    assert_eq!(msgs[0].0, 3, "{msgs:?}");
    assert!(msgs[0].1.starts_with("stapel next: "), "{msgs:?}");

    assert!(!tdir.join("page.md").exists());
    let (resp, notes) = c.execute("stapel.page", json!([uri]));
    assert_eq!(resp["result"], Value::Null, "{resp}");
    let msgs = shown(&notes);
    assert_eq!(msgs.len(), 1, "{notes:?}");
    assert_eq!(msgs[0].0, 3, "{msgs:?}");
    assert!(msgs[0].1.starts_with("stapel page ABC-1: "), "{msgs:?}");
    assert!(tdir.join("page.md").is_file());
}

// AC-4: a failing run gives an Error message with the exit code and the first line.
#[test]
fn a_failing_button() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    let tdir = dir.join(".stapel/tickets/ABC-1");
    let ticket = tdir.join("ticket.md");
    let uri = file_uri(&ticket);
    std::fs::write(tdir.join("state.json"), "not json").unwrap();
    let mut c = Client::start();
    c.initialize();
    c.open(&uri, &std::fs::read_to_string(&ticket).unwrap());
    let (resp, notes) = c.execute("stapel.page", json!([uri]));
    assert_eq!(resp["result"], Value::Null, "{resp}");
    let msgs = shown(&notes);
    assert_eq!(msgs.len(), 1, "{notes:?}");
    assert_eq!(msgs[0].0, 1, "{msgs:?}");
    assert!(
        msgs[0].1.starts_with("stapel page ABC-1: exit 1: stapel: "),
        "{msgs:?}"
    );
    assert!(!logged(&notes).is_empty(), "{notes:?}");

    // a ticket that is not the branch's: the child's refusal
    let pair = common::two_ticket_worktrees();
    let b = &pair.b;
    std::fs::create_dir_all(b.join(".stapel/tickets/ABC-1")).unwrap();
    for file in ["ticket.md", "state.json"] {
        std::fs::copy(
            pair.a.join(".stapel/tickets/ABC-1").join(file),
            b.join(".stapel/tickets/ABC-1").join(file),
        )
        .unwrap();
    }
    let foreign = b.join(".stapel/tickets/ABC-1/ticket.md");
    let uri = file_uri(&foreign);
    c.open(&uri, &std::fs::read_to_string(&foreign).unwrap());
    let (resp, notes) = c.execute("stapel.page", json!([uri]));
    assert_eq!(resp["result"], Value::Null, "{resp}");
    let msgs = shown(&notes);
    assert_eq!(msgs.len(), 1, "{notes:?}");
    assert_eq!(msgs[0].0, 1, "{msgs:?}");
    assert!(
        msgs[0].1.starts_with("stapel page ABC-1: exit 2: "),
        "{msgs:?}"
    );
    assert!(
        msgs[0].1.contains("ABC-1 is not the ticket of branch ABC-2"),
        "{msgs:?}"
    );
}

// AC-5: a bad command name or bad arguments get InvalidParams and nothing runs.
#[test]
fn refuses_unknown_commands() {
    let dir = tempfile::tempdir().unwrap();
    repo_with(dir.path(), "STP-1");
    repo_with(dir.path(), "STP-2");
    std::fs::write(dir.path().join("README.md"), "x\n").unwrap();
    let tdir = dir.path().join(".stapel/tickets/STP-1");
    let ticket = file_uri(&tdir.join("ticket.md"));
    let page = file_uri(&tdir.join("page.md"));
    let other = file_uri(&dir.path().join(".stapel/tickets/STP-2/ticket.md"));
    let readme = file_uri(&dir.path().join("README.md"));
    let closed = file_uri(&dir.path().join(".stapel/tickets/STP-2/page.md"));
    let mut c = Client::start();
    c.initialize();
    c.open(&ticket, "# T\n");
    c.open(&page, "page\n");
    c.open(&other, "# T\n");
    c.open(&readme, "x\n");
    c.open(&closed, "page\n");
    c.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": closed}}),
    );

    let bad_args = "stapel lsp: the arguments must be the URI of an open ticket.md or page.md";
    let cases: Vec<(&str, Value, String)> = vec![
        (
            "stapel.close",
            json!([ticket]),
            "stapel lsp: unknown command stapel.close".to_string(),
        ),
        ("", json!([ticket]), "stapel lsp: unknown command ".to_string()),
        ("stapel.status", json!([]), bad_args.to_string()),
        ("stapel.status", json!([ticket, ticket]), bad_args.to_string()),
        ("stapel.status", json!([5]), bad_args.to_string()),
        ("stapel.status", json!([readme]), bad_args.to_string()),
        ("stapel.status", json!([other, "STP-2"]), bad_args.to_string()),
        ("stapel.status", json!([closed]), bad_args.to_string()),
        ("stapel.ok", json!([page]), bad_args.to_string()),
        ("stapel.ok", json!([ticket, "0123456789abcdef"]), bad_args.to_string()),
    ];
    for (command, args, message) in cases {
        let (resp, notes) = c.execute(command, args.clone());
        assert_eq!(resp["error"]["code"], json!(-32602), "{command} {args}: {resp}");
        assert_eq!(resp["error"]["message"], json!(message), "{command} {args}: {resp}");
        assert!(shown(&notes).is_empty(), "{command} {args}: {notes:?}");
        assert!(logged(&notes).is_empty(), "{command} {args}: {notes:?}");
    }
}

// AC-6: in an agent shell no valid run goes ahead; an invalid one is still refused by AC-5.
#[test]
fn refuses_in_an_agent_shell() {
    let repo = common::repo_with_ticket();
    let tdir = repo.path().join(".stapel/tickets/ABC-1");
    let ticket = tdir.join("ticket.md");
    let page = tdir.join("page.md");
    std::fs::write(&page, "page\n").unwrap();
    let state = std::fs::read(tdir.join("state.json")).unwrap();
    let ticket_uri = file_uri(&ticket);
    let page_uri = file_uri(&page);
    let message = "stapel lsp: this editor was started from an agent's shell (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); start it from your own shell to use the buttons";
    for (name, value) in [("CLAUDECODE", "1"), ("CLAUDE_CODE_ENTRYPOINT", "cli")] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_stapel"))
            .arg("lsp")
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .env(name, value)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        c.initialize();
        c.open(&ticket_uri, &std::fs::read_to_string(&ticket).unwrap());
        c.open(&page_uri, "page\n");

        let runs = [
            ("stapel.status", json!([ticket_uri])),
            ("stapel.next", json!([ticket_uri])),
            ("stapel.page", json!([page_uri])),
            ("stapel.ok", json!([page_uri, "0123456789abcdef"])),
        ];
        for (command, args) in runs {
            let (resp, notes) = c.execute(command, args);
            assert_eq!(resp["result"], Value::Null, "{name} {command}: {resp}");
            assert!(resp.get("error").is_none(), "{name} {command}: {resp}");
            assert_eq!(
                shown(&notes),
                vec![(1, message.to_string())],
                "{name} {command}: {notes:?}"
            );
            assert!(logged(&notes).is_empty(), "{name} {command}: {notes:?}");
        }
        assert_eq!(std::fs::read(tdir.join("state.json")).unwrap(), state);
        assert_eq!(std::fs::read_to_string(&page).unwrap(), "page\n");

        let (resp, notes) = c.execute("stapel.close", json!([ticket_uri]));
        assert_eq!(resp["error"]["code"], json!(-32602), "{name}: {resp}");
        assert!(shown(&notes).is_empty(), "{name}: {notes:?}");
        let _ = c.child.kill();
    }
}
