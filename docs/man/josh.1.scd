josh(1)

# NAME

josh - JavaScript Object Shell

# SYNOPSIS

*josh* [--no-config]

*josh* [--no-config] -c <source>

*josh* [--no-config] <script.josh>

*josh* -h | --help

*josh* -V | --version

# DESCRIPTION

Josh is a Unix-first shell built around structured data. Command pipelines carry
bytes on Unix pipes by default; explicit stages (_text_, _json_, _lines_,
_jsonl_, _chunks_, _map_, _filter_, _take_, _takeLast_, _first_, _collect_)
convert between bytes and typed values. Values include booleans, Int, Float,
String, Bytes, Array, Object, Function, Error, and Status, with JavaScript-flavored
prototypes and namespaces (_Object_, _String_, _Number_, _Boolean_, _Array_,
_Function_, _Date_, _File_, _Math_).

The scripting language is a strict JavaScript-shaped subset: _let_ bindings,
_fn_ declarations and arrow closures, destructuring, _if_/_while_/_loop_/
_try_ control flow, structured _throw_/_catch_ errors with typed kinds, range
slices (_a[b..c]_) on arrays and strings, and _source f.josh_ for explicitly
including other scripts. There is deliberately no _for_ loop, no _==_ coercing
equality, no _++/--_, and no background jobs, job control, or module system;
_import_/_export_ are rejected.

For the full language reference see the mdBook manual in _docs/_ of the
source tree (_mdbook serve docs_) or the rendered copy committed at
_docs/book/index.html_.

# OPTIONS

*--no-config*
	Skip the _env.josh_ and interactive _init.josh_ startup files.

*-h*, *--help*
	Show help and exit 0 without loading config.

*-V*, *--version*
	Show the version and exit 0 without loading config.

# INVOCATION

*josh*
	Run startup files and start the interactive REPL (Reedline editor, token
	highlighting, prefix hints, completion).

*josh -c SOURCE*
	Run startup environment, then evaluate one source argument and exit.

*josh PATH*
	Run startup environment, then evaluate one UTF-8 script file and exit.

Neither _-c_ nor script mode accepts trailing positional arguments.

# FILES

*$XDG_CONFIG_HOME/josh/env.josh*, otherwise *$HOME/.config/josh/env.josh*
	Startup file for every batch, script, and interactive session. Runs in the
	same global lexical scope as the session; may use _source_ for includes.

*<config-root>/josh/init.josh*
	Second startup file, interactive sessions only.

*$HOME/.josh_history*
	Plain-file interactive history (overridable with _JOSH_HISTORY_; falls back
	to _.josh_history_ in the current directory when HOME is absent). Treat it
	as sensitive: commands may contain paths or literal data.

# ENVIRONMENT

*XDG_CONFIG_HOME*
	Selects the config root (see FILES). Requires _HOME_ otherwise.

*HOME*
	Fallback config root plus tilde and history anchoring.

*PATH*
	Command resolution. _env.PATH_ exposes array and scalar views and may be
	edited from Josh code; edits persist for the session and its children.

*JOSH_HISTORY*
	Overrides the history file path.

# EXIT STATUS

*0*
	Success.

*1*
	Parse, runtime, or process error in requested source (including _-c_ and
	script modes and failed command pipelines).

*2*
	Usage/read/startup-file errors. In batch and script modes a startup-file
	error exits 2 before requested source runs; an _exit N_ attempted in a
	startup file also exits 2.

*exit N* inside requested source returns _N_.

# SEE ALSO

The Josh manual: _docs/book/index.html_ in the source tree, and the capability
status matrix at _docs/src/status/matrix.md_, which is the availability
authority.

# BUGS

Known gaps and deliberate exclusions are tracked in the manual's status matrix
and roadmap rather than in bug lists.
