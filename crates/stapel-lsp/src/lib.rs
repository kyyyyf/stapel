//! LSP server for Zed: section actions and diagnostics on the ticket documents.

use lsp_server::{Connection, ErrorCode, Message, Notification, Response};
use lsp_types::{
    CodeAction, CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability, Command,
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, ExecuteCommandOptions,
    ExecuteCommandParams, InitializeParams, LogMessageParams, MessageType, Position,
    PublishDiagnosticsParams, Range, ServerCapabilities, ShowMessageParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Uri,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// A ticket document: its root, its key and whether it is `page.md`.
pub struct TicketDoc {
    pub root: PathBuf,
    pub key: String,
    pub is_page: bool,
}

/// Classifies a URI as a ticket document; `None` for anything else.
pub fn classify(uri: &Uri) -> Option<TicketDoc> {
    let doc = locate(uri)?;
    let text = std::fs::read_to_string(doc.root.join(".stapel/stapel.toml")).ok()?;
    let config = stapel_core::config::Config::parse_at(&text, &doc.root).ok()?;
    stapel_core::tickets::key_number(&config, &doc.key)?;
    Some(doc)
}

/// The place of a URI: `ticket.md` or `page.md` in a ticket folder with a `state.json`; the
/// configuration is not read.
fn locate(uri: &Uri) -> Option<TicketDoc> {
    if !uri.scheme()?.as_str().eq_ignore_ascii_case("file") {
        return None;
    }
    let decoded = uri.path().as_estr().decode().into_string().ok()?;
    let path = PathBuf::from(decoded.as_ref() as &str);
    if !path.is_absolute() {
        return None;
    }
    let is_page = match path.file_name()?.to_str()? {
        "ticket.md" => false,
        "page.md" => true,
        _ => return None,
    };
    let key_dir = path.parent()?;
    let key = key_dir.file_name()?.to_str()?.to_string();
    let tickets = key_dir.parent()?;
    if tickets.file_name()? != "tickets" {
        return None;
    }
    let stapel = tickets.parent()?;
    if stapel.file_name()? != ".stapel" {
        return None;
    }
    let root = stapel.parent()?.to_path_buf();
    if !key_dir.join("state.json").is_file() {
        return None;
    }
    Some(TicketDoc { root, key, is_page })
}

/// The Information diagnostics on the heading of each waiting section of `text`, or the cause
/// (naming the file) when `stapel.toml` or `state.json` cannot be read.
fn waiting_diagnostics(doc: &TicketDoc, text: &str) -> Result<Option<Computed>, String> {
    let toml = doc.root.join(".stapel/stapel.toml");
    let source = std::fs::read_to_string(&toml)
        .map_err(|e| format!("cannot read {}: {e}", toml.display()))?;
    let config = stapel_core::config::Config::parse_at(&source, &doc.root)
        .map_err(|e| format!("cannot load {}: {e}", toml.display()))?;
    if stapel_core::tickets::key_number(&config, &doc.key).is_none() {
        return Ok(None);
    }
    let path = doc.root.join(format!(".stapel/tickets/{}/state.json", doc.key));
    let state = match stapel_core::state::load(&path) {
        stapel_core::state::Loaded::State(state) => state,
        // A hand-written state of STP-1 has no confirmations to compare with.
        stapel_core::state::Loaded::Legacy(_) => return Ok(None),
        stapel_core::state::Loaded::Unreadable(why) => {
            return Err(format!("cannot read {}: {why}", path.display()));
        }
    };
    let sections = stapel_core::ticket::parse(text);
    let scanned = stapel_core::ticket::scan(text);
    let mut out = Vec::new();
    let mut logs = Vec::new();
    for def in config.sections.iter().filter(|s| s.owner != "generated") {
        if stapel_core::stage::is_fresh(&config, &sections, &state, &def.id) {
            continue;
        }
        if let Some(hints) = changed_lines(&scanned, &sections, &state, &def.id, &def.title, &mut logs)
        {
            out.extend(hints);
        }
        for (line, s) in scanned.iter().enumerate() {
            let Some((2, title)) = &s.heading else { continue };
            if !title.trim().eq_ignore_ascii_case(def.title.trim()) {
                continue;
            }
            let end = s.line.trim_end_matches('\r').encode_utf16().count();
            out.push(Diagnostic {
                range: Range::new(
                    Position::new(line as u32, 0),
                    Position::new(line as u32, end as u32),
                ),
                severity: Some(DiagnosticSeverity::INFORMATION),
                source: Some("stapel".to_string()),
                message: format!("waiting for confirmation (owner: {})", def.owner),
                ..Default::default()
            });
        }
    }
    Ok(Some(Computed { list: out, logs }))
}

/// The diagnostics of a buffer and the lines for the log.
struct Computed {
    list: Vec<Diagnostic>,
    logs: Vec<String>,
}

/// The longest body or confirmed text that gets Hints, in lines.
const MAX_HINT_LINES: usize = 5000;

/// The Hints on the lines of a changed section that the line diff marks inserted against its
/// confirmed text; none for a fresh section, a section without a stored text, a missing or
/// duplicated heading, or a text of more than `MAX_HINT_LINES` lines (logged).
fn changed_lines(
    scanned: &[stapel_core::ticket::Scanned],
    sections: &[stapel_core::ticket::Section],
    state: &stapel_core::state::State,
    id: &str,
    title: &str,
    logs: &mut Vec<String>,
) -> Option<Vec<Diagnostic>> {
    let confirmed = stapel_core::stage::latest(state, id)?;
    let old_text = confirmed.text.as_deref()?;
    let body = match stapel_core::ticket::find(sections, title) {
        stapel_core::ticket::Lookup::Found(body) => body,
        _ => return None,
    };
    if stapel_core::hash::section_hash(body) == confirmed.hash {
        return None;
    }
    let heading = scanned.iter().position(|s| {
        matches!(&s.heading, Some((2, t)) if t.trim().eq_ignore_ascii_case(title.trim()))
    })?;
    let end = scanned[heading + 1..]
        .iter()
        .position(|s| matches!(s.heading, Some((2, _))))
        .map_or(scanned.len(), |i| heading + 1 + i);
    let first = heading + 1;
    let new_lines: Vec<String> = scanned[first..end]
        .iter()
        .map(|s| stapel_core::hash::normal_form(&s.line))
        .collect();
    let old_lines: Vec<String> = old_text
        .split(['\n', '\r'])
        .map(stapel_core::hash::normal_form)
        .collect();
    if new_lines.len() > MAX_HINT_LINES || old_lines.len() > MAX_HINT_LINES {
        logs.push(format!(
            "stapel lsp: section {id} has more than {MAX_HINT_LINES} lines; no changed-line hints"
        ));
        return None;
    }
    let old_refs: Vec<&str> = old_lines.iter().map(String::as_str).collect();
    let new_refs: Vec<&str> = new_lines.iter().map(String::as_str).collect();
    let diff = similar::TextDiff::from_slices(&old_refs, &new_refs);
    let mut out = Vec::new();
    for change in diff.iter_all_changes() {
        if change.tag() != similar::ChangeTag::Insert {
            continue;
        }
        let Some(index) = change.new_index() else { continue };
        if new_lines[index].is_empty() {
            continue;
        }
        let line = first + index;
        let width = scanned[line].line.trim_end_matches('\r').encode_utf16().count();
        out.push(Diagnostic {
            range: Range::new(
                Position::new(line as u32, 0),
                Position::new(line as u32, width as u32),
            ),
            severity: Some(DiagnosticSeverity::HINT),
            source: Some("stapel".to_string()),
            message: "changed since the last confirmation".to_string(),
            ..Default::default()
        });
    }
    Some(out)
}

/// What the server knows about the diagnostics it sent.
#[derive(Default)]
struct Published {
    /// Documents with a non-empty list on the client.
    shown: HashSet<Uri>,
    /// The last cause logged for a document, until its files read again.
    bad: HashMap<Uri, String>,
}

fn publish(connection: &Connection, uri: &Uri, diagnostics: Vec<Diagnostic>) {
    let params = PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics,
        version: None,
    };
    let note = Notification::new("textDocument/publishDiagnostics".to_string(), params);
    let _ = connection.sender.send(Message::Notification(note));
}

