# Install stapel

`stapel init` writes the CI workflow `.github/workflows/stapel-check.yml`. The workflow installs `stapel`
with `cargo install --git https://github.com/kyyyyf/stapel --rev <commit> --locked stapel-cli`, where
`<commit>` is the git commit the running binary was built from. So the binary must know its build commit:
`stapel --version` prints it, and `stapel init` writes no workflow without it.

Install from a clean checkout of this repository with this command:

```sh
STAPEL_BUILD_COMMIT="$(git rev-parse HEAD)$(test -z "$(git status --porcelain)" || echo -dirty)" cargo install --path crates/stapel-cli --locked
```

- `git rev-parse HEAD` gives the full commit id (64 hex digits in a SHA-256 repository).
- `git status --porcelain` prints a line for any change, untracked files included. The command then adds
  `-dirty`, and `stapel init` writes no workflow from a dirty build, because CI cannot fetch that tree.
- Commit and push the change before you install, so that the commit is on `github.com/kyyyyf/stapel`.
