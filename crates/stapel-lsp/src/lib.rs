//! LSP server for Zed: section actions and diagnostics on the ticket documents.

use lsp_server::{Connection, ErrorCode, Message, Response};
use lsp_types::{
    CodeActionProviderCapability, ExecuteCommandOptions, InitializeParams, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind,
};

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
    while let Ok(msg) = connection.receiver.recv() {
        match msg {
            Message::Request(req) => {
                if shutdown {
                    reply_error(connection, req.id, ErrorCode::InvalidRequest, "shutting down");
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
                if note.method == "exit" {
                    return if shutdown { 0 } else { 1 };
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
