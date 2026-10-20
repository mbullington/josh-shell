# Verified 0.1.0 scope

Josh 0.1.0 now verifies the six-crate workspace end to end: one lossless parser across tolerant interaction and strict execution; lexical data/functions and UFCS; typed non-job control flow; planned external execution; explicit byte/value streams with stable cardinality and bounded cancellation; redirections and sorted globs; dedicated startup scripts; and Reedline interaction.


The canonical Nix development shell completed format, warnings-denied Clippy, locked tests, locked builds, smoke, and the complete scenario with matching Rust tooling. Remaining limits are deliberate: raw byte capture is whole-buffer; unsupported glyphs render as U+FFFD; lifecycle cleanup is not hostile-process containment; and Jobs, modules, and general source loading are unavailable.
