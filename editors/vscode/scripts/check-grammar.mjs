// Grammar smoke check: tokenize representative Josh with the same
// vscode-textmate/Oniguruma stack VSCode uses, and assert the scopes that
// matter land on the right text. Runs in `npm run package` so a malformed
// pattern fails the build instead of silently mis-coloring.
import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);
const oniguruma = require("vscode-oniguruma");
const vsctm = require("vscode-textmate");

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const wasmBin = await readFile(
  join(root, "node_modules/vscode-oniguruma/release/onig.wasm"),
);
await oniguruma.loadWASM(wasmBin.buffer.slice(
  wasmBin.byteOffset,
  wasmBin.byteOffset + wasmBin.byteLength,
));

const grammar = await readFile(join(root, "syntaxes/josh.tmLanguage.json"), "utf8");
const registry = new vsctm.Registry({
  onigLib: Promise.resolve(oniguruma),
  loadGrammar: (scopeName) =>
    scopeName === "source.josh" ? JSON.parse(grammar) : undefined,
});
const grammarInstance = await registry.loadGrammar("source.josh");
if (!grammarInstance) throw new Error("grammar did not load");

// Collect "text → scopes" for tokens of one line.
function tokensOf(line) {
  const result = grammarInstance.tokenizeLine(line);
  return result.tokens.map((token) => ({
    startIndex: token.startIndex,
    endIndex: token.endIndex,
    scopes: token.scopes,
  }));
}

const failures = [];
// Assert every character of `text`'s first occurrence in `line` is covered
// by tokens carrying `scope` (TextMate may merge or split token segments,
// so exact-token matching is the wrong shape for these checks).
function check(label, line, expected) {
  const tokens = tokensOf(line);
  for (const [text, scope] of expected) {
    const start = line.indexOf(text);
    if (start < 0) {
      failures.push(`${label}: ${JSON.stringify(text)} not in line`);
      continue;
    }
    let cursor = start;
    for (const token of tokens) {
      if (token.startIndex <= cursor && token.endIndex > cursor) {
        cursor = token.scopes.includes(scope) ? token.endIndex : cursor;
      }
    }
    if (cursor < start + text.length) {
      failures.push(
        `${label}: ${JSON.stringify(text)} uncovered by ${scope} at ${cursor - start}\n` +
          `  got: ${tokens.map((t) => JSON.stringify(line.slice(t.startIndex, t.endIndex)) + " => " + t.scopes.join(",")).join(" | ")}`,
      );
    }
  }
}

check("keywords and strings", "let x = 'raw' + \"done\"", [
  ["let", "keyword.control.josh"],
  ["raw", "string.quoted.josh"],
  ["done", "string.quoted.josh"],
]);

check("raw strings stay literal", "echo r'raw $raw' 'cooked $n \\n'", [
  ["r'raw $raw'", "string.quoted.raw.single.josh"],
  ["'cooked $n \\n'", "string.quoted.josh"],
  ["$n", "variable.other.josh"],
  ["\\n", "constant.character.escape.josh"],
]);

check("interpolation and capture nest", "echo \"hi ${name.first} $(date | cut)\"", [
  ["${name.first}", "meta.embedded.expression.josh"],
  ["$(date | cut)", "meta.embedded.command.josh"],
  ["|", "keyword.operator.pipe.josh"],
]);

check("nested braces inside interpolation", "echo \"${ {a: 1}.a }\"", [
  ["${ {a: 1}.a }", "meta.embedded.expression.josh"],
  ["{a: 1}", "meta.group.braces.josh"],
  ["1", "constant.numeric.josh"],
]);

check("operators and keywords", "try { x === y && z >= 3 } catch { throw err }", [
  ["try", "keyword.control.josh"],
  ["catch", "keyword.control.josh"],
  ["throw", "keyword.control.josh"],
  ["===", "keyword.operator.comparison.josh"],
  ["&&", "keyword.operator.logical.josh"],
  [">=", "keyword.operator.comparison.josh"],
]);

check("redirects and pipes", "cat < in.txt 2>&1 | tee >> out.log", [
  ["<", "keyword.operator.redirect.josh"],
  ["2>&1", "keyword.operator.redirect.josh"],
  [">>", "keyword.operator.redirect.josh"],
  ["|", "keyword.operator.pipe.josh"],
]);

check("unsupported tokens are invalid", "let π = 1 == 2 & x", [
  ["==", "invalid.illegal.operator.josh"],
  ["&", "invalid.illegal.operator.josh"],
]);

check("excluded commands at line head are invalid", "import foo.josh", [
  ["import", "invalid.illegal.excluded-command.josh"],
]);

check("excluded words mid-command stay plain", "echo import export", []);

check("comments, variables, numbers", "echo $path 3.5 # trailing", [
  ["$path", "variable.other.josh"],
  ["3.5", "constant.numeric.josh"],
  ["# trailing", "comment.line.number-sign.josh"],
]);

check("spaced arithmetic, unspaced words", "ls -la /tmp && b % 7", [
  ["&&", "keyword.operator.logical.josh"],
  ["%", "keyword.operator.arithmetic.josh"],
]);

check("arrow and assignment", "let f = (x) => x + 1", [
  ["=>", "keyword.operator.arrow.josh"],
  ["=", "keyword.operator.assignment.josh"],
]);

if (failures.length > 0) {
  for (const failure of failures) console.error(`FAIL ${failure}`);
  process.exit(1);
}
console.log("check-grammar: ok");
