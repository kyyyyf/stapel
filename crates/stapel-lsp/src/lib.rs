//! LSP server for Zed: section actions and diagnostics on the ticket documents.

use lsp_server::{Connection, ErrorCode, Message, Notification, Response};
use lsp_types::{
    CodeAction, CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability, Command,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    ExecuteCommandOptions, ExecuteCommandParams, InitializeParams, LogMessageParams, MessageType,
    ShowMessageParams, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// A ticket document: its root, its key and whether it is `page.md`.
pub struct TicketDoc {
    pub root: PathBuf,
    pub key: String,
    pub is_page: bool,
}

/// Classifies a URI as a ticket document; `None` for anything else.
pub fn classify(uri: &Uri) -> Option<TicketDoc> {
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
    let text = std::fs::read_to_string(stapel.join("stapel.toml")).ok()?;
    let config = stapel_core::config::Config::parse_at(&text, &root).ok()?;
    stapel_core::tickets::key_number(&config, &key)?;
    Some(TicketDoc { root, key, is_page })
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
    vec![
        button(format!("stapel status {}", doc.key), "stapel.status", uri),
        button("stapel next".to_string(), "stapel.next", uri),
        button(format!("stapel page {}", doc.key), "stapel.page", uri),
    ]
}

/// The command names of the buttons.
pub const COMMANDS: [&str; 4] = ["stapel.status", "stapel.next", "stapel.page", "stapel.ok"];

fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
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
                        Some((args, doc)) => {
                            if params.command == "stapel.ok" {
                                // The `ok` run comes with its page check (steps 7 and 8).
                                let text = format!(
                                    "stapel lsp: page.md is not current; run stapel page {} and read it again",
                                    doc.key
                                );
                                show(connection, MessageType::ERROR, text);
                            } else {
                                run_button(connection, &doc, &args);
                            }
                            let _ = connection
                                .sender
                                .send(Message::Response(Response::new_ok(req.id, ())));
                        }
                        None => reply_error(
                            connection,
                            req.id,
                            ErrorCode::InvalidParams,
                            "stapel lsp: the arguments must be the URI of an open ticket.md or page.md",
                        ),
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
                            docs.insert(p.text_document.uri, p.text_document.text);
                        }
                    }
                    "textDocument/didChange" => {
                        if let Ok(p) =
                            serde_json::from_value::<DidChangeTextDocumentParams>(note.params)
                        {
                            if let (Some(text), Some(slot)) = (
                                p.content_changes.into_iter().last(),
                                docs.get_mut(&p.text_document.uri),
                            ) {
                                *slot = text.text;
                            }
                        }
                    }
                    "textDocument/didClose" => {
                        if let Ok(p) =
                            serde_json::from_value::<DidCloseTextDocumentParams>(note.params)
                        {
                            docs.remove(&p.text_document.uri);
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
) -> Option<(Vec<String>, TicketDoc)> {
    let uri: Uri = params.arguments.first()?.as_str()?.parse().ok()?;
    if !docs.contains_key(&uri) {
        return None;
    }
    let doc = classify(&uri)?;
    let args = match params.command.as_str() {
        "stapel.status" => vec!["status".to_string(), doc.key.clone()],
        "stapel.next" => vec!["next".to_string()],
        "stapel.page" => vec!["page".to_string(), doc.key.clone()],
        "stapel.ok" => vec!["ok".to_string(), doc.key.clone()],
        _ => return None,
    };
    Some((args, doc))
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
