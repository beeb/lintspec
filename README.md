# 🔎 lintspec

![lintspec screenshot](./screenshot.png)

<div align="center">
  <a href="https://github.com/beeb/lintspec"><img
      alt="github"
      src="https://img.shields.io/badge/github-beeb%2Flintspec-228b22?style=flat&logo=github"
      height="20"
  /></a>
  <a href="https://crates.io/crates/lintspec"><img
      alt="crates.io"
      src="https://img.shields.io/crates/v/lintspec.svg?style=flat&color=e37602&logo=rust"
      height="20"
  /></a>
  <a href="https://docs.rs/lintspec/latest/lintspec/"><img
      alt="docs.rs"
      src="https://img.shields.io/badge/docs.rs-lintspec-3b74d1?style=flat&labelColor=555555&logo=docs.rs"
      height="20"
  /></a>
      <a href="https://docs.rs/lintspec/latest/lintspec/"><img
      alt="docs.rs"
      src="https://img.shields.io/badge/MSRV-1.80.0-b83fbf?style=flat&labelColor=555555&logo=docs.rs"
      height="20"
  /></a>
</div>

Lintspec is a command-line utility (linter) that checks the completeness and validity of
[NatSpec](https://docs.soliditylang.org/en/latest/natspec-format.html) doc-comments in Solidity code. It is focused on
speed and ergonomics. By default, lintspec will respect gitignore rules when looking for Solidity source files.

Dual-licensed under MIT or Apache 2.0.

## Installation

#### Via `cargo`

```bash
cargo install lintspec
```

#### Via [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall)

```bash
cargo binstall lintspec
```

#### Via `nix` (coming soon)

```bash
nix-env -iA nixpkgs.lintspec
# or
nix-shell -p lintspec
# or
nix run nixpkgs#lintspec
```

#### Pre-built binaries and install script

Head over to the [releases page](https://github.com/beeb/lintspec/releases)!

## Usage

```
Usage: lintspec [OPTIONS] [PATH]...

Arguments:
  [PATH]...  One or more paths to files and folders to analyze

Options:
  -e, --exclude <EXCLUDE>            Path to a file or folder to exclude (can be used more than once)
  -o, --out <OUT>                    Write output to a file instead of stderr
      --inheritdoc                   Enforce that all public and external items have `@inheritdoc`
      --constructor                  Enforce that constructors have NatSpec
      --enum-params                  Enforce that enums have `@param` for each variant
      --json                         Output diagnostics in JSON format
      --compact                      Compact output
  -h, --help                         Print help (see more with '--help')
  -V, --version                      Print version
```

## Configuration

The tool can be configured with a `.lintspec.toml` file ([see example](./.lintspec.toml)), environment variables
([see example](./.env.example)) or CLI arguments (see above). CLI arguments take precedence over environment variables,
which take precedence over the config file.

## Usage in GitHub Actions

You can check your code in CI with the lintspec GitHub Action. Any `.lintspec.toml` or `.nsignore` file in
repository's root will be used to configure the execution.

The action generates
[annotations](https://docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/workflow-commands-for-github-actions#setting-a-warning-message)
that are displayed in the source files when viewed (e.g. in a PR's "Files" tab).

### Options

The following options are available for the action (all are optional):

| Input | Default Value | Description | Example |
|---|---|---|---|
| `working-directory` | `"./"` | Working directory path | `"./src"` |
| `paths` | `"[]"` | Paths to scan, relative to the working directory, in square brackets and separated by commas. | `"[path/to/file.sol,test/test.sol]"` |
| `exclude` | `"[]"` | Paths to exclude, relative to the working directory, in square brackets and separated by commas | `"[path/to/exclude,other/path.sol]"` | 
| `extra-args` | | Extra arguments passed to the `lintspec` command | `"--constructor=true"` |
| `version` | `"latest"` | Version of lintspec to use. For enhanced security, you can pin this to a fixed version | `"0.1.5"` |
| `fail-on-problem` | `"true"` | Whether the action should fail when NatSpec problems have been found. Disabling this only creates annotations for found problems, but succeeds | `"false"` |

### Example Workflow

```yaml
name: Lintspec

on:
  pull_request:

jobs:
  lintspec:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: beeb/lintspec@main
        with:
          version: "latest"
          fail-on-problem: "true"
```

## Credits

This tool walks in the footsteps of [natspec-smells](https://github.com/defi-wonderland/natspec-smells), thanks to
them for inspiring this project!

## Comparison with natspec-smells

### Benchmark

On an AMD Ryzen 9 7950X processor with 64GB of RAM, linting the
[Uniswap/v4-core](https://github.com/Uniswap/v4-core) `src` folder on WSL2 (Ubuntu), lintspec is about 214x faster, or
0.46% of the execution time:

```
Benchmark 1: npx @defi-wonderland/natspec-smells --include "src/**/*.sol"
  Time (mean ± σ):     12.484 s ±  0.157 s    [User: 13.581 s, System: 0.594 s]
  Range (min … max):   12.288 s … 12.817 s    10 runs

  Warning: Ignoring non-zero exit code.

Benchmark 2: lintspec src --compact=true
  Time (mean ± σ):      58.2 ms ±   1.3 ms    [User: 264.0 ms, System: 67.9 ms]
  Range (min … max):    53.9 ms …  61.3 ms    50 runs

  Warning: Ignoring non-zero exit code.

Summary
  lintspec src --compact=true ran
  214.55 ± 5.61 times faster than npx @defi-wonderland/natspec-smells --include "src/**/*.sol"
```

### Features

| Feature                         | `lintspec` | `natspec-smells` |
|---------------------------------|------------|------------------|
| Identify missing NatSpec        | ✅          | ✅                |
| Identify duplicate NatSpec      | ✅          | ✅                |
| Include files/folders           | ✅          | ✅                |
| Exclude files/folders           | ✅          | ✅                |
| Enforce usage of `@inheritdoc`  | ✅          | ✅                |
| Enforce NatSpec on constructors | ✅          | ✅                |
| Configure via config file       | ✅          | ✅                |
| Enforce NatSpec on enums        | ✅          | ❌                |
| Respects gitignore files        | ✅          | ❌                |
| JSON output                     | ✅          | ❌                |
| Pretty output with code excerpt | ✅          | ❌                |
| Output to file                  | ✅          | ❌                |
| Configure via env variables     | ✅          | ❌                |
| Multithreaded                   | ✅          | ❌                |
| No pre-requisites (npm)         | ✅          | ❌                |
