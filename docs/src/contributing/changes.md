# Change grammar or runtime behavior

<div class="status-coverage">

**Status coverage:** This page makes no product-availability claims. It defines review procedure for status-bearing code changes.

</div>

## Grammar changes

Start with the source distinction the grammar must preserve. Update token kinds/modes, AST variants, parser production, recovery ownership, completeness, diagnostics, and statement-head goldens together. Verify token spans still partition Unicode source and strict policy still returns the tolerant parse's storage.

Add a test only for a concrete regression: ambiguity, delimiter ownership, EOF classification, stable diagnostic, or lossless span invariant. Do not add a second tokenizer or runtime-dependent parse fallback.

## Runtime and pipeline changes

Define value shape, ownership, and process boundary before execution logic. Planning must reject unsupported stage classes and resolve every external executable before spawning. Preserve ordered outcomes and terminate/reap partial starts.

For structured streams, define each byte/value transition and capture cardinality explicitly. Split the capability row when a transition can be delivered or verified independently; do not fake a transformer by JSON-detecting behind unrelated syntax.

## Documentation co-change points

A status transition changes exactly these linked facts:

1. one matrix row, including revision and evidence;
2. one anchored capability block and availability sentence;
3. adjacent example/evidence;
4. roadmap view when the old/new status appears there;
5. affected SUMMARY placement only when reader navigation changes.

Run the checker after any heading, anchor, link, or code-fence edit.
