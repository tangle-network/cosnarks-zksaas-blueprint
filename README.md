# <h1 align="center">CoSNARKs: Collaborative zkSNARKs as a Service Blueprint 🌐</h1>

## 📚 Overview

This Tangle Blueprint implements a "Collaborative zkSNARKs as a Service" (CoSNARKs-zkSaaS). It allows users to register ZK circuits (Circom or Noir) and then collaboratively generate zero-knowledge proofs with a group of registered operators.

Blueprints are infrastructure as code templates on the Tangle Network. This CoSNARKs blueprint leverages the Tangle Network for operator registration, job submission, and result handling, while utilizing a sophisticated off-chain P2P and MPC network for the core proof generation task.

For more general details on Blueprints, please refer to the [project documentation](https://docs.tangle.tools/developers/blueprints/introduction).

## ✨ Key Features

- **Circuit Registration**: Users can register Circom or Noir circuits by providing artifacts (compiled circuit, proving key, verification key) via URLs.
- **Collaborative Proof Generation**: Registered operators work together to generate proofs for submitted jobs using Multi-Party Computation (MPC).
- **Secure Configuration Exchange**: Employs a secure, round-based P2P protocol (`mpc_config_exchange`) to reliably establish the necessary MPC network configuration (`mpc-net`) among participants before each proof generation session.
- **Robust Networking**: Integrates Blueprint SDK's libp2p networking for peer discovery and the round-based protocol, combined with the specialized `mpc-net` library for the high-performance, secure transport layer required during MPC.
- **Persistent State**: Uses `sled` database for storing circuit metadata and manages artifact files locally.

## ⚙️ Architecture

1.  **Circuit Registration (`register_circuit` job)**:
    - Accepts circuit details (name, type, backend) and URLs for artifacts.
    - Downloads artifacts (circuit, pk, vk).
    - Stores circuit metadata in the local `CircuitStore` (sled DB).
    - Stores artifact files in the blueprint's data directory.
2.  **Proof Generation (`generate_proof` job)**:
    - Receives a request with the `circuit_id` and `witness_data`.
    - Retrieves circuit information from the `CircuitStore`.
    - Identifies the participating operators for the service (fetched via `CosnarksContext::get_operators`).
    - Initiates the **MPC Configuration Exchange**:
      - Uses the `MpcNetworkManager` and Blueprint's `RoundBasedNetworkAdapter`.
      - Runs the `mpc_config_exchange` protocol (commit-reveal scheme) to securely share and verify each operator's MPC-Net connection details (hostname:port, certificate path).
      - Assigns MPC IDs based on the deterministic order of operators.
    - **Establishes MPC-Net**: Uses the verified configuration from the exchange protocol to establish a secure `mpc-net` session via `MpcNetworkHandler`.
    - **Executes MPC**: (Placeholder) Calls the appropriate `co-circom` or `co-noir` library function, passing the circuit data, witness, and the established `MpcNetworkHandler`.
    - Returns the generated `ProofResult`.

## 🧩 Core Components

- **`CosnarksContext`**: Holds shared state like the `BlueprintEnvironment`, `CircuitStore`, and the `MpcNetworkManager`.
- **`CircuitStore`**: Manages persistent storage of circuit metadata and artifacts.
- **`MpcNetworkManager`**: Orchestrates the establishment of MPC sessions, including running the configuration exchange protocol and setting up the `mpc-net` handler.
- **`p2p::mpc_config_exchange`**: The round-based protocol implementation for secure MPC-Net configuration sharing.

## 📋 Prerequisites

Before you can run this project, you will need to have the following software installed on your machine:

- [Rust](https://www.rust-lang.org/tools/install) (latest stable recommended)
- [Docker](https://docs.docker.com/get-docker/) (if running operators in containers or using containerized dependencies)
- [Forge](https://getfoundry.sh) (for EVM interactions if extending for Eigenlayer/EVM)

You will also need to install [cargo-tangle](https://crates.io/crates/cargo-tangle), our CLI tool for creating and deploying Tangle Blueprints:

To install the Tangle CLI, run the following command:

> Supported on Linux, MacOS, and Windows (WSL2)

```bash
cargo install cargo-tangle --git https://github.com/tangle-network/blueprint --force
```

## ⭐ Getting Started

1.  **Clone the repository:**
    ```sh
    git clone <repository-url>
    cd cosnarks-zksaas-blueprint
    ```
2.  **Build the project:**
    ```sh
    cargo build
    ```

## 🛠️ Configuration & Running

Operators running this blueprint need specific configuration, typically provided via environment variables managed by the `BlueprintEnvironment`:

- **Networking**: Libp2p keys, bootnodes, listen addresses (standard Blueprint config).
- **Keystore**: Access to the operator's signing key.
- **Tangle RPC**: Endpoint for the Tangle node.
- **Data Directory**: Path for storing the `sled` database and downloaded artifacts.
- **MPC Configuration**:
  - `MPC_LISTEN_DNS`: The publicly reachable DNS name or IP address and port for the `mpc-net` listener (e.g., `operator.example.com:9001`).
  - `MPC_KEY_PATH`: Path to the private key file for `mpc-net` TLS.
  - `MPC_CERT_PATH`: Path to the public certificate file for `mpc-net` TLS.

**(Note:** Generating the `mpc-net` key/cert pairs is outside the scope of this blueprint but is required for `mpc-net` operation. Standard TLS certificate generation methods can be used.)

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
