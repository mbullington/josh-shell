# Disambiguation catalog

<a id="J-PARSE-003"></a>
## Statement-head outcomes

| Source | Committed interpretation | Parse result |
|---|---|---|
| `ls -la` | Command mode | Command `ls`, argv `-la` |
| `x = 5` | Assignment then expression RHS | Assignment of Int 5 |
| `x(1, 2)` | Adjacent `(` selects expression | Call expression |
| `x (1 + 2)` | Space keeps command head; `(` inserts expression argument | Command with one evaluated argument |
| `items.filter(f)` | Adjacent member/call selects expression | Call whose callee is Member |
| `x - 1` | No adjacent expression continuation | Command `x`, args `-`, `1` |
| `let y = ls` | `let` requires expression RHS | Identifier `ls`; evaluation hints `$(ls)` if undefined |
| `x=>x` | Arrow at expression head | Arrow expression |
| `x => x` | Arrow ignores trivia for head classification | Arrow expression |
| `for foo` | `for` is not reserved | Command `for`, arg `foo` |
| `if (n > 3) { ... }` | Parenthesized expression condition | `IfCondition::Expr` |
| `if grep -q foo file { ... }` | Command condition until unquoted standalone `{` | `IfCondition::Command` |

A quoted `'{'` or escaped `\{` remains a command-condition argument. Adjacency is significant for calls/member/index but not for arrows. `foo.bar` therefore favors member expression parsing; use `./foo.bar` for a dotted executable path.
