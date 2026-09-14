# Dependency patches

Josh uses [patch-crate 0.1.13](https://docs.rs/patch-crate/0.1.13/patch_crate/) to apply `reedline+0.49.0.patch`. The patch accounts for the `...> ` continuation indicator when mapping a screen click to a buffer position. It includes regression tests for cursor splits, wrapping, and a scrolled continuation line.

## Prepare a checkout

From the repository root:

```sh
cargo install patch-crate --version 0.1.13 --locked
cargo patch-crate
cargo build --locked --workspace
```

The tool needs Git. Its Linux build also needs native development dependencies, including OpenSSL headers and pkg-config.

`cargo patch-crate` recreates missing sources under `target/patch/`, even when Cargo's path override is already present. Run it after `cargo clean`. CI must run it before Cargo build, test, or lint commands.

After pulling changes to a patch, run `cargo patch-crate --force`. This deletes the generated sources and reapplies the checked-in patches. Save any edits in `target/patch/` first. An ordinary invocation leaves existing sources unchanged.

## Update the Reedline patch

Edit `target/patch/reedline-0.49.0/src/`, then generate and inspect the diff:

```sh
cargo patch-crate reedline
git diff -- patches/reedline+0.49.0.patch
cargo patch-crate --force
cargo test --manifest-path target/patch/reedline-0.49.0/Cargo.toml --lib
cargo test -p josh-cli --test cli
```

Patch generation uses Git to create a temporary baseline commit, so configure a Git author identity. Preserve the crate's CRLF source line endings to avoid whole-file diffs. Check in the patch, not the generated crate.

Keep the exact Reedline version, patch filename, workspace exclusion, and Cargo path override in sync. Once an upstream release contains the fix, remove the override and patch setup and rerun the PTY tests before upgrading.
