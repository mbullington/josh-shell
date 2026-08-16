# Install agent-terminal

<div class="status-coverage">

**Status coverage:** [AT-BUILD-001](../status/matrix.md#AT-BUILD-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="AT-BUILD-001"></a>
## Pinned static build <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in agent-terminal 0.1.0. Evidence: exact Ghostty pin and Zig 0.16.0 checks, linked-library identity tests, and `otool`/`nm` static-link inspection.

The repository vendors Ghostty as a git submodule. `build.rs` refuses a missing or mismatched revision, a dirty source tree, a mismatched tree digest, and a different Zig version. The static archive is keyed by the verified tree digest and build profile. Checked-in narrow bindings keep bindgen out of normal builds.

**Host command**
```sh
cd /Users/mbullington/Projects/agent-terminal
git submodule update --init --recursive
test "$(nix develop -c zig version)" = 0.16.0
nix develop -c cargo build --locked
nix develop -c cargo test --locked
scripts/smoke.sh target/debug/agent-terminal
otool -L target/debug/agent-terminal
nm target/debug/agent-terminal | grep ghostty_terminal_new
```

On Linux, use `ldd` and `nm` equivalents. A successful build must not dynamically depend on `libghostty-vt` and must contain `ghostty_terminal_new`.

## Provisioning note

The first `nix develop` can spend several minutes fetching the pinned Rust and Zig toolchains. On 2026-08-15, the canonical development shell completed after its cache was populated and ran the static build, tests, smoke checks, link inspection, and Josh scenario. Do not substitute another Zig version or terminal parser.
