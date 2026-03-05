# Seer CLI

Seer CLI is a cross-platform command-line tool that lets you trace transactions running on your Solana programs — directly from the terminal.

With Seer you can finally see inside your on-chain programs. Run a transaction, and get back a full execution trace — every instruction, every account touched, every state change — all annotated against your original source code. No more guessing from logs. No more blind debugging. Just a clear picture of exactly what your program did, line by line.

## What It Does

Every transaction your Solana program handles tells a story — Seer CLI lets you read it. Point the CLI at your project and it takes care of everything: building your programs, uploading the artifacts, and handing you back a link where the full execution of your transaction unfolds step by step. No log grepping, no manual ELF inspection — just a clear, source-level trace of exactly what happened, and why.

## Features

- Detects all Solana native programs in your project (single-package or workspace)
- Builds each program and prepares it for tracing
- Uploads artifacts to the Seer platform and returns a shareable trace URL
- API key management via flag, environment variable, or stored config
- Works on Linux, macOS and WSL.

## Installation

Download the latest binary directly:

```
<URL here>
```

Or install with curl (Linux/macOS):

```sh
curl -sSfL <curl command here> | sh
```


## Usage

Navigate to the root of your Solana project and run the desired subcommand.

---

### `seer build`

Builds all Solana native programs in the current project, preparing them for tracing.

```sh
seer build [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--silent` | Suppress build output for a quieter experience | `false` |

**Example:**
```sh
seer build
seer build --silent
```

---

### `seer run`

Builds programs (unless skipped), uploads the artifacts to Seer, and prints a URL to the resulting debug trace.

```sh
seer run [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--artifacts <PATH>` | Path to the build artifacts directory | `./target/deploy` |
| `--skip-build` | Skip building programs and upload existing artifacts | `false` |
| `--consent` | Automatically approve the upload consent prompt | `false` |
| `--silent` | Suppress build output | `true` |
| `--api-key <API_KEY>` | API key for this run (overrides env variable and saved config) | — |

**API key resolution order:**
1. `--api-key` flag
2. `SEER_API_KEY` environment variable
3. Key saved by `seer login`

**Consent:** By default, Seer will prompt you to confirm before uploading your files. Pass `--consent` to skip the prompt (useful in CI). Uploaded files are stored temporarily and deleted automatically after **7 days**.

**Example:**
```sh
seer run
seer run --skip-build --artifacts ./my/custom/deploy
seer run --consent --api-key sk_...
SEER_API_KEY=sk_... seer run --consent
seer run --silent=false   # re-enable build output (silent is true by default)
```

---

### `seer login`

Saves your Seer API key to the local config file so you don't have to pass it on every `run` invocation.

```sh
seer login [API_KEY]
```

- If `API_KEY` is omitted you will be prompted to enter it securely (input is hidden).
- The key is stored in the platform config directory (e.g. `~/.config/seer/cli/api_key` on Linux).

**Example:**
```sh
seer login sk_...
seer login          # prompts for key
```

---

## How It Works

### `seer build`
1. Detects all Solana native programs in the current directory (supports workspaces).
2. Builds each program with the configuration needed for tracing.
3. Restores the original project state after building.

### `seer run`
1. Performs all steps of `seer build` (unless `--skip-build` is used).
2. Resolves the API key (flag → env → saved config).
3. Prompts for upload consent (skippable with `--consent`).
4. Collects built artifacts: program binaries, keypair files, and source paths.
5. Uploads everything to the Seer backend.
6. Prints the URL to the cust RPC-provider that would trace all your transactions.

---

## Tracing Your Transaction

When `seer run` completes it prints a URL — this is a custom Seer RPC endpoint scoped to your upload session. You need to point your Solana tooling at it so that the transaction you want to trace is routed through Seer.

**Native Solana CLI / program:**
```sh
solana --url <rpc-url> program deploy target/deploy/your_program.so
# and just write the same endpoint inside your test connection
```

**Anchor project:**
```sh
anchor test --provider.cluster <seer-rpc-url>
# or set it in Anchor.toml:
# [provider]
# cluster = "<seer-rpc-url>"
```

Once your transaction is confirmed, open [Seer](https://app.seer.run/) and you'll find the full trace waiting for you — every cross-program invocation, account mutation, and log line mapped back to your source code.

## Requirements

- Rust toolchain
- Solana toolchain (`cargo-build-sbf`)
- A Seer API key (get one at [Seer](https://app.seer.run/))

---

## Local Gripmock Testing

If you want instructions for local Gripmock testing of the CLI, please see:

- [gripmock.md](gripmock.md)

