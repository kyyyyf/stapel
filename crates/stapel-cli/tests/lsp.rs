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

// AC-7: on a current page the `ok` button is last, with the hash of `page.md`; its run confirms
// every waiting section as a terminal run does.
#[test]
fn ok_on_a_current_page() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    common::stapel(dir).args(["page", "ABC-1"]).assert().success();
    let page = dir.join(".stapel/tickets/ABC-1/page.md");
    let bytes = std::fs::read(&page).unwrap();
    let hash = stapel_core::page::page_hash(&bytes);
    let uri = file_uri(&page);
    let mut c = Client::start();
    c.initialize();
    c.open(&uri, &String::from_utf8(bytes).unwrap());

    for line in [0, 2] {
        let (resp, _) = c.code_action(&uri, line);
        assert_eq!(
            titles(&resp),
            [
                "stapel status ABC-1",
                "stapel next",
                "stapel page ABC-1",
                "stapel ok ABC-1"
            ]
        );
        let last = &resp["result"][3];
        assert_eq!(last["command"]["command"], json!("stapel.ok"));
        assert_eq!(last["command"]["arguments"], json!([uri, hash]));
    }

    let (resp, notes) = c.execute("stapel.ok", json!([uri, hash]));
    assert_eq!(resp["result"], Value::Null, "{resp}");
    assert!(resp.get("error").is_none(), "{resp}");
    let msgs = shown(&notes);
    assert_eq!(msgs.len(), 1, "{notes:?}");
    assert_eq!(msgs[0].0, 3, "{msgs:?}");
    assert!(msgs[0].1.starts_with("stapel ok ABC-1: "), "{msgs:?}");
    let state = common::state_json(dir, "ABC-1");
    let confirmations = state["confirmations"].as_array().unwrap();
    let mut sections: Vec<&str> = confirmations
        .iter()
        .map(|c| c["section"].as_str().unwrap())
        .collect();
    sections.sort();
    assert_eq!(sections, ["design", "plan", "proof", "spec"]);
    assert!(confirmations.iter().all(|c| c["via"] == json!("terminal")));
}

// AC-8: no `ok` button on a page that is not current, nor on `ticket.md`.
#[test]
fn no_ok_on_a_stale_page() {
    fn has_ok(c: &mut Client, uri: &str) -> bool {
        let (resp, _) = c.code_action(uri, 0);
        let got = titles(&resp);
        assert!(got.len() >= 3, "{resp}");
        got.iter().any(|t| t.starts_with("stapel ok"))
    }
    fn fresh_page() -> (tempfile::TempDir, std::path::PathBuf) {
        let repo = common::repo_with_ticket();
        common::stapel(repo.path())
            .args(["page", "ABC-1"])
            .assert()
            .success();
        let page = repo.path().join(".stapel/tickets/ABC-1/page.md");
        (repo, page)
    }
    let mut c = Client::start();
    c.initialize();

    // control: the page as written is current
    let (_repo, page) = fresh_page();
    let uri = file_uri(&page);
    c.open(&uri, &std::fs::read_to_string(&page).unwrap());
    assert!(has_ok(&mut c, &uri));

    // one byte changed on disk, the buffer equal to the disk
    let (_repo, page) = fresh_page();
    let mut text = std::fs::read_to_string(&page).unwrap();
    text.push('x');
    std::fs::write(&page, &text).unwrap();
    let uri = file_uri(&page);
    c.open(&uri, &text);
    assert!(!has_ok(&mut c, &uri), "one byte changed");

    // written before a later edit of the Spec
    let (repo, page) = fresh_page();
    common::set_section(repo.path(), "ABC-1", "Spec", "The spec, revised.");
    let uri = file_uri(&page);
    c.open(&uri, &std::fs::read_to_string(&page).unwrap());
    assert!(!has_ok(&mut c, &uri), "older than the ticket text");

    // buffer changed, not saved
    let (_repo, page) = fresh_page();
    let mut text = std::fs::read_to_string(&page).unwrap();
    text.push('x');
    let uri = file_uri(&page);
    c.open(&uri, &text);
    assert!(!has_ok(&mut c, &uri), "unsaved buffer");

    // no page.md
    let (_repo, page) = fresh_page();
    let text = std::fs::read_to_string(&page).unwrap();
    std::fs::remove_file(&page).unwrap();
    let uri = file_uri(&page);
    c.open(&uri, &text);
    assert!(!has_ok(&mut c, &uri), "no page.md");

    // no waiting section
    let repo = common::confirmed_repo_with_ticket();
    let page = repo.path().join(".stapel/tickets/ABC-1/page.md");
    std::fs::write(&page, "nothing to confirm\n").unwrap();
    let uri = file_uri(&page);
    c.open(&uri, "nothing to confirm\n");
    assert!(!has_ok(&mut c, &uri), "no waiting section");

    // a page of a ticket that is not the branch's
    let pair = common::two_ticket_worktrees();
    common::stapel(&pair.b).args(["page", "ABC-2"]).assert().success();
    let from = pair.b.join(".stapel/tickets/ABC-2");
    let to = pair.a.join(".stapel/tickets/ABC-2");
    std::fs::create_dir_all(&to).unwrap();
    for name in ["ticket.md", "state.json", "page.md"] {
        std::fs::copy(from.join(name), to.join(name)).unwrap();
    }
    let uri = file_uri(&to.join("page.md"));
    c.open(&uri, &std::fs::read_to_string(to.join("page.md")).unwrap());
    assert!(!has_ok(&mut c, &uri), "another ticket's page");

    // ticket.md of a ticket whose page is current
    let (_repo, page) = fresh_page();
    let ticket = page.with_file_name("ticket.md");
    let uri = file_uri(&ticket);
    c.open(&uri, &std::fs::read_to_string(&ticket).unwrap());
    assert!(!has_ok(&mut c, &uri), "ticket.md");
}