/// Publishes the diagnostics of an open `ticket.md` from its buffer; logs a bad state once.
fn refresh(connection: &Connection, docs: &HashMap<Uri, String>, uri: &Uri, sent: &mut Published) {
    let Some(text) = docs.get(uri) else { return };
    let Some(doc) = locate(uri).filter(|d| !d.is_page) else {
        return;
    };
    match waiting_diagnostics(&doc, text) {
        Ok(Some(Computed { list, logs })) => {
            for message in logs {
                let note = Notification::new(
                    "window/logMessage".to_string(),
                    LogMessageParams {
                        typ: MessageType::WARNING,
                        message,
                    },
                );
                let _ = connection.sender.send(Message::Notification(note));
            }
            sent.bad.remove(uri);
            if list.is_empty() {
                sent.shown.remove(uri);
            } else {
                sent.shown.insert(uri.clone());
            }
            publish(connection, uri, list);
        }
        Ok(None) => {}
        Err(cause) => {
            if sent.shown.remove(uri) {
                publish(connection, uri, Vec::new());
            }
            if sent.bad.get(uri) != Some(&cause) {
                let note = Notification::new(
                    "window/logMessage".to_string(),
                    LogMessageParams {
                        typ: MessageType::ERROR,
                        message: format!("stapel lsp: {cause}"),
                    },
                );
                let _ = connection.sender.send(Message::Notification(note));
                sent.bad.insert(uri.clone(), cause);
            }
        }
    }
}

