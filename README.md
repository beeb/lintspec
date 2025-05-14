# 🔎 lintspec

![lintspec screenshot](https://raw.githubusercontent.com/beeb/lintspec/refs/heads/main/screenshot.png)

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
      src="https://img.shields.io/badge/MSRV-1.86.0-b83fbf?style=flat&labelColor=555555&logo=docs.rs"
      height="20"
  /></a>
</div>

Lintspec is a command-line utility (linter) that checks the completeness and validity of
[NatSpec](https://docs.soliditylang.org/en/latest/natspec-format.html) doc-comments in Solidity code. It is focused on
speed and ergonomics. By default, lintspec will respect gitignore rules when looking for Solidity source files.

Dual-licensed under MIT or Apache 2.0.

> Note: the `main` branch can contain unreleased changes. To view the README information for the latest stable release,
> visit [crates.io](https://crates.io/crates/lintspec) or select the latest git tag from the branch/tag dropdown.

## Installation

#### Via `cargo`

```bash
cargo install lintspec
```

#### Via [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall)

```bash
cargo binstall lintspec
```

#### Via `nix`

```bash
nix-env -iA nixpkgs.lintspec
# or
nix-shell -p lintspec
# or
nix run nixpkgs#lintspec
```

#### Pre-built binaries and install script

Head over to the [releases page](https://github.com/beeb/lintspec/releases)!

### Experimental `solar` backend

An experimental (and very fast) parsing backend using [`solar`](https://github.com/paradigmxyz/solar) can be tested
by installing with the `solar` feature flag enabled. This is only possible via `cargo install` at the moment.

```bash
cargo install lintspec -F solar`
```

With this backend, the parsing step is roughly 15x faster than with the default
[`slang`](https://github.com/NomicFoundation/slang) backend. In practice, overall gains of 2-3x can be expected on the
total execution time.
**Note that `solar` only supports Solidity >=0.8.0.**

## Usage

```text
Usage: lintspec [OPTIONS] [PATH]... [COMMAND]

Commands:
  init  Create a `.lintspec.toml` config file with default values
  help  Print this message or the help of the given subcommand(s)

Arguments:
  [PATH]...  One or more paths to files and folders to analyze

Options:
  -e, --exclude <EXCLUDE>        Path to a file or folder to exclude (can be used more than once)
  -o, --out <OUT>                Write output to a file instead of stderr
      --inheritdoc               Enforce that all public and external items have `@inheritdoc`
      --notice-or-dev            Do not distinguish between `@notice` and `@dev` when considering "required" validation rules
      --skip-version-detection   Skip the detection of the Solidity version from pragma statements
      --notice-ignored <TYPE>    Ignore `@notice` for these items (can be used more than once)
      --notice-required <TYPE>   Enforce `@notice` for these items (can be used more than once)
      --notice-forbidden <TYPE>  Forbid `@notice` for these items (can be used more than once)
      --dev-ignored <TYPE>       Ignore `@dev` for these items (can be used more than once)
      --dev-required <TYPE>      Enforce `@dev` for these items (can be used more than once)
      --dev-forbidden <TYPE>     Forbid `@dev` for these items (can be used more than once)
      --param-ignored <TYPE>     Ignore `@param` for these items (can be used more than once)
      --param-required <TYPE>    Enforce `@param` for these items (can be used more than once)
      --param-forbidden <TYPE>   Forbid `@param` for these items (can be used more than once)
      --return-ignored <TYPE>    Ignore `@return` for these items (can be used more than once)
      --return-required <TYPE>   Enforce `@return` for these items (can be used more than once)
      --return-forbidden <TYPE>  Forbid `@return` for these items (can be used more than once)
      --json                     Output diagnostics in JSON format
      --compact                  Compact output
      --sort                     Sort the results by file path
  -h, --help                     Print help (see more with '--help')
  -V, --version                  Print version
```

## Configuration

### Config File

Create a default configuration with the following command:

```bash
lintspec init
```

This will create a `.lintspec.toml` file with the default configuration in the current directory.

### Environment Variables

Environment variables (in capitals, with the `LS_` prefix) can also be used and take precedence over the
configuration file. They use the same names as in the TOML config file and use the `_` character as delimiter for
nested items.

Examples:

- `LS_LINTSPEC_PATHS=[src,test]`
- `LS_LINTSPEC_INHERITDOC=false`
- `LS_LINTSPEC_NOTICE_OR_DEV=true`: if the setting name contains `_`, it is not considered a delimiter
- `LS_OUTPUT_JSON=true`
- `LS_CONSTRUCTOR_NOTICE=required`

### CLI Arguments

Finally, the tool can be customized with command-line arguments, which take precedence over the other two methods.
To see the CLI usage information, run:

```bash
lintspec help
```

## Usage in GitHub Actions

You can check your code in CI with the lintspec GitHub Action. Any `.lintspec.toml` or `.nsignore` file in the
repository's root will be used to configure the execution.

The action generates
[annotations](https://docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/workflow-commands-for-github-actions#setting-a-warning-message)
that are displayed in the source files when viewed (e.g. in a PR's "Files" tab).

### Options

The following options are available for the action (all are optional if a config file is present):

| Input | Default Value | Description | Example |
|---|---|---|---|
| `working-directory` | `"./"` | Working directory path | `"./src"` |
| `paths` | `"[]"` | Paths to scan, relative to the working directory, in square brackets and separated by commas. Required unless a `.lintspec.toml` file is present in the working directory. | `"[path/to/file.sol,test/test.sol]"` |
| `exclude` | `"[]"` | Paths to exclude, relative to the working directory, in square brackets and separated by commas | `"[path/to/exclude,other/path.sol]"` |
| `extra-args` | | Extra arguments passed to the `lintspec` command | `"--inheritdoc=false"` |
| `version` | `"latest"` | Version of lintspec to use. For enhanced security, you can pin this to a fixed version | `"0.4.1"` |
| `fail-on-problem` | `"true"` | Whether the action should fail when `NatSpec` problems have been found. Disabling this only creates annotations for found problems, but succeeds | `"false"` |

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
      - uses: beeb/lintspec@v0.4.1
        # all the lines below are optional
        with:
          working-directory: "./"
          paths: "[]"
          exclude: "[]"
          extra-args: ""
          version: "latest"
          fail-on-problem: "true"
```

## Credits

This tool walks in the footsteps of [natspec-smells](https://github.com/defi-wonderland/natspec-smells), thanks to
them for inspiring this project!

## Comparison with natspec-smells

### Benchmark

On an AMD Ryzen 9 7950X processor with 64GB of RAM, linting the
[Uniswap/v4-core](https://github.com/Uniswap/v4-core) `src` folder on WSL2 (Ubuntu), lintspec v0.4 is about 200x
faster, or 0.5% of the execution time:

```text
Benchmark 1: npx @defi-wonderland/natspec-smells --include "src/**/*.sol"
  Time (mean ± σ):     13.034 s ±  0.138 s    [User: 13.349 s, System: 0.560 s]
  Range (min … max):   12.810 s … 13.291 s    10 runs

Benchmark 2: lintspec src --compact --param-required struct
  Time (mean ± σ):      62.9 ms ±   2.4 ms    [User: 261.9 ms, System: 69.6 ms]
  Range (min … max):    55.1 ms …  66.5 ms    47 runs

Summary
  lintspec src --compact --param-required struct ran
  207.34 ± 8.28 times faster than npx @defi-wonderland/natspec-smells --include "src/**/*.sol"
```

On a MacBook Pro M2 Max with 96GB of RAM, linting the same Uniswap V4 repo using the experimental Solar backend is roughly
900x faster than natspec-smells:

```text
Benchmark 1: npx @defi-wonderland/natspec-smells --include "src/**/*.sol" --enforceInheritdoc --constructorNatspec
  Time (mean ± σ):     11.421 s ±  0.110 s    [User: 12.957 s, System: 0.578 s]
  Range (min … max):   11.241 s … 11.556 s    10 runs
 
  Warning: Ignoring non-zero exit code.
 
Benchmark 2: lintspec src --compact
  Time (mean ± σ):      12.7 ms ±   0.2 ms    [User: 17.6 ms, System: 9.7 ms]
  Range (min … max):    12.1 ms …  13.9 ms    184 runs
 
  Warning: Ignoring non-zero exit code.
 
Summary
  lintspec src --compact ran
  899.67 ± 19.41 times faster than npx @defi-wonderland/natspec-smells --include "src/**/*.sol" --enforceInheritdoc --constructorNatspec
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
| Configure via env variables     | ✅          | ❌                |
| Respects gitignore files        | ✅          | ❌                |
| Granular validation rules       | ✅          | ❌                |
| Pretty output with code excerpt | ✅          | ❌                |
| JSON output                     | ✅          | ❌                |
| Output to file                  | ✅          | ❌                |
| Multithreaded                   | ✅          | ❌                |
| Built-in CI action              | ✅          | ❌                |
| No pre-requisites (node/npm)    | ✅          | ❌                |
