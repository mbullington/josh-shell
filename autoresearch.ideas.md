# Untried hypotheses — Josh interpreter, regex benchmark

Ordered guesses by expected impact (profile first; do not trust this order):

1. **String `.length` UTF-16 recomputation.** `eval_member` recomputes
   `utf16_length` on every access — `re_build_result`'s units-map loop makes
   the astral/capture cases O(n²). Cache the unit count alongside the string
   (Value repr change or memo table), or make split("") carry cached lengths.
2. **Frame/binding lookup.** Every identifier walks a chain of HashMap frames.
   Hot featherweight wins: resolve with a generational symbol table, or move
   top-level globals into one flat map, or add a small per-frame inline cache.
3. **Value clone/alloc churn in re_run.** caps arrays are rebuilt via
   slice+push per SAVE crossing; ArrayValue::RevisionLock + to_vec per slice.
   Consider Rc<[Value]> slicing or reserve/push paths.
4. **Prototype dispatch cost.** `member_fn`/`apply_native` and `dir()` bootstrap
   calls run per `.length`/`.push`/`.split` call; cache the resolved native
   per member name per type (dispatch table), or pre-solve known names.
5. **Function call overhead.** `FunctionCall(frame(...))` allocs per call;
   regex internals are tiny functions in very hot loops. Frame pooling or
   avoiding child frames for non-closure prototypes is suspect.
6. **`chars[sp]` indexing path.** `index_value` clones via to_index etc; hot in
   the VM loop. Fast-path integer array index, skip bounds re-scan.
7. **Object member access (`cls.singles[ch]`, `inst[0]`)**: HashMap lookups per
   regex char; representation-level wins would be big (ordered flatmaps?).
8. **Parser is near-zero here** (bench spends on interpreter), but regex.josh
   top-level source() runs per benchmark invocation — loader-only.

Dump per-case evidence before choosing: literal-long vs astral-group vs
date-extract stress different paths (scan vs captures vs units map).
