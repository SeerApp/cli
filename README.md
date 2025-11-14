# Seer CLI

Seer CLI is a cross-platform command-line tool for building Solana native programs with DWARF debug information. It automates the process of compiling Solana programs so that the resulting ELF files contain the necessary debug data for advanced inspection and debugging.

## Features
- Detects all Solana native programs in your project (single-package or workspace)
- Creates a `seer.toml` file for each program with `[profile.release] debug = true` before building
- Builds each program using Solana's `cargo-build-sbf` with DWARF debug info
- Restores original state of project after build
- Works on Windows, Linux, and Mac

## Installation

1. Build from source (seer-main/seer-cli):
   ```sh
   cargo build --release
   ```
   The binary will be located at `./target/release/seer-cli` (or `seer-cli.exe` on Windows).

2. (Optionally) **After build** install globally using the CLI:
   ```sh
   ./target/release/seer-cli install
   ```
   - On Linux/macOS/WSL, this copies the binary to `/usr/local/bin/seer-cli` (may require `sudo`).
   - On Windows, it copies to `%USERPROFILE%\.cargo\bin\seer-cli.exe`.

    After installation, you can run `seer-cli` from any directory.

## Usage

Navigate to the folder of your Solana project you want to debug and run:

If you have installed the CLI globally, you can simply run:
```sh
seer-cli build
```

If you have not installed the CLI globally, use the path to the binary file. For example (from seer-main/demo on WSL):
```sh
/mnt/e/Projects/Rust/seer-main/seer-cli/target/release/seer-cli build
```

### Options
- `--silent`        Suppress output for a quieter build process

Examples:
```sh
seer-cli build --silent
```

## How It Works
1. Detects all Solana native programs in the current project directory.
2. Creates a `seer.toml` file for each program with `[profile.release] debug = true`.
3. Builds each program with DWARF debug info using `cargo-build-sbf`.
4. Restores the original state of project after building.
6. The `install` subcommand copies the binary to a directory in your PATH for global usage.

## Output
- Compiled ELF files with `.so` extension are placed in `target/deploy` for each program.
- These files contain DWARF debug data and can be inspected with tools like `llvm-dwarfdump` and `rustfilt`.

## Debug Data Verification
To verify that the ELF files contain the correct debug info heuristicly and manually you can run:
```sh
program=package_name; [ "$(llvm-dwarfdump --debug-info --debug-line target/deploy/$program.so | rustfilt | grep -o "$program" | wc -l)" -ge 3 ] && echo OK || echo FAIL
```

## Requirements
- Rust toolchain
- Solana toolchain (`cargo-build-sbf`)
- (For data manual verification) LLVM tools (`llvm-dwarfdump`)
