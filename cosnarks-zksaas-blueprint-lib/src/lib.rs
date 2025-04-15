//! cosnarks-zksaas-blueprint-lib
//! Core library for the Collaborative zkSNARKs as a Service Blueprint.

// Modules
pub mod context;
pub mod error;
pub mod jobs;
pub mod network;
pub mod p2p;
pub mod state;
pub mod types;

// Re-exports for convenience
pub use context::CosnarksContext;
pub use error::{Error, Result};
pub use jobs::{GENERATE_PROOF_JOB_ID, REGISTER_CIRCUIT_JOB_ID};
pub use state::CircuitStore;
pub use types::{CircuitId, CircuitInfo, CircuitType, ProofResult, ProvingBackend, WitnessInput};

// Ensure blueprint_sdk is accessible
pub use blueprint_sdk;