fn button(title: String, command: &str, uri: &Uri) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.clone(),
        command: Some(Command {
            title,
            command: command.to_string(),
            arguments: Some(vec![serde_json::json!(uri.as_str())]),
        }),
        ..Default::default()
    })
}

/// The buttons of an open ticket document; none for any other URI.
fn buttons(docs: &HashMap<Uri, String>, uri: &Uri) -> Vec<CodeActionOrCommand> {
    if !docs.contains_key(uri) {
        return Vec::new();
    }
    let Some(doc) = classify(uri) else {
        return Vec::new();
    };
    let mut list = vec![
        button(format!("stapel status {}", doc.key), "stapel.status", uri),
        button("stapel next".to_string(), "stapel.next", uri),
        button(format!("stapel page {}", doc.key), "stapel.page", uri),
    ];
    if let Some(hash) = current_page(docs, uri, &doc) {
        let title = format!("stapel ok {}", doc.key);
        list.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: title.clone(),
            command: Some(Command {
                title,
                command: "stapel.ok".to_string(),
                arguments: Some(vec![serde_json::json!(uri.as_str()), serde_json::json!(hash)]),
            }),
            ..Default::default()
        }));
    }
    list
}

/// The `ok` button's run is the child's `stapel ok KEY`, which records `via: terminal`.
///
/// The hash of `page.md` when it is a current page: on disk it is the text `stapel page KEY`
/// would write now, a section waits, and the open buffer equals the disk.
fn current_page(docs: &HashMap<Uri, String>, uri: &Uri, doc: &TicketDoc) -> Option<String> {
    if !doc.is_page {
        return None;
    }
    let text = std::fs::read_to_string(doc.root.join(".stapel/stapel.toml")).ok()?;
    let config = stapel_core::config::Config::parse_at(&text, &doc.root).ok()?;
    let waiting = stapel_core::confirm::prepare_waiting(&doc.root, &config, &doc.key).ok()?;
    if waiting.is_empty() {
        return None;
    }
    let want = stapel_core::page::page_text(&config, &waiting);
    let disk = std::fs::read(doc.root.join(format!(".stapel/tickets/{}/page.md", doc.key))).ok()?;
    if disk != want.as_bytes() || docs.get(uri)?.as_bytes() != disk.as_slice() {
        return None;
    }
    Some(stapel_core::page::page_hash(&disk))
}

/// The command names of the buttons.
pub const COMMANDS: [&str; 4] = ["stapel.status", "stapel.next", "stapel.page", "stapel.ok"];

fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                ..Default::default()
            },
        )),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: COMMANDS.iter().map(|c| c.to_string()).collect(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Serves LSP on standard input and output; returns the process exit code.
pub fn run() -> i32 {
    let (connection, io_threads) = Connection::stdio();
    let code = serve(&connection);
    drop(connection);
    let _ = io_threads.join();
    code
}

fn serve(connection: &Connection) -> i32 {
    let (id, params) = match connection.initialize_start() {
        Ok(pair) => pair,
        Err(_) => return 1,
    };
    if serde_json::from_value::<InitializeParams>(params).is_err() {
        return 1;
    }
    let result = serde_json::json!({ "capabilities": capabilities() });
    if connection.initialize_finish(id, result).is_err() {
        return 1;
    }
    let mut shutdown = false;
    let mut docs: HashMap<Uri, String> = HashMap::new();
    let mut sent = Published::default();
    while let Ok(msg) = connection.receiver.recv() {
        match msg {
            Message::Request(req) => {
                if shutdown {
                    reply_error(connection, req.id, ErrorCode::InvalidRequest, "shutting down");
                } else if req.method == "textDocument/codeAction" {
                    let actions = match serde_json::from_value::<CodeActionParams>(req.params) {
                        Ok(p) => buttons(&docs, &p.text_document.uri),
                        Err(e) => {
                            reply_error(connection, req.id, ErrorCode::InvalidParams, &e.to_string());
                            continue;
                        }
                    };
                    let _ = connection
                        .sender
                        .send(Message::Response(Response::new_ok(req.id, actions)));
                } else if req.method == "workspace/executeCommand" {
                    let params = match serde_json::from_value::<ExecuteCommandParams>(req.params) {
                        Ok(p) => p,
                        Err(e) => {
                            reply_error(connection, req.id, ErrorCode::InvalidParams, &e.to_string());
                            continue;
                        }
                    };
                    match button_args(&docs, &params) {
                        Ok((args, doc)) => {
                            if agent_shell() {
                                show(
                                    connection,
                                    MessageType::ERROR,
                                    "stapel lsp: this editor was started from an agent's shell (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); start it from your own shell to use the buttons".to_string(),
                                );
                            } else if params.command == "stapel.ok" && !offered_page_is_current(&docs, &params, &doc) {
                                show(
                                    connection,
                                    MessageType::ERROR,
                                    format!(
                                        "stapel lsp: page.md is not current; run stapel page {} and read it again",
                                        doc.key
                                    ),
                                );
                            } else {
                                run_button(connection, &doc, &args);
                                let open: Vec<Uri> = docs.keys().cloned().collect();
                                for uri in open {
                                    refresh(connection, &docs, &uri, &mut sent);
                                }
                            }
                            let _ = connection
                                .sender
                                .send(Message::Response(Response::new_ok(req.id, ())));
                        }
                        Err(text) => {
                            reply_error(connection, req.id, ErrorCode::InvalidParams, &text)
                        }
                    }
                } else if req.method == "shutdown" {
                    shutdown = true;
                    let _ = connection
                        .sender
                        .send(Message::Response(Response::new_ok(req.id, ())));
                } else {
                    let text = format!("unhandled method {}", req.method);
                    reply_error(connection, req.id, ErrorCode::MethodNotFound, &text);
                }
            }
            Message::Notification(note) => {
                match note.method.as_str() {
                    "exit" => return if shutdown { 0 } else { 1 },
                    "textDocument/didOpen" => {
                        if let Ok(p) = serde_json::from_value::<DidOpenTextDocumentParams>(note.params) {
                            let uri = p.text_document.uri;
                            docs.insert(uri.clone(), p.text_document.text);
                            refresh(connection, &docs, &uri, &mut sent);
                        }
                    }
                    "textDocument/didChange" => {
                        if let Ok(p) =
                            serde_json::from_value::<DidChangeTextDocumentParams>(note.params)
                        {
                            let uri = p.text_document.uri;
                            if let (Some(text), Some(slot)) =
                                (p.content_changes.into_iter().last(), docs.get_mut(&uri))
                            {
                                *slot = text.text;
                                refresh(connection, &docs, &uri, &mut sent);
                            }
                        }
                    }
                    "textDocument/didSave" => {
                        if let Ok(p) =
                            serde_json::from_value::<DidSaveTextDocumentParams>(note.params)
                        {
                            refresh(connection, &docs, &p.text_document.uri, &mut sent);
                        }
                    }
                    "textDocument/didClose" => {
                        if let Ok(p) =
                            serde_json::from_value::<DidCloseTextDocumentParams>(note.params)
                        {
                            let uri = p.text_document.uri;
                            let was_open = docs.remove(&uri).is_some();
                            sent.shown.remove(&uri);
                            sent.bad.remove(&uri);
                            if was_open && locate(&uri).is_some_and(|d| !d.is_page) {
                                publish(connection, &uri, Vec::new());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Message::Response(_) => {}
        }
    }
    1
}

fn reply_error(connection: &Connection, id: lsp_server::RequestId, code: ErrorCode, text: &str) {
    let resp = Response::new_err(id, code as i32, text.to_string());
    let _ = connection.sender.send(Message::Response(resp));
}

/// The `stapel` arguments and the document of a button command on an open ticket document.
fn button_args(
    docs: &HashMap<Uri, String>,
    params: &ExecuteCommandParams,
) -> Result<(Vec<String>, TicketDoc), String> {
    let command = params.command.as_str();
    if !COMMANDS.contains(&command) {
        return Err(format!("stapel lsp: unknown command {command}"));
    }
    let bad = || "stapel lsp: the arguments must be the URI of an open ticket.md or page.md".to_string();
    let want = if command == "stapel.ok" { 2 } else { 1 };
    if params.arguments.len() != want {
        return Err(bad());
    }
    if want == 2 && !params.arguments[1].is_string() {
        return Err(bad());
    }
    let uri: Uri = params.arguments[0]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(bad)?;
    if !docs.contains_key(&uri) {
        return Err(bad());
    }
    let doc = classify(&uri).ok_or_else(bad)?;
    if command == "stapel.ok" && !doc.is_page {
        return Err(bad());
    }
    let args = match command {
        "stapel.status" => vec!["status".to_string(), doc.key.clone()],
        "stapel.next" => vec!["next".to_string()],
        "stapel.page" => vec!["page".to_string(), doc.key.clone()],
        _ => vec!["ok".to_string(), doc.key.clone()],
    };
    Ok((args, doc))
}

/// True when `page.md` is a current page now and its hash is the offered one.
fn offered_page_is_current(
    docs: &HashMap<Uri, String>,
    params: &ExecuteCommandParams,
    doc: &TicketDoc,
) -> bool {
    let Some(uri) = params.arguments[0].as_str().and_then(|s| s.parse::<Uri>().ok()) else {
        return false;
    };
    let offered = params.arguments[1].as_str();
    match current_page(docs, &uri, doc) {
        Some(hash) => Some(hash.as_str()) == offered,
        None => false,
    }
}

/// True when the server's environment says it was started from an agent's shell.
fn agent_shell() -> bool {
    std::env::var_os("CLAUDECODE").is_some() || std::env::var_os("CLAUDE_CODE_ENTRYPOINT").is_some()
}

fn show(connection: &Connection, typ: MessageType, message: String) {
    let params = ShowMessageParams { typ, message };
    let note = Notification::new("window/showMessage".to_string(), params);
    let _ = connection.sender.send(Message::Notification(note));
}

/// Runs this executable with `args` in the root, logs the output and shows the outcome.
fn run_button(connection: &Connection, doc: &TicketDoc, args: &[String]) {
    let label = format!("stapel {}", args.join(" "));
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            show(connection, MessageType::ERROR, format!("{label}: could not run: {e}"));
            return;
        }
    };
    let output = std::process::Command::new(exe)
        .args(args)
        .current_dir(&doc.root)
        .stdin(std::process::Stdio::null())
        .output();
    let output = match output {
        Ok(o) => o,
        Err(e) => {
            show(connection, MessageType::ERROR, format!("{label}: could not run: {e}"));
            return;
        }
    };
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let note = Notification::new(
        "window/logMessage".to_string(),
        LogMessageParams {
            typ: MessageType::INFO,
            message: format!("{label}\n{text}"),
        },
    );
    let _ = connection.sender.send(Message::Notification(note));
    let first = text.lines().find(|l| !l.trim().is_empty()).map(str::trim);
    match output.status.code() {
        Some(0) => show(
            connection,
            MessageType::INFO,
            format!("{label}: {}", first.unwrap_or("done")),
        ),
        Some(n) => {
            let message = match first {
                Some(l) => format!("{label}: exit {n}: {l}"),
                None => format!("{label}: exit {n}"),
            };
            show(connection, MessageType::ERROR, message);
        }
        None => {
            use std::os::unix::process::ExitStatusExt;
            let sig = output.status.signal().unwrap_or(0);
            show(connection, MessageType::ERROR, format!("{label}: killed by signal {sig}"));
        }
    }
}
