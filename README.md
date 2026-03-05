# Seer CLI

Commandline tool for tracing Solana transactions using Seer.

---

## Getting Started

Create an account at [app.seer.run](http://app.seer.run) and generate an API key.

### Installation

This CLI can be installed using either the installation script or from source.

#### Script

```sh
curl -fsSL https://seer.run/install.sh | sh
```

#### Source

1. Download this repo:

```sh
git clone https://github.com/SeerApp/cli .
```

1. From root, build:

```sh
cargo build --release
```

1. Install `seer` globally:

```sh
./target/release/seer install
```

---

### Sessions

A Seer session is a special Solana Validator which processes your transactions and outputs your transaction traces in the Seer App. It works exactly like a regular Solana Test Validator, with the difference that it runs on a remote server, has access to mainnet state, and connects your transaction execution flow to your source code.

Log into your Seer CLI with the API key you generated in the app:

```sh
seer login <API_KEY>
```

Then, from inside of your Solana project, run:

```sh
seer run
```

This will compile your local programs with debug data, list the files necessary for the Seer debugger, ask for your consent to upload them, and start your Seer session. 

This command works in both native Solana and Anchor projects.

If you wish to give consent to upload all files necessary by default, run:

```sh
seer run --consent
```

After the command executes successfully, you will see your Seer session URL, in the following format:

```
New Seer session at: https://rpc.seer.run/...
```

The Seer session currently automatically ends after 15 minutes of inactivity.

---

### Tracing

You need to point your Solana tooling at the Seer session URL so that the transaction you want to trace is routed through Seer.

**Native Solana CLI / program:**

```sh
solana --url https://rpc.seer.run/... program deploy target/deploy/your_program.so
# and just write the same endpoint inside your test connection
```

**Anchor project:**

```sh
anchor test --provider.cluster https://rpc.seer.run/...
# or set it in Anchor.toml:
# [provider]
# cluster = "https://rpc.seer.run/..."
```

---

## Requirements

- Rust toolchain
- Solana CLI 3.0.0 and higher (`cargo-build-sbf`)
- A Seer API key (get one at [Seer](https://app.seer.run/))

> ⚠️ Seer **will not** work with Solana CLI versions before 3.0.0.

## Bug Reports

Seer is in early beta, so occasional bugs are expected. To report a bug, open an issue on this repo, or [DM us on X](https://x.com/SeerForSolana).