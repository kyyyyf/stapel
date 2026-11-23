//! STP-39: `stapel lsp`, the language server on standard input and output.

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
