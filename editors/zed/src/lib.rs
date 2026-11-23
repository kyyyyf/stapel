use zed_extension_api as zed;

struct StapelExtension;

impl zed::Extension for StapelExtension {
    fn new() -> Self {
        StapelExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let found = worktree.which("stapel");
        let command = found.ok_or_else(|| "stapel not found on PATH".to_string())?;
        Ok(zed::Command {
            command,
            args: vec!["lsp".to_string()],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(StapelExtension);
