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
pub use error::Error;
pub use jobs::{GENERATE_PROOF_JOB_ID, REGISTER_CIRCUIT_JOB_ID};
pub use types::{
    CircuitId, CircuitInfo, CircuitType, ProofRequestInput, ProofResult, ProvingBackend,
};

// Alias for Result used throughout the library
pub type Result<T, E = Error> = std::result::Result<T, E>;

// Ensure blueprint_sdk is accessible
pub use blueprint_sdk;
