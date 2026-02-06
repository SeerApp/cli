# Seer Preflight Configuration Guide

This guide helps you set up your machine for Seer and local Gripmock testing, including resolving common errors like unauthorized access to the mocker Docker image.

---

## 1. GitHub Container Registry (GHCR) Authentication

### Why do I get "unauthorized" when pulling the mocker image?

If you see this error:

```
Error response from daemon: Head "https://ghcr.io/v2/seerapp/mocker/manifests/latest": unauthorized
```

It means Docker cannot access the image because GHCR requires authentication.

### Solution: Authenticate Docker with GHCR

#### Step 1: Create a GitHub Personal Access Token (PAT)

1. Go to https://github.com/settings/tokens
2. Click **"Generate new token (classic)"**. (**Only classic tokens work for such authentication.**)
3. Set a name (e.g., "Docker GHCR access")
4. Set expiration (e.g., 30 days)
5. Under "Select scopes", check:
   - `read:packages`
   - (Optional) `write:packages` if you need to push images
6. Click "Generate token"
7. Copy the token (you won't see it again!)

#### Step 2: Log in to Docker with your PAT

Open a terminal and run:

```
docker login ghcr.io -u <your-github-username> --password-stdin
```

Paste your PAT when prompted. 

If login is successful, you can pull the image:

```
docker pull ghcr.io/seerapp/mocker:latest
```

---

## 2. Buf Installation (for working with protos)

## 2. Accessing Buf Packages for Rust Projects

To use generated protobuf code in your Rust project, you do not need to install Buf CLI unless you want to generate code yourself. For most users, access is handled through Cargo and the Buf registry.

### Requirements

- You must be added to the Seer organization on GitHub and Buf to access private packages.
- You need a Buf API token (not a GitHub token) for authentication.

### Steps

1. Add the Buf registry to your `.cargo/config.toml`:

  ```toml
  [registries.buf]
  index = "sparse+https://buf.build/gen/cargo/"
  credential-provider = "cargo:token"
  ```

2. Login to Buf registry using your Buf API token:

  ```sh
  cargo login --registry buf
  ```
  - When prompted, use your Buf API token. To create a Buf API token:
    1. Go to https://buf.build and log in with your GitHub account.
    2. Click your profile icon (top right) and select "Settings".
    3. Give your token a name (pls use clear naming e.g., "Cargo access").
    4. Click "Create" and copy the token shown. You will not see it again.
    5. Use this token when cargo prompts for authentication.

3. Install packages:

  ```sh
    cargo add --registry buf seer_protos_community_neoeinstein-tonic seer_protos_community_neoeinstein-prost
  ```

4. Usage

```rust
// Messages
use seer_protos_community_neoeinstein_prost::seer::sessions::v1::*;

// Client
use seer_protos_community_neoeinstein_tonic::seer::sessions::v1::tonic::sessions_client;

// Server
use seer_protos_community_neoeinstein_tonic::seer::sessions::v1::tonic::sessions_server;
```

If you are not a member of the Seer organization or do not have a Buf API token, request access from your team lead or administrator.

---

## 3. Troubleshooting

- If you get "unauthorized" again, double-check your PAT scopes and username.
- If you lose your PAT, generate a new one.
- If Docker login fails, try logging out first:
  ```sh
  docker logout ghcr.io
  ```
- Make sure you are using your GitHub username, not your email.

---

## 4. Additional Resources

- [GHCR documentation](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)
- [Buf documentation](https://docs.buf.build/)

---

## 5. Next Steps

Once you have completed these steps, you can follow the [gripmock.md](gripmock.md) guide to run the mocker and upload payloads.

If you encounter issues not covered here, check the official docs or search for the error message online before asking in group chat.
