# Change grammar or runtime behavior

## Grammar changes

Start with the source distinction the grammar must preserve. Update token kinds/modes, AST variants, parser production, recovery ownership, completeness, diagnostics, and statement-head goldens together. Verify token spans still partition Unicode source and strict policy still returns the tolerant parse's storage.

Add a test only for a concrete regression: ambiguity, delimiter ownership, EOF classification, stable diagnostic, or lossless span invariant. Do not add a second tokenizer or runtime-dependent parse fallback.

## Runtime and pipeline changes

Define value shape, ownership, and process boundary before execution logic. Planning must reject unsupported stage classes and resolve every external executable before spawning. Preserve ordered outcomes and terminate/reap partial starts.

For structured streams, define each byte/value transition and capture cardinality explicitly; do not fake a transformer by JSON-detecting behind unrelated syntax.

## Documentation co-change points

A behavior change updates the linked facts in one pass:

1. the affected manual section and its prose;
2. adjacent examples, re-run against the build;
3. roadmap views when the capability appears there;
4. SUMMARY placement only when reader navigation changes.

Run the checker after any heading, anchor, link, or code-fence edit.
