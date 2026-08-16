# Status conventions

<div class="status-coverage">

**Status coverage:** This page makes no product-availability claims. It defines how to read availability claims in this manual.

</div>

Status applies to an atomic capability, not to a whole page. A mixed page lists each capability separately.

- **Implemented** means runnable in the named version and backed by executable evidence.
- **Specified** means the contract is accepted but not available in the current verified build.
- **Planned** records direction. Syntax and behavior may change.
- **Unresolved** records an open question. No syntax or behavior is promised.

A feature cannot carry two statuses. Partial delivery becomes multiple capability rows. For example, semantic snapshots and PNG rendering have separate Implemented rows.

## Reading examples

Every behavioral fence has a nearby text label. Only **Runnable example · Implemented** asks you to execute Josh syntax. **Specified syntax · Not implemented**, **Illustrative syntax · Planned · Not runnable**, and **Design alternative · Unresolved · Not runnable** prevent copied snippets from becoming accidental promises. **Host command** marks commands for a Unix shell rather than Josh language input.

The [capability matrix](../status/matrix.md) is authoritative. Its stable IDs link to detailed blocks, identify exact exclusions, and name evidence or a specification. Roadmap pages are views of those IDs, not independent status lists.

## Promotion rule

Source code, an architecture proposal, or successful compilation alone does not prove a capability Implemented. Promotion requires the closest executable evidence: a real CLI invocation, parser/runtime test, PTY scenario, or deterministic artifact inspection. If a blocker prevents that evidence, the manual preserves the contract as Specified and reports the blocker.
