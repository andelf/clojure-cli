# clojure-cli

Clojure CLI for Windows and all the other platforms.

## Usage

```console
> cargo install --git https://github.com/andelf/clojure-cli.git

> clojure

> clojure -Sverbose -Ttools
```

## Rationale

[clj on Windows](https://github.com/clojure/tools.deps.alpha/wiki/clj-on-Windows) is defined as PowerShell functions.

- Cannot be used as binary commands (use with yarn/npm/gulp/etc.)
- Not compatible with pwsh 7.0+ (repl quits at once)
- Cannot be used in cmd.exe easily (require call to powershell.exe)
- Difficult to handle escape sequences in cmd/powershell
