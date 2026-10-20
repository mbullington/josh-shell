# Install and build

Josh is a six-crate Rust workspace with one `josh` binary. The repository lockfile pins Rust dependencies.

To try Josh without installing anything, open [Try Josh in your browser](https://mbullington.github.io/josh-shell/): it boots the latest `main` build in a browser-tab VM via [trynix](https://trynix.dev). With Nix, `nix run github:mbullington/josh-shell` builds and runs the same flake (`nix build` also produces `josh-lsp`, the man page, and `share/`).

<a id="J-CLI-001-install"></a>
## Build Josh

**Host command**
```sh
cargo install patch-crate --version 0.1.13 --locked
cargo patch-crate
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
./target/debug/josh --version
```

Run `cargo patch-crate` from the repository root after a fresh checkout or `cargo clean`. Use `cargo patch-crate --force` after pulling patch changes; it replaces the generated sources under `target/patch/`. Git and the native build dependencies for `patch-crate` are required, including OpenSSL development headers on Linux.

The verified binary reports `josh 0.1.0`. Use `cargo install --path crates/josh-cli --locked` if you want it on Cargo's binary path.

## Platform boundary

Josh is Unix-first. Byte-preserving command arguments and signal behavior have Unix-specific paths. The parser can compile elsewhere, but this manual does not claim complete shell behavior on Windows.

agent-terminal has a separate pinned Nix/Zig build. Follow [Install agent-terminal](../agent-terminal/install.md); do not substitute another VT parser or Zig version when provisioning fails.
