// Job IDs for the zkSaaS Blueprint

// Job to register a new circuit (program) and generate its keys.
// Input: Circuit definition (source code or compiled artifact), Name, Description, Type, Backend
// Output: Circuit ID
pub const REGISTER_CIRCUIT_JOB_ID: u64 = 0; // Use u64 as per Blueprint examples

// Job to generate a proof for a registered circuit.
// Input: Circuit ID, Witness Data
// Output: Proof Bytes, Public Inputs
pub const GENERATE_PROOF_JOB_ID: u64 = 1;

// --- Potentially add more jobs later ---
// pub const UNREGISTER_CIRCUIT_JOB_ID: u64 = 2;
// pub const GET_CIRCUIT_INFO_JOB_ID: u64 = 3;

// --- Job Handler Modules ---
pub mod generate_proof;
pub mod register_circuit;

// Re-export handlers
pub use generate_proof::generate_proof;
pub use register_circuit::register_circuit;