// AC-2: a non-empty `only` filter gets no button, on a current page.md and on ticket.md.
#[test]
fn no_buttons_under_a_kind_filter() {
    let repo = common::repo_with_ticket();
    common::stapel(repo.path())
        .args(["page", "ABC-1"])
        .assert()
        .success();
    let mut c = Client::start();
    c.initialize();
    for doc in ["page.md", "ticket.md"] {
        let path = repo.path().join(".stapel/tickets/ABC-1").join(doc);
        let uri = file_uri(&path);
        c.open(&uri, &std::fs::read_to_string(&path).unwrap());
        let (resp, _) = c.code_action(&uri, 0);
        assert!(titles(&resp).len() >= 3, "{doc} without a filter: {resp}");
        for only in [json!(["source.fixAll"]), json!(["quickfix"])] {
            let (resp, _) = c.request_with_notes(
                "textDocument/codeAction",
                json!({
                    "textDocument": {"uri": uri},
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                    "context": {"diagnostics": [], "only": only}
                }),
            );
            assert!(titles(&resp).is_empty(), "{doc} under {only}: {resp}");
        }
    }
}

// AC-9: a run on a page that changed after the offer, or with another hash, runs nothing.
#[test]
fn ok_refused_after_the_page_changed() {
    let message = "stapel lsp: page.md is not current; run stapel page ABC-1 and read it again";
    let mut c = Client::start();
    c.initialize();
    let fresh = || {
        let repo = common::repo_with_ticket();
        common::stapel(repo.path())
            .args(["page", "ABC-1"])
            .assert()
            .success();
        repo
    };
    let check = |c: &mut Client, uri: &str, hash: &str, tdir: &std::path::Path, what: &str| {
        let state = std::fs::read(tdir.join("state.json")).unwrap();
        let (resp, notes) = c.execute("stapel.ok", json!([uri, hash]));
        assert_eq!(resp["result"], Value::Null, "{what}: {resp}");
        assert!(resp.get("error").is_none(), "{what}: {resp}");
        assert_eq!(shown(&notes), vec![(1, message.to_string())], "{what}: {notes:?}");
        assert!(logged(&notes).is_empty(), "{what}: {notes:?}");
        assert_eq!(std::fs::read(tdir.join("state.json")).unwrap(), state, "{what}");
    };

    // offered, then the Spec changed, then the run
    let repo = fresh();
    let tdir = repo.path().join(".stapel/tickets/ABC-1");
    let page = tdir.join("page.md");
    let bytes = std::fs::read(&page).unwrap();
    let hash = stapel_core::page::page_hash(&bytes);
    let uri = file_uri(&page);
    c.open(&uri, &String::from_utf8(bytes).unwrap());
    common::set_section(repo.path(), "ABC-1", "Spec", "The spec, revised.");
    check(&mut c, &uri, &hash, &tdir, "spec changed");

    // the page regenerated after the offer
    common::stapel(repo.path()).args(["page", "ABC-1"]).assert().success();
    check(&mut c, &uri, &hash, &tdir, "page regenerated");

    // a made-up hash on a current page
    let repo = fresh();
    let tdir = repo.path().join(".stapel/tickets/ABC-1");
    let page = tdir.join("page.md");
    let uri = file_uri(&page);
    c.open(&uri, &std::fs::read_to_string(&page).unwrap());
    check(&mut c, &uri, "0123456789abcdef", &tdir, "made-up hash");
}

