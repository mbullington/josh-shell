# Control flow and jobs

<div class="status-coverage">

**Status coverage:** [J-RUN-006](../status/matrix.md#J-RUN-006) — **Implemented**; [J-EXPR-003](../status/matrix.md#J-EXPR-003) — **Implemented**; [J-CF-001](../status/matrix.md#J-CF-001) — **Implemented**; [J-JOBS-001](../status/matrix.md#J-JOBS-001) — **Planned**; [J-BG-001](../status/matrix.md#J-BG-001) — **Unresolved**. See [status conventions](../welcome/status-conventions.md).

</div>

<a id="J-RUN-006"></a>
## If statements <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: parser and evaluator tests for expression/command conditions, else-if, and `else`.

A parenthesized condition is an expression and uses Josh truthiness. An unparenthesized condition is a command pipeline ending before an unquoted standalone `{`. A completed nonzero command condition is false. Planning, interpolation, spawn, stream, and type errors propagate; they are not false.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
if (3 > 2) { printf 'yes\n' } else { printf 'no\n' }
if grep -q needle file.txt { printf 'found\n' }
```

<a id="J-EXPR-003"></a>
## If and try as expressions <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in the current development snapshot. Evidence: parser tests for expression-position `if`/`try` and runtime tests for produced values.

`if` and `try` produce values wherever an expression is legal: assignment right-hand sides, call arguments, captures, and pipeline sources. The `else` branch is required in expression position, and `try` still requires its `catch` block. As statements they behave exactly as before.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
size_label = if (total > 1000) { "large" } else { "small" }
recovered = try { read_config() } catch (e) { defaults }
```

<a id="J-CF-001"></a>
## Non-job control flow <span class="status status--implemented" aria-label="Status: Implemented">Implemented</span>

**Availability:** Available in Josh 0.1.0. Evidence: typed-unwinding, process-error, status, chain, and cross-product tests.

| Form | Behavior |
|---|---|
| `while condition { ... }` | Re-evaluate an expression or command condition; return the last completed body value or Null |
| `loop { ... }` | Repeat until control leaves the loop |
| `break` | Leave the nearest loop |
| `continue` | Start the nearest loop's next iteration |
| `return [value]` | Leave the current function; omitted value is Null |
| `throw value` | Raise any Value |
| `try { ... } catch (pattern) { ... }` | Catch thrown values and evaluator/process errors into a binding pattern |

Every block gets a child lexical frame. Assignment updates the nearest existing binding; a new name is created in the current frame. `catch` does not consume `return`, `break`, `continue`, or `exit`. Parse errors occur before evaluation and cannot be caught by a `try` inside the invalid source.

A completed command failure normally becomes an Error with `kind`, `message`, and Status-valued `status`. `status pipeline` returns Status instead of throwing only for completed outcomes. Planning, interpolation, spawn, decode, and type errors still propagate.

| Status member | Meaning |
|---|---|
| `.success` | True only when every ordered external stage outcome succeeded |
| `.code` | Last stage's exit code; a signal maps to `128 + signal`, and no outcome maps to 0 |
| `.outcomes` | Array of `{stage, command, code, signal, success}` in stage order |

Command-mode `&&` and `||` short-circuit on the completed Status: `&&` runs its right pipeline after success, and `||` runs it after failure. Entering a command chain handles completed nonzero status, including its final branch; lookup, interpolation, spawn, and type errors are not converted to chain booleans. Expression-mode `&&` and `||` instead short-circuit on truthiness and return one operand.

Josh has no `for` production. At statement head, `for` remains an ordinary command word and may resolve through PATH.

<p class="example-label example-label--implemented"><strong>Runnable example · Implemented</strong></p>

```josh
fn count() {
  n = 0
  loop {
    n += 1
    if (n === 2) { continue }
    if (n === 4) { break }
  }
  return n
}
try { sh -c 'exit 7' } catch (problem) { printf '%s\n' (problem.status.code) }
```

<a id="J-JOBS-001"></a>
## Background jobs <span class="status status--planned" aria-label="Status: Planned">Planned</span>

**Availability:** Excluded from the current implementation. Tracking: [Planned work](../roadmap/planned.md#j-jobs-001).

`&`, `jobs`, `fg`, and `bg` are rejected. Background execution, a job table, terminal process-group transfer, and Job values are not partially implemented. The syntax and value shape for assigning a background operation remain [Unresolved](../roadmap/unresolved.md#J-BG-001).
