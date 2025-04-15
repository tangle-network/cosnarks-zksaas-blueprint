// Placeholder for generate_proof job handler

use crate::context::CosnarksContext;
use crate::error::{Error, Result};
use crate::state::CircuitStore;
use crate::types::{CircuitId, ProofResult, WitnessInput};
use blueprint_sdk::crypto::KeyType;
use blueprint_sdk::extract::Context;
use blueprint_sdk::std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use blueprint_sdk::tangle::extract::{CallId, TangleArgs2, TangleResult};
use blueprint_sdk::{debug, info, warn};
use hex;
use std::sync::Arc;

/// Wrapper function that extracts arguments from TangleArgs2 and calls the main implementation
pub async fn generate_proof_job<K: KeyType + 'static>(
    Context(ctx): Context<CosnarksContext<K>>,
    CallId(call_id): CallId,
    TangleArgs2(circuit_id_bytes, witness_input): TangleArgs2<[u8; 32], WitnessInput>,
) -> Result<TangleResult<ProofResult>>
where
    K::Public: Ord + Hash, // Add Hash bound for session ID generation
{
    // Convert CircuitId bytes if needed, depends on how CircuitId is used internally
    // Assuming CircuitId is used directly as [u8; 32] internally now
    let circuit_id: CircuitId = circuit_id_bytes;

    let witness_data_str = match witness_input {
        WitnessInput::Json(json_str) => json_str,
        WitnessInput::Uri(uri_str) => {
            // TODO: Implement downloading witness data from URI
            warn!(uri = %uri_str, "Witness URI download not implemented, using empty witness.");
            "{}".to_string() // Placeholder
        }
    };

    let result = generate_proof(ctx, call_id, circuit_id, witness_data_str).await?;
    Ok(TangleResult(result))
}

/// Core implementation of the proof generation logic
pub async fn generate_proof<K: KeyType + 'static>(
    ctx: CosnarksContext<K>,
    call_id: u64,
    circuit_id: CircuitId, // Use [u8; 32] type directly
    witness_data_json: String,
) -> Result<ProofResult>
// Return standard ProofResult
where
    K::Public: Ord + Hash, // Add Hash bound for session ID generation
{
    let circuit_id_hex = hex::encode(circuit_id);
    info!(%call_id, %circuit_id_hex, "Starting proof generation");

    // 1. Get the circuit information
    let circuit_info = ctx
        .circuit_store()
        .get_circuit_info(&circuit_id_hex)? // Use hex ID for lookup if keys are hex strings
        .ok_or_else(|| Error::InvalidInput(format!("Circuit ID not found: {}", circuit_id_hex)))?;
    debug!(?circuit_info, "Found circuit info");

    // 2. Get the ordered list of participants for this session
    let mut participants = ctx.get_operators().await?;
    if participants.is_empty() {
        return Err(Error::ConfigError(
            "No operators found for the service/circuit".to_string(),
        ));
    }
    participants.sort();
    info!(num_participants = participants.len(), "Using participants");

    // 3. Create a unique session ID
    let session_id = generate_session_id(call_id, &participants);
    info!(%session_id, "Generated session ID");

    // 4. Establish the MPC session using the round-based protocol
    let mpc_handler = ctx
        .mpc_network_manager()
        .establish_mpc_session(&session_id, participants)
        .await?;

    // 5. Use the MPC handler to run the actual proof generation
    info!(%session_id, "MPC network established, running proof generation protocol...");

    // TODO: Replace placeholder with actual co-circom/co-noir call
    // let proof_result = co_lib::generate_proof(
    //     circuit_info.artifact_path, // Assuming CircuitStore provides absolute paths
    //     &witness_data_json,
    //     mpc_handler
    // ).await?;

    // Placeholder: Simulate proof generation
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let proof_bytes = format!("proof_for_{}_{}", circuit_id_hex, call_id).into_bytes();
    // Public inputs should be Vec<Vec<u8>>
    let public_inputs: Vec<Vec<u8>> = vec![format!("input_for_{}", call_id).into_bytes()];

    // 6. Construct the proof result
    let proof_result = ProofResult {
        proof_bytes,
        public_inputs,
    };

    info!(%call_id, %circuit_id_hex, %session_id, "Generated proof successfully.");
    Ok(proof_result)
}

/// Generates a unique session ID based on the call ID and participant keys.
fn generate_session_id<P: Hash>(call_id: u64, participants: &[P]) -> String {
    let mut hasher = DefaultHasher::new();
    call_id.hash(&mut hasher);
    participants.hash(&mut hasher);
    format!("mpc-session-{}", hasher.finish())
}
