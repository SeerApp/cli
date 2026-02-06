## Local Gripmock Testing for Seer CLI

This guide describes how to set up and use local Gripmock testing for the Seer CLI.

> **Before you start:**
>
> If you are new to Docker, Gripmock, or Buf, or encounter errors like `unauthorized` when pulling the mocker image, please see [preflight-setup.md](preflight-setup.md) for step-by-step configuration and troubleshooting.


### 1. Pull and Run the Mocker Container

First, pull the mocker Docker image and run it as described in the mocker documentation:

```sh
docker pull ghcr.io/seerapp/mocker:latest
```

Run the mocker for the sessions proto (or your desired proto):

```sh
docker run -p 4770:4770 -p 4771:4771 ghcr.io/seerapp/mocker:latest sessions/sessions.proto
```

This will start the mock gRPC server on port 4770 and the HTTP API for stubs on port 4771.

### 2. Upload the Payload

With the mocker running, upload your test payload to configure the mock responses:

```sh
curl -X POST http://localhost:4771/add -H "Content-Type: application/json" -d @payload_create.json && 
curl -X POST http://localhost:4771/add -H "Content-Type: application/json" -d @payload_run.json
```

This will set up the mock server with the responses defined in your payload.json file.

### 3. Run Seer CLI

After the above preparations, you can run the Seer CLI (installation needed) on your project with

```sh
seer run
```

This will build your project and interact with the locally running mock server as configured.

---

For more details on the mocker container, see:
- [cli/protos/mocker/README.md](https://github.com/SeerApp/protos/tree/main/mocker)