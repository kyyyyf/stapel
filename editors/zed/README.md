# stapel for Zed

This extension only starts `stapel lsp` for Markdown. It finds `stapel` on `PATH`; when there is none, Zed shows
`stapel not found on PATH`. The extension is outside the cargo workspace of the repository.

## Install as a dev extension

1. Put `stapel` on `PATH` (`cargo install --path crates/stapel-cli`).
2. Install Rust with `rustup`; Zed builds the extension to wasm.
3. In Zed run `zed: install dev extension` and choose this folder, `editors/zed`.
4. Open a ticket of a stapel repository and a `ticket.md` or `page.md`; the code actions show the buttons.

The wasm build and the lock file stay out of git (`editors/zed/target`, `editors/zed/Cargo.lock`).
