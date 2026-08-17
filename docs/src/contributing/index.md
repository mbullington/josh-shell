# Contributor guide

Contributors work across two source trees and one manual:

- [`josh-shell`](https://github.com/mbullington/josh-shell) (this repository): Josh Rust source, tests, and `docs/`.
- [`agent-terminal`](https://github.com/mbullington/agent-terminal): separate PTY/VT automation source. Its `agent-terminal-cli` crate provides the `agent-terminal` binary.

Do not edit Rust while performing documentation-only work. Before promoting a capability, exercise its closest real entry point. A build or source review is not runtime evidence.