impl Client {
    fn change(&mut self, uri: &str, version: i64, text: &str) {
        self.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": text}]}),
        );
    }
}

/// The diagnostic lists published for `uri` in `notes`, in order.
fn published(notes: &[Value], uri: &str) -> Vec<Vec<Value>> {
    notes
        .iter()
        .filter(|n| n["method"] == json!("textDocument/publishDiagnostics"))
        .filter(|n| n["params"]["uri"] == json!(uri))
        .map(|n| n["params"]["diagnostics"].as_array().unwrap().clone())
        .collect()
}

/// The (line, message) pairs of the diagnostics, sorted; the fixed fields are checked on the way.
fn waiting_lines(list: &[Value], text: &str) -> Vec<(u64, String)> {
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let mut out: Vec<(u64, String)> = list
        .iter()
        // the Hints of changed lines (STP-39 step 10) are checked in `changed_lines`
        .filter(|d| d["severity"] != json!(4))
        .map(|d| {
            assert_eq!(d["severity"], json!(3), "{d}");
            assert_eq!(d["source"], json!("stapel"), "{d}");
            let line = d["range"]["start"]["line"].as_u64().unwrap();
            assert_eq!(d["range"]["start"]["character"], json!(0), "{d}");
            assert_eq!(d["range"]["end"]["line"], json!(line), "{d}");
            assert_eq!(
                d["range"]["end"]["character"],
                json!(lines[line as usize].encode_utf16().count()),
                "{d}"
            );
            (line, d["message"].as_str().unwrap().to_string())
        })
        .collect();
    out.sort();
    out
}

/// The expected (line, message) pairs for the headings in `text` named by `titles`.
fn expected_waiting(text: &str, titles: &[(&str, &str)]) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        let line = line.trim_end_matches('\r');
        for (title, owner) in titles {
            if line == format!("## {title}") {
                out.push((i as u64, format!("waiting for confirmation (owner: {owner})")));
            }
        }
    }
    out.sort();
    out
}

