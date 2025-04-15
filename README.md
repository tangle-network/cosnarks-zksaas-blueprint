# <h1 align="center">CoSNARKs: Collaborative zkSNARKs as a Service Blueprint 🌐</h1>

## 📚 Overview

This Tangle Blueprint implements a "Collaborative zkSNARKs as a Service" (CoSNARKs-zkSaaS). It allows users to register ZK circuits (Circom or Noir) and then collaboratively generate zero-knowledge proofs with a group of registered operators.

Blueprints are infrastructure as code templates on the Tangle Network. This CoSNARKs blueprint leverages the Tangle Network for operator registration, job submission, and result handling, while utilizing a sophisticated off-chain P2P and MPC network for the core proof generation task.

For more general details on Blueprints, please refer to the [project documentation](https://docs.tangle.tools/developers/blueprints/introduction).

## ✨ Key Features

- **Circuit Registration**: Users can register Circom or Noir circuits by providing circuit metadata and a URL to the compiled artifact (`.r1cs`, `.acir`). Key generation (PK, VK) is intended to be handled by the operators.
- **Collaborative Proof Generation**: Registered operators work together to generate proofs for submitted jobs using Multi-Party Computation (MPC).
- **Secure Configuration Exchange**: Employs a secure, round-based P2P protocol (`mpc_config_exchange`) using `round_based` to reliably establish the necessary MPC network configuration (`mpc-net`) among participants before each proof generation session.
- **Robust Networking**: Integrates Blueprint SDK's libp2p networking for peer discovery and the round-based protocol, combined with the specialized `mpc-net` library for the high-performance, secure transport layer required during MPC.
- **Persistent State**: Uses `sled` database for storing circuit metadata (keyed by hex representation of `CircuitId`) and manages artifact files locally.
- **Flexible Witness Input**: Accepts witness data either directly as a JSON string or via a URI pointing to a JSON file.

## ⚙️ Architecture

1.  **Circuit Registration (`register_circuit` job)**:
    - Accepts: `name` (String), `circuit_type` (Enum), `proving_backend` (Enum), `artifact_url` (String).
    - Generates a deterministic `CircuitId` (`[u8; 32]`) based on metadata.
    - Validates backend/type compatibility.
    - Downloads the circuit artifact from the provided URL.
    - Generates placeholder proving and verification keys.
    - Stores circuit metadata (including relative artifact/key paths) in the local `CircuitStore` (sled DB, keyed by hex ID).
    - Stores the downloaded artifact and generated keys in the blueprint's data directory (`artifacts/{circuit_id_hex}/...`).
    - Returns `(CircuitId, VerifierAddress, VerificationKey)` as `([u8; 32], [u8; 20], Vec<u8>)` for Solidity.
2.  **Proof Generation (`generate_proof` job)**:
    - Accepts: `circuit_id` (`[u8; 32]`), `witness_input` (`WitnessInput` enum: JSON string or URI).
    - Retrieves circuit information from the `CircuitStore` using the hex ID.
    - Handles `WitnessInput`: uses JSON string directly or downloads from URI (TODO).
    - Identifies the participating operators for the service (`ctx.get_operators().await?`).
    - Sorts operators to ensure deterministic ordering.
    - Generates a unique session ID based on the `call_id` and participants.
    - Initiates the **MPC Configuration Exchange** (`mpc_config_exchange` protocol) via `MpcNetworkManager`:
      - Uses Blueprint's `RoundBasedNetworkAdapter`.
      - Securely exchanges and verifies MPC-Net listener details (DNS name, cert path) using commit-reveal.
    - **Establishes MPC-Net**: Uses the verified configuration to establish a secure `mpc-net` session (`MpcNetworkHandler`).
    - **Executes MPC**: (Placeholder) Calls the appropriate `co-circom`/`co-noir` library function with circuit data, witness, and the `MpcNetworkHandler`.
    - Returns the `ProofResult` (`{ proof_bytes: Vec<u8>, public_inputs: Vec<Vec<u8>> }`) for Solidity.

## 🧩 Core Components

- **`CosnarksContext`**: Holds shared state: `BlueprintEnvironment`, `CircuitStore`, `MpcNetworkManager`.
- **`CircuitStore`**: Manages persistent storage of circuit metadata and artifacts using `sled`.
- **`MpcNetworkManager`**: Orchestrates MPC session setup via the `mpc_config_exchange` protocol and `mpc-net`.
- **`p2p::mpc_config_exchange`**: The `round_based` protocol implementation for secure MPC-Net config sharing.
- **`types.rs`**: Defines core data structures (`CircuitId`, `CircuitInfo`, `CircuitType`, `ProvingBackend`, `ProofResult`, `WitnessInput`).

## 📋 Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable recommended)
- [Docker](https://docs.docker.com/get-docker/) (if running operators in containers)
- `cargo-tangle` CLI:
  ```bash
  cargo install cargo-tangle --git https://github.com/webb-tools/tangle --force
  ```

## 🛠️ Configuration & Running

Operators running this blueprint require specific configuration provided via environment variables managed by the `BlueprintEnvironment`:

**Standard Blueprint Variables:**

- Libp2p Networking: Keys, bootnodes, listen addresses.
- Keystore: Path and password for the operator's signing key.
- Tangle RPC: Endpoint URL for the Tangle node.
- Data Directory (`DATA_DIR`): Path for storing the `sled` database and downloaded artifacts. **Must be set.**

**MPC-Specific Environment Variables:**

- `MPC_LISTEN_DNS`: **Required.** The publicly reachable DNS name **and port** for the `mpc-net` listener. Must be resolvable by other operators. Example: `operator.example.com:9001` or `123.45.67.89:9001`.
- `MPC_KEY_PATH`: **Optional.** Path _relative to the `DATA_DIR`_ for the private key file used for `mpc-net` TLS. Defaults to `mpc_certs/mpc_key.der`.
- `MPC_CERT_PATH`: **Optional.** Path _relative to the `DATA_DIR`_ for the public certificate file used for `mpc-net` TLS. Defaults to `mpc_certs/mpc_cert.der`.

**(Note:** Generating the `mpc-net` key/cert pairs is outside the scope of this blueprint but is required for `mpc-net` operation. Standard TLS certificate generation methods (e.g., using `openssl`) can be used. Ensure the certificate corresponds to the private key and is trusted by other operators, typically via a shared CA or by distributing the certificates.)

**Running Locally (Testing)**

Refer to the integration tests (`tests/`) for examples using `TangleTestHarness`. This simulates the Tangle network and job lifecycle for local development and testing.

**Deployment**

```sh
cargo tangle blueprint deploy
```

Follow the prompts to configure deployment parameters on a live Tangle network.

## 📜 License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 📬 Feedback and Contributions

We welcome feedback and contributions to improve this blueprint.
Please open an issue or submit a pull request on our GitHub repository.
Please let us know if you fork this blueprint and extend it too!

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
