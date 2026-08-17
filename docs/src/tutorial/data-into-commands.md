# Interpolation, splicing, and capture

A command word can combine literals, quotes, variables, captures, and an evaluated expression. Josh concatenates parts into one argv entry; it never performs implicit word splitting.

<p class="example-label"><strong>Runnable example</strong></p>

```console
josh> who = "Josh"
Josh
josh> printf 'hello %s\n' $who
hello Josh
josh> printf '%s\n' "user=${who}"
user=Josh
```

An array expands to several argv entries only when a sole unquoted `$variable` forms the entire word. Inside double quotes, array elements join with spaces. As part of another unquoted word, an array is a type error.

<p class="example-label"><strong>Runnable example</strong></p>

```console
josh> names = ["Ada", "Lin"]
[Ada, Lin]
josh> printf '[%s]\n' $names
[Ada]
[Lin]
josh> printf '%s\n' "names=$names"
names=Ada Lin
```

Capture runs a pipeline synchronously. It removes every terminal LF and an immediately preceding CR, returns a string for valid UTF-8, and otherwise preserves bytes. It never parses JSON automatically.

<p class="example-label"><strong>Runnable example</strong></p>

```console
josh> loud = $(printf hello | tr a-z A-Z)
HELLO
josh> printf '[%s]\n' $loud
[HELLO]
```
