use blueprint_sdk::crypto::KeyType;
use blueprint_sdk::networking::types::ParticipantId;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

// Represents the type of circuit (Circom or Noir)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CircuitType {
    Circom,
    Noir,
}

// Represents the ZK proving backend (Groth16, Plonk, UltraHonk)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProvingBackend {
    Groth16,   // Compatible with Circom
    Plonk,     // Compatible with Circom
    UltraHonk, // Compatible with Noir
}

// Identifier for a registered circuit
pub type CircuitId = String;

// Information stored about a registered circuit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitInfo {
    pub id: CircuitId,
    pub name: String,
    pub description: Option<String>,
    pub circuit_type: CircuitType,
    pub proving_backend: ProvingBackend,
    // Path to the compiled circuit artifact (e.g., R1CS, ACIR bytecode)
    pub artifact_path: PathBuf,
    // Path to the generated proving key (specific to the backend)
    pub proving_key_path: PathBuf,
    // Path to the verification key (useful for on-chain verification)
    pub verification_key_path: PathBuf,
}

// Input data for generating a proof for a specific circuit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRequestInput {
    // Could be JSON, a file path, or raw bytes depending on circuit input format
    pub witness_data: serde_json::Value, // Example: JSON for witness values
                                         // Potentially public inputs separated if needed by the contract
                                         // pub public_inputs: Vec<String>,
}

// The generated proof and public inputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofResult {
    pub proof_bytes: Vec<u8>,
    pub public_inputs: Vec<String>, // Or a more structured type
}

// Job ID constants (defined in lib.rs and jobs/mod.rs, but good to reference)
// pub const REGISTER_CIRCUIT_JOB_ID: u32 = 0;
// pub const GENERATE_PROOF_JOB_ID: u32 = 1;

/// Message gossiped over Blueprint's libp2p network
/// for operators to announce their MPC-Net listener info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpcNetAnnounce<K: KeyType> {
    /// The public key of the announcing operator.
    pub public_key: K::Public,
    /// The SocketAddr the operator's mpc-net instance is listening on.
    pub listen_addr: SocketAddr,
    /// The filesystem path to the operator's public certificate (DER format).
    /// Peers need this path *relative to their own environment* if they share
    /// a filesystem, or the *content* of the certificate if they don't.
    /// Sending the path is simpler if a shared FS or known structure is assumed.
    /// Sending content is more robust but increases message size.
    /// Let's send the path for now, assuming a standard deployment structure.
    pub cert_path: PathBuf,
    /// A nonce or timestamp to ensure freshness
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpcPeerInfo {
    pub id: u32,
    pub dns_name: String,
    pub cert_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpcNetworkConfig {
    pub my_id: u32,
    pub bind_addr: String,
    pub key_path: PathBuf,
    pub parties: Vec<MpcPeerInfo>,
}

// Topic name for MPC discovery on the P2P network
pub const MPC_DISCOVERY_TOPIC: &str = "zkSaaS.mpc.discovery.v1";

/// Message types for MPC discovery over the Blueprint p2p network
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MpcDiscoveryMessage {
    /// Announce this node's presence and MPC connection details
    #[serde(rename = "announce")]
    Announce {
        /// The assigned MPC ID (once assigned by coordinator)
        mpc_id: ParticipantId,
        /// The bind address for MPC-Net
        bind_addr: String,
        /// Path to the certificate for TLS
        cert_path: String,
        /// The session ID
        session_id: u64,
        /// Timestamp of the announcement
        timestamp: u64,
    },

    /// Acknowledge receipt of an announcement
    #[serde(rename = "acknowledge")]
    Acknowledge {
        /// Our assigned MPC ID (if any)
        mpc_id: ParticipantId,
        /// Our bind address
        bind_addr: String,
        /// Path to our certificate
        cert_path: String,
        /// The session ID
        session_id: u64,
        /// Timestamp of the acknowledgement
        timestamp: u64,
    },

    /// Indicate that a session is ready to begin
    #[serde(rename = "session_ready")]
    SessionReady {
        /// The session ID
        session_id: u64,
        /// Timestamp of the ready message
        timestamp: u64,
    },
}
