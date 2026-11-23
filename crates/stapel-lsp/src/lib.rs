//! LSP server for Zed: section actions and diagnostics on the ticket documents.

use lsp_server::{Connection, ErrorCode, Message, Response};
use lsp_types::{
    CodeAction, CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability, Command,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    ExecuteCommandOptions, InitializeParams, ServerCapabilities, TextDocumentSyncCapability,
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
