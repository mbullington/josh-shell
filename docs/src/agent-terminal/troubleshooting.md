# agent-terminal troubleshooting

<div class="status-coverage">

**Status coverage:** [AT-BUILD-001](../status/matrix.md#AT-BUILD-001) — **Implemented**; [AT-LIFE-001](../status/matrix.md#AT-LIFE-001) — **Implemented**; [AT-WAIT-001](../status/matrix.md#AT-WAIT-001) — **Implemented**. See [status conventions](../welcome/status-conventions.md).

</div>

- **Zig version check times out:** let the pinned Nix fetch complete, then rerun `test "$(nix develop -c zig version)" = 0.16.0`. Do not use system Zig or replace Ghostty.
- **Ghostty revision mismatch:** run `git submodule update --init --recursive` and verify the exact full SHA. Do not move the pin without reviewing bindings and schemas.
- **No session or ambiguous session:** run `agent-terminal list`; pass a full ID or unique prefix of at least eight hex characters.
- **Runtime path rejected:** remove symlinks, fix ownership, use 0700 directories, and keep the path short enough for `sockaddr_un`.
- **Text wait times out:** matching is case-sensitive and limited to current Ghostty-formatted visible text. Inspect the last revision metadata and a JSON snapshot.
- **Stable wait never completes:** output is still advancing revision. Bound the child or increase the quiet duration and timeout.
- **Snapshot after a successful stable wait shows pre-input text:** known wait-admission defect; do not wait immediately after submitting input. See the [errata](errata.md) for the settle and polling rules.
- **Exited child remains listed:** final terminal state is retained by design. Run `close` to remove the session.
- **Screenshot shows U+FFFD:** the grapheme is absent from the pinned JetBrains Mono faces. Host font fallback is disabled for deterministic output.
- **Repeated screenshots differ:** wait for a stable revision, keep grid, palette, default colors, and cursor state fixed, and compare commands built from the same lockfile and font assets.
- **Close cannot contain a descendant:** lifecycle cleanup is not a sandbox; inspect escaped process groups with host tools.

Every test script should use a unique temporary runtime directory and an EXIT/INT/TERM trap. After testing, require an empty `list --json`, no `control.sock`, and no new daemon or child process.
