# Install and build

<div class="status-coverage">

**Status coverage:** [J-CLI-001](../status/matrix.md#J-CLI-001) — **Implemented**; [AT-BUILD-001](../status/matrix.md#AT-BUILD-001) — **Implemented**. See [status conventions](status-conventions.md).

</div>

Josh is a six-crate Rust workspace with one `josh` binary. The repository lockfile pins Rust dependencies.

<a id="J-CLI-001-install"></a>
## Build Josh

**Host command**
```sh
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
./target/debug/josh --version
```

The verified binary reports `josh 0.1.0`. Use `cargo install --path crates/josh-cli --locked` if you want it on Cargo's binary path.

## Platform boundary

Josh is Unix-first. Byte-preserving command arguments and signal behavior have Unix-specific paths. The parser can compile elsewhere, but this manual does not claim complete shell behavior on Windows.

agent-terminal has a separate pinned Nix/Zig build. Follow [Install agent-terminal](../agent-terminal/install.md); do not substitute another VT parser or Zig version when provisioning fails.
