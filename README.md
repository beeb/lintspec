# lintspec

![lintspec screenshot](./screenshot.png)

Lintspec is a command-line utility (linter) that checks the completeness and validity of
[NatSpec](https://docs.soliditylang.org/en/latest/natspec-format.html) doc-comments in Solidity code. It is focused on
speed and ergonomics. By default, lintspec will respect gitignore rules when looking for Solidity source files.

Dual-licensed under MIT or Apache 2.0.

## Installation

Via `cargo`:

```bash
cargo install --locked lintspec
```

Via `nix` (coming soon):

```bash
nix-env -iA nixpkgs.lintspec
# or
nix-shell -p lintspec
# or
nix run nixpkgs#lintspec
```

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

## Credits

This tool walks in the footsteps of [natspec-smells](https://github.com/defi-wonderland/natspec-smells), thanks to
them for inspiring this project!

## Comparison with natspec-smells

### Benchmark

On an AMD Ryzen 9 7950X processor with 64GB of RAM, linting the
[Uniswap/v4-core](https://github.com/Uniswap/v4-core) `src` folder on WSL2 (Ubuntu), lintspec is about 142x faster, or
a reduction of the execution time of 99.3%:

```
Benchmark 1: npx @defi-wonderland/natspec-smells --include "src/**/*.sol"
  Time (mean ± σ):     12.524 s ±  0.142 s    [User: 13.628 s, System: 0.580 s]
  Range (min … max):   12.282 s … 12.765 s    10 runs

Benchmark 2: lintspec src
  Time (mean ± σ):      87.9 ms ±   3.7 ms    [User: 268.0 ms, System: 97.9 ms]
  Range (min … max):    84.9 ms … 105.7 ms    28 runs
```
