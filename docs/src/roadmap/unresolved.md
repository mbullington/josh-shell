# Unresolved decisions

<div class="status-coverage">

**Status coverage:** [J-BG-001](../status/matrix.md#J-BG-001) — **Unresolved**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-BG-001"></a>
## Job expression assignment <span class="status status--unresolved" aria-label="Status: Unresolved">Unresolved</span>

**Availability:** Open design question; no behavior is promised. Decide whether a background command is only a statement or can form an affine Job value without making parsing depend on runtime type.

No assignment syntax is accepted. A decision must define ownership, cancellation, status, process-group lifetime, and interaction with error-first execution.

## Remote module trust

**Status: Unresolved.** URL or hosted-library imports need identity, integrity, permission, cache, update, and offline rules. Until then, [modules remain Planned](../language/modules-configuration.md#J-MOD-001) and no URL syntax is promised.