// AC-10: an Information diagnostic on the heading of each waiting section of a `ticket.md`.
#[test]
fn waiting_headings() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    common::confirm_these(dir, "ABC-1", &["spec"]);
    let tdir = dir.join(".stapel/tickets/ABC-1");
    let ticket = tdir.join("ticket.md");
    let uri = file_uri(&ticket);
    let all = [
        ("Spec", "product"),
        ("Design", "engineer"),
        ("Proof", "qa"),
        ("Plan", "engineer"),
    ];
    let text = std::fs::read_to_string(&ticket).unwrap();
    let mut c = Client::start();
    c.initialize();

    // a confirmed Spec has none; the others wait
    c.open(&uri, &text);
    let (_, notes) = c.code_action(&uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    assert_eq!(
        waiting_lines(&got[0], &text),
        expected_waiting(&text, &all[1..])
    );

    // the buffer edits the confirmed Spec: its diagnostic comes back
    let edited = text.replace("The spec.", "The spec, edited.");
    assert_ne!(edited, text);
    c.change(&uri, 2, &edited);
    let (_, notes) = c.code_action(&uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    let lf_lines = waiting_lines(&got[0], &edited);
    assert_eq!(lf_lines, expected_waiting(&edited, &all));

    // the same buffer with CRLF line ends: the same lines
    let crlf = edited.replace('\n', "\r\n");
    c.change(&uri, 3, &crlf);
    let (_, notes) = c.code_action(&uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    assert_eq!(waiting_lines(&got[0], &crlf), lf_lines);

    // a duplicated Spec heading: one on each
    let dup = format!("{text}\n## Spec\n\nmore\n");
    c.change(&uri, 4, &dup);
    let (_, notes) = c.code_action(&uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    let lines = waiting_lines(&got[0], &dup);
    assert_eq!(lines, expected_waiting(&dup, &all));
    assert_eq!(lines.iter().filter(|(_, m)| m.contains("product")).count(), 2);

    // the Design heading removed: none for Design (its text now lies in the Spec, which changed)
    let no_design = text.replace("## Design\n", "");
    assert_ne!(no_design, text);
    c.change(&uri, 5, &no_design);
    let (_, notes) = c.code_action(&uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    assert_eq!(
        waiting_lines(&got[0], &no_design),
        expected_waiting(
            &no_design,
            &[("Spec", "product"), ("Proof", "qa"), ("Plan", "engineer")]
        )
    );

    // a page.md gets none
    let page_uri = file_uri(&tdir.join("page.md"));
    c.open(&page_uri, "page\n");
    let (_, notes) = c.code_action(&page_uri, 0);
    assert!(published(&notes, &page_uri).is_empty(), "{notes:?}");

    // close: an empty list
    c.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": uri}}),
    );
    let (_, notes) = c.code_action(&page_uri, 0);
    assert_eq!(published(&notes, &uri), vec![Vec::<Value>::new()], "{notes:?}");
}

// AC-10: an empty list once when the state goes, the logged errors forgotten, an empty list on close.
#[test]
fn diagnostics_cleared_when_the_state_goes() {
    let repo = common::repo_with_ticket();
    let tdir = repo.path().join(".stapel/tickets/ABC-1");
    let ticket = tdir.join("ticket.md");
    let state = tdir.join("state.json");
    let uri = file_uri(&ticket);
    let text = std::fs::read_to_string(&ticket).unwrap();
    let good = std::fs::read_to_string(&state).unwrap();
    let errors = |notes: &[Value]| -> usize {
        notes
            .iter()
            .filter(|n| n["method"] == json!("window/logMessage"))
            .filter(|n| n["params"]["type"] == json!(1))
            .count()
    };
    let mut c = Client::start();
    c.initialize();
    c.open(&uri, &text);
    let (_, notes) = c.code_action(&uri, 0);
    assert!(!published(&notes, &uri)[0].is_empty(), "{notes:?}");
    let mut version = 1;
    let mut edit = |c: &mut Client| {
        version += 1;
        c.change(&uri, version, &format!("{text}\nmore {version}\n"));
        c.code_action(&uri, 0).1
    };

    // state.json deleted: an empty list once
    std::fs::remove_file(&state).unwrap();
    let notes = edit(&mut c);
    assert_eq!(published(&notes, &uri), vec![Vec::<Value>::new()], "{notes:?}");
    let notes = edit(&mut c);
    assert!(published(&notes, &uri).is_empty(), "{notes:?}");

    // restored, then a state without schema_version
    std::fs::write(&state, &good).unwrap();
    let notes = edit(&mut c);
    assert!(!published(&notes, &uri)[0].is_empty(), "{notes:?}");
    std::fs::write(&state, "{}").unwrap();
    let notes = edit(&mut c);
    assert_eq!(published(&notes, &uri), vec![Vec::<Value>::new()], "{notes:?}");
    let notes = edit(&mut c);
    assert!(published(&notes, &uri).is_empty(), "{notes:?}");

    // a bad file twice: an empty list once each time, and the error logged again
    for round in 0..2 {
        std::fs::write(&state, &good).unwrap();
        let notes = edit(&mut c);
        assert!(!published(&notes, &uri)[0].is_empty(), "{round}: {notes:?}");
        std::fs::write(&state, "{ not json").unwrap();
        let notes = edit(&mut c);
        assert_eq!(published(&notes, &uri), vec![Vec::<Value>::new()], "{round}: {notes:?}");
        assert_eq!(errors(&notes), 1, "{round}: {notes:?}");
    }

    // deleted again, then closed: an empty list
    std::fs::write(&state, &good).unwrap();
    let notes = edit(&mut c);
    assert!(!published(&notes, &uri)[0].is_empty(), "{notes:?}");
    std::fs::remove_file(&state).unwrap();
    c.notify("textDocument/didClose", json!({"textDocument": {"uri": uri}}));
    let (_, notes) = c.code_action(&uri, 0);
    assert_eq!(published(&notes, &uri), vec![Vec::<Value>::new()], "{notes:?}");
}

// AC-10: `didSave` publishes again; after `stapel ok` from a terminal the waiting diagnostics are gone.
#[test]
fn save_refreshes_diagnostics() {
    let repo = common::repo_with_ticket();
    let tdir = repo.path().join(".stapel/tickets/ABC-1");
    let ticket = tdir.join("ticket.md");
    let uri = file_uri(&ticket);
    let text = std::fs::read_to_string(&ticket).unwrap();
    let mut c = Client::start();
    c.initialize();
    c.open(&uri, &text);
    let (_, notes) = c.code_action(&uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    assert!(!waiting_lines(&got[0], &text).is_empty(), "{notes:?}");

    common::confirm_sections(repo.path(), "ABC-1");
    c.notify(
        "textDocument/didSave",
        json!({"textDocument": {"uri": uri}}),
    );
    let (_, notes) = c.code_action(&uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    assert!(waiting_lines(&got[0], &text).is_empty(), "{notes:?}");
}

// AC-9: a page regenerated after the offer and sent as `didChange` is refused by the hash.
#[test]
fn ok_refused_after_a_reloaded_page() {
    let message = "stapel lsp: page.md is not current; run stapel page ABC-1 and read it again";
    let repo = common::repo_with_ticket();
    common::stapel(repo.path())
        .args(["page", "ABC-1"])
        .assert()
        .success();
    let tdir = repo.path().join(".stapel/tickets/ABC-1");
    let page = tdir.join("page.md");
    let bytes = std::fs::read(&page).unwrap();
    let hash = stapel_core::page::page_hash(&bytes);
    let uri = file_uri(&page);
    let mut c = Client::start();
    c.initialize();
    c.open(&uri, &String::from_utf8(bytes.clone()).unwrap());

    common::set_section(repo.path(), "ABC-1", "Spec", "The spec, revised.");
    common::stapel(repo.path())
        .args(["page", "ABC-1"])
        .assert()
        .success();
    let now = std::fs::read(&page).unwrap();
    assert_ne!(now, bytes);
    c.change(&uri, 2, &String::from_utf8(now).unwrap());

    let state = std::fs::read(tdir.join("state.json")).unwrap();
    let (resp, notes) = c.execute("stapel.ok", json!([uri, hash]));
    assert_eq!(resp["result"], Value::Null, "{resp}");
    assert!(resp.get("error").is_none(), "{resp}");
    assert_eq!(shown(&notes), vec![(1, message.to_string())], "{notes:?}");
    assert!(logged(&notes).is_empty(), "{notes:?}");
    assert_eq!(std::fs::read(tdir.join("state.json")).unwrap(), state);
}

// AC-13: a state or config that cannot be read gives no diagnostic and one Error log line.
#[test]
fn bad_state_is_logged() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    let tdir = dir.join(".stapel/tickets/ABC-1");
    let ticket = tdir.join("ticket.md");
    let uri = file_uri(&ticket);
    let text = std::fs::read_to_string(&ticket).unwrap();
    let good_state = std::fs::read_to_string(tdir.join("state.json")).unwrap();
    let errors = |notes: &[Value]| -> Vec<String> {
        notes
            .iter()
            .filter(|n| n["method"] == json!("window/logMessage"))
            .filter(|n| n["params"]["type"] == json!(1))
            .map(|n| n["params"]["message"].as_str().unwrap().to_string())
            .collect()
    };
    let mut c = Client::start();
    c.initialize();

    // a malformed state.json, over an open and two changes: logged once
    std::fs::write(tdir.join("state.json"), "{ not json").unwrap();
    c.open(&uri, &text);
    let (resp, notes) = c.code_action(&uri, 0);
    assert!(titles(&resp).len() >= 3, "{resp}");
    assert!(published(&notes, &uri).iter().all(|l| l.is_empty()), "{notes:?}");
    let first = errors(&notes);
    assert_eq!(first.len(), 1, "{notes:?}");
    assert!(first[0].contains("state.json"), "{first:?}");
    for version in [2, 3] {
        c.change(&uri, version, &format!("{text}\nmore {version}\n"));
        let (resp, notes) = c.code_action(&uri, 0);
        assert!(titles(&resp).len() >= 3, "{resp}");
        assert!(published(&notes, &uri).iter().all(|l| l.is_empty()), "{notes:?}");
        assert!(errors(&notes).is_empty(), "{notes:?}");
    }

    // the state reads again: diagnostics come
    std::fs::write(tdir.join("state.json"), &good_state).unwrap();
    c.change(&uri, 4, &text);
    let (_, notes) = c.code_action(&uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    assert!(!got[0].is_empty(), "{notes:?}");
    assert!(errors(&notes).is_empty(), "{notes:?}");

    // a stapel.toml that no longer loads
    std::fs::write(dir.join(".stapel/stapel.toml"), "this is = [not valid").unwrap();
    for version in [5, 6] {
        c.change(&uri, version, &format!("{text}\nmore {version}\n"));
        let (_, notes) = c.code_action(&uri, 0);
        assert!(published(&notes, &uri).iter().all(|l| l.is_empty()), "{notes:?}");
        let got = errors(&notes);
        if version == 5 {
            assert_eq!(got.len(), 1, "{notes:?}");
            assert!(got[0].contains("stapel.toml"), "{got:?}");
        } else {
            assert!(got.is_empty(), "{notes:?}");
        }
    }

    // a good document in another repository is still served
    let other = common::repo_with_ticket();
    let other_ticket = other.path().join(".stapel/tickets/ABC-1/ticket.md");
    let other_uri = file_uri(&other_ticket);
    c.open(&other_uri, &std::fs::read_to_string(&other_ticket).unwrap());
    let (resp, notes) = c.code_action(&other_uri, 0);
    assert!(titles(&resp).len() >= 3, "{resp}");
    assert!(!published(&notes, &other_uri).is_empty(), "{notes:?}");
}

/// The (line, message) of the Hint diagnostics in `list`; the fixed fields are checked on the way.
fn hint_lines(list: &[Value], text: &str) -> Vec<u64> {
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let mut out: Vec<u64> = list
        .iter()
        .filter(|d| d["severity"] == json!(4))
        .map(|d| {
            assert_eq!(d["source"], json!("stapel"), "{d}");
            assert_eq!(d["message"], json!("changed since the last confirmation"), "{d}");
            let line = d["range"]["start"]["line"].as_u64().unwrap();
            assert_eq!(d["range"]["start"]["character"], json!(0), "{d}");
            assert_eq!(d["range"]["end"]["line"], json!(line), "{d}");
            assert_eq!(
                d["range"]["end"]["character"],
                json!(lines[line as usize].encode_utf16().count()),
                "{d}"
            );
            line
        })
        .collect();
    out.sort();
    out
}

fn line_of(text: &str, line: &str) -> u64 {
    text.split('\n')
        .position(|l| l.trim_end_matches('\r') == line)
        .unwrap_or_else(|| panic!("no line {line}")) as u64
}

// AC-11: Hints on the lines that the line diff marks inserted against the confirmed text.
#[test]
fn changed_lines() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    common::set_section(dir, "ABC-1", "Spec", "- s1\n- s2\n- s3\n- s4");
    common::set_section(dir, "ABC-1", "Design", "- a\n- a\n- b");
    common::confirm_these(dir, "ABC-1", &["spec", "design", "proof"]);
    // the confirmation of Proof keeps no text
    let tdir = dir.join(".stapel/tickets/ABC-1");
    let mut state: Value =
        serde_json::from_str(&std::fs::read_to_string(tdir.join("state.json")).unwrap()).unwrap();
    for c in state["confirmations"].as_array_mut().unwrap() {
        if c["section"] == json!("proof") {
            c.as_object_mut().unwrap().remove("text");
        }
    }
    std::fs::write(tdir.join("state.json"), state.to_string()).unwrap();

    let ticket = tdir.join("ticket.md");
    let uri = file_uri(&ticket);
    let text = std::fs::read_to_string(&ticket).unwrap();
    let mut c = Client::start();
    c.initialize();
    c.open(&uri, &text);
    let (_, notes) = c.code_action(&uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    assert_eq!(hint_lines(&got[0], &text), Vec::<u64>::new(), "unchanged");

    let mut version = 1;
    let mut hints = |c: &mut Client, buffer: &str| -> (Vec<u64>, Vec<Value>) {
        version += 1;
        c.change(&uri, version, buffer);
        let (_, notes) = c.code_action(&uri, 0);
        let got = published(&notes, &uri);
        assert_eq!(got.len(), 1, "{notes:?}");
        (hint_lines(&got[0], buffer), notes)
    };

    // one line changed, one added, one removed: Hints on the changed and the added line only
    let buffer = text.replace("- s2\n", "- S2 changed\n").replace("- s4\n", "- s5 new\n");
    assert_ne!(buffer, text);
    let want = vec![line_of(&buffer, "- S2 changed"), line_of(&buffer, "- s5 new")];
    let (got, _) = hints(&mut c, &buffer);
    assert_eq!(got, want, "changed, added");

    // `- a` twice and a third added: one Hint
    let buffer = text.replace("- a\n- a\n- b\n", "- a\n- a\n- a\n- b\n");
    assert_ne!(buffer, text);
    let first = line_of(&buffer, "- a");
    let (got, _) = hints(&mut c, &buffer);
    // `similar` marks the first of the three equal lines as the inserted one
    assert_eq!(got, vec![first], "a duplicate line");

    // a fresh section with CRLF line ends: none
    let (got, _) = hints(&mut c, &text.replace('\n', "\r\n"));
    assert_eq!(got, Vec::<u64>::new(), "fresh with CRLF");

    // trailing spaces and two leading blank lines on the other lines: the changed line only
    let buffer = text.replace(
        "- s1\n- s2\n- s3\n- s4\n",
        "\n\n- s1  \n- S2 changed \n- s3\t\n- s4  \n",
    );
    let want = vec![line_of(&buffer, "- S2 changed ")];
    let (got, _) = hints(&mut c, &buffer);
    assert_eq!(got, want, "whitespace");

    // a section confirmed without stored text: none
    let buffer = text.replace("The proof.", "The proof, edited.");
    assert_ne!(buffer, text);
    let (got, _) = hints(&mut c, &buffer);
    assert_eq!(got, Vec::<u64>::new(), "no stored text");

    // a body of 5001 lines: none and one log line
    let big: String = (0..5001).map(|i| format!("- line {i}\n")).collect();
    let buffer = text.replace("- s1\n- s2\n- s3\n- s4\n", &big);
    assert_ne!(buffer, text);
    let (got, notes) = hints(&mut c, &buffer);
    assert_eq!(got, Vec::<u64>::new(), "too long");
    let logs = notes
        .iter()
        .filter(|n| n["method"] == json!("window/logMessage"))
        .count();
    assert_eq!(logs, 1, "{notes:?}");
}

// AC-11: the confirmed text is split as `ticket::scan` splits, with no empty element after a final line end.
#[test]
fn crlf_confirmed_text() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    common::set_section(dir, "ABC-1", "Spec", "- s1\n- s2\n- s3\n- s4");
    common::confirm_these(dir, "ABC-1", &["spec"]);
    let tdir = dir.join(".stapel/tickets/ABC-1");
    let ticket = tdir.join("ticket.md");
    let uri = file_uri(&ticket);
    let text = std::fs::read_to_string(&ticket).unwrap();
    let set_text = |stored: &str| {
        let mut state: Value =
            serde_json::from_str(&std::fs::read_to_string(tdir.join("state.json")).unwrap())
                .unwrap();
        for c in state["confirmations"].as_array_mut().unwrap() {
            if c["section"] == json!("spec") {
                c["text"] = json!(stored);
            }
        }
        std::fs::write(tdir.join("state.json"), state.to_string()).unwrap();
    };
    let mut c = Client::start();
    c.initialize();
    c.open(&uri, &text);
    let _ = c.code_action(&uri, 0);
    let mut version = 1;
    let mut hints = |c: &mut Client, buffer: &str| -> Vec<u64> {
        version += 1;
        c.change(&uri, version, buffer);
        let (_, notes) = c.code_action(&uri, 0);
        let got = published(&notes, &uri);
        assert_eq!(got.len(), 1, "{notes:?}");
        hint_lines(&got[0], buffer)
    };

    // a confirmed text with CRLF line ends and one changed line: one Hint on that line
    set_text("- s1\r\n- s2\r\n- s3\r\n- s4\r\n");
    let buffer = text.replace("- s2\n", "- S2 changed\n");
    assert_ne!(buffer, text);
    let want = vec![line_of(&buffer, "- S2 changed")];
    assert_eq!(hints(&mut c, &buffer), want, "CRLF text");

    // a confirmed text of exactly 5000 lines with a final line end: Hints still computed
    let old: String = (0..5000).map(|i| format!("- line {i}\n")).collect();
    set_text(&old);
    let mut new: String = (0..4999).map(|i| format!("- line {i}\n")).collect();
    new.push_str("- last changed\n");
    let buffer = text.replace("- s1\n- s2\n- s3\n- s4\n", &new);
    assert_ne!(buffer, text);
    let want = vec![line_of(&buffer, "- last changed")];
    assert_eq!(hints(&mut c, &buffer), want, "5000 lines");
}

// AC-12: after a button run the diagnostics of every open `ticket.md` are published again.
#[test]
fn diagnostics_after_ok() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    common::stapel(dir).args(["page", "ABC-1"]).assert().success();
    let tdir = dir.join(".stapel/tickets/ABC-1");
    let page = tdir.join("page.md");
    let bytes = std::fs::read(&page).unwrap();
    let hash = stapel_core::page::page_hash(&bytes);
    let page_uri = file_uri(&page);
    let ticket = tdir.join("ticket.md");
    let uri = file_uri(&ticket);
    let mut c = Client::start();
    c.initialize();
    c.open(&uri, &std::fs::read_to_string(&ticket).unwrap());
    c.open(&page_uri, &String::from_utf8(bytes).unwrap());
    let (_, notes) = c.code_action(&page_uri, 0);
    let got = published(&notes, &uri);
    assert_eq!(got.len(), 1, "{notes:?}");
    assert!(!got[0].is_empty(), "{notes:?}");

    let (resp, notes) = c.execute("stapel.ok", json!([page_uri, hash]));
    assert!(resp.get("error").is_none(), "{resp}");
    let got = published(&notes, &uri);
    assert_eq!(got, vec![Vec::<Value>::new()], "{notes:?}");
    assert!(published(&notes, &page_uri).is_empty(), "{notes:?}");
}

// AC-14: the Zed extension only starts `stapel lsp` from PATH for Markdown.
#[test]
fn the_zed_extension_starts_stapel_lsp() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../editors/zed");
    let manifest: toml::Value =
        toml::from_str(&std::fs::read_to_string(root.join("extension.toml")).unwrap()).unwrap();
    let server = &manifest["language_servers"]["stapel"];
    let languages: Vec<&str> =
        server["languages"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(languages, vec!["Markdown"], "{manifest:?}");

    let lib = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
    assert!(lib.contains("worktree.which(\"stapel\")"), "{lib}");
    assert!(lib.contains("vec![\"lsp\".to_string()]"), "{lib}");
    assert!(lib.contains("stapel not found on PATH"), "{lib}");

    let out = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

// AC-1, AC-5, AC-6 (XR1-5): refusals the first tests leave out.
#[test]
fn refusals_beyond_the_first_cases() {
    // `stapel lsp` takes no argument: a usage error, exit 2
    let out = Command::new(env!("CARGO_BIN_EXE_stapel"))
        .args(["lsp", "extra"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");

    let repo = common::repo_with_ticket();
    common::stapel(repo.path())
        .args(["page", "ABC-1"])
        .assert()
        .success();
    let tdir = repo.path().join(".stapel/tickets/ABC-1");
    let ticket = tdir.join("ticket.md");
    let page = tdir.join("page.md");
    let bytes = std::fs::read(&page).unwrap();
    let hash = stapel_core::page::page_hash(&bytes);
    let ticket_uri = file_uri(&ticket);
    let page_uri = file_uri(&page);
    let readme = file_uri(&repo.path().join("README.md"));
    let bad_args = "stapel lsp: the arguments must be the URI of an open ticket.md or page.md";
    let agent = "stapel lsp: this editor was started from an agent's shell (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); start it from your own shell to use the buttons";
    let state = std::fs::read(tdir.join("state.json")).unwrap();

    // the server of a client: with the agent variable set, or without it
    fn start(agent_shell: bool) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_stapel"));
        cmd.arg("lsp")
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if agent_shell {
            cmd.env("CLAUDECODE", "1");
        }
        let mut child = cmd.spawn().unwrap();
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Client {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    for agent_shell in [false, true] {
        let mut c = start(agent_shell);
        c.initialize();
        c.open(&ticket_uri, &std::fs::read_to_string(&ticket).unwrap());
        c.open(&page_uri, &String::from_utf8(bytes.clone()).unwrap());

        // a number as the second argument, three arguments, a closed document
        let cases = [
            ("stapel.ok", json!([page_uri, 5])),
            ("stapel.ok", json!([page_uri, hash, hash])),
            ("stapel.ok", json!([page_uri, null])),
            ("stapel.status", json!([readme])),
            ("stapel.page", json!([ticket_uri, ticket_uri])),
        ];
        for (command, args) in cases {
            let (resp, notes) = c.execute(command, args.clone());
            assert_eq!(resp["error"]["code"], json!(-32602), "{agent_shell} {command} {args}: {resp}");
            assert_eq!(resp["error"]["message"], json!(bad_args), "{agent_shell} {command} {args}: {resp}");
            assert!(shown(&notes).is_empty(), "{agent_shell} {command} {args}: {notes:?}");
            assert!(logged(&notes).is_empty(), "{agent_shell} {command} {args}: {notes:?}");
        }
        let (resp, _) = c.execute("stapel.close", json!([ticket_uri]));
        assert_eq!(
            resp["error"]["message"],
            json!("stapel lsp: unknown command stapel.close"),
            "{agent_shell}: {resp}"
        );

        if agent_shell {
            // a valid ok on a page that is current: still nothing runs
            let (resp, notes) = c.execute("stapel.ok", json!([page_uri, hash]));
            assert_eq!(resp["result"], Value::Null, "{resp}");
            assert!(resp.get("error").is_none(), "{resp}");
            assert_eq!(shown(&notes), vec![(1, agent.to_string())], "{notes:?}");
            assert!(logged(&notes).is_empty(), "{notes:?}");
        }
        assert_eq!(std::fs::read(tdir.join("state.json")).unwrap(), state);
        assert_eq!(std::fs::read(&page).unwrap(), bytes);
        let _ = c.child.kill();
    }
}
