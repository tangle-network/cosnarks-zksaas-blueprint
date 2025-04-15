// Placeholder for generate_proof job handler

use crate::context::CosnarksContext;
use crate::error::{Error, Result};
use crate::types::{CircuitId, ProofResult};
use blueprint_sdk::crypto::KeyType;
use blueprint_sdk::extract::Context;
use blueprint_sdk::std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use blueprint_sdk::tangle::extract::{CallId, TangleArgs2, TangleResult};
use blueprint_sdk::{debug, info};

/// Wrapper function that extracts arguments from TangleArgs2 and calls the main implementation
pub async fn generate_proof_job<K: KeyType + 'static>(
    Context(ctx): Context<CosnarksContext<K>>,
    CallId(call_id): CallId,
    TangleArgs2(circuit_id, witness_data_json): TangleArgs2<CircuitId, String>,
) -> Result<TangleResult<ProofResult>>
where
    K::Public: Ord + Hash, // Add Hash bound for session ID generation
{
    let result = generate_proof(ctx, call_id, circuit_id, witness_data_json).await?;
    Ok(TangleResult(result))
}

/// Core implementation of the proof generation logic
pub async fn generate_proof<K: KeyType + 'static>(
    ctx: CosnarksContext<K>,
    call_id: u64,
    circuit_id: CircuitId,
    witness_data_json: String,
) -> Result<ProofResult>
where
    K::Public: Ord + Hash, // Add Hash bound for session ID generation
{
    info!(%call_id, %circuit_id, "Starting proof generation");

    // 1. Get the circuit information
    let circuit_info = ctx
        .circuit_store()
        .get_circuit_info(&circuit_id)?
        .ok_or_else(|| Error::InvalidInput(format!("Circuit ID not found: {}", circuit_id)))?;
    debug!(?circuit_info, "Found circuit info");

    // 2. Get the ordered list of participants for this session
    // In a real scenario, this might depend on the circuit or service instance
    let mut participants = ctx.get_operators().await?;
    if participants.is_empty() {
        // If no specific operators registered, maybe fall back to a default set or fail
        return Err(Error::ConfigError(
            "No operators found for the service/circuit".to_string(),
        ));
    }
    // Ensure deterministic order for PartyIndex mapping
    participants.sort();
    info!(num_participants = participants.len(), "Using participants");

    // 3. Create a unique session ID
    let session_id = generate_session_id(call_id, &participants);
    info!(%session_id, "Generated session ID");

    // 4. Establish the MPC session using the round-based protocol
    let mpc_handler = ctx
        .mpc_network_manager()
        .lock()
        .unwrap() // Use synchronous lock here, consider Tokio mutex if needed elsewhere
        .establish_mpc_session(&session_id, participants)
        .await?;

    // 5. Use the MPC handler to run the actual proof generation
    info!(%session_id, "MPC network established, running proof generation protocol...");

    // This is where we'd run the actual MPC proof generation logic
    // let proof_data = co_lib::generate_proof(
    //     circuit_info.artifact_path(),
    //     &witness_data_json,
    //     mpc_handler
    // ).await?;

    // Placeholder: Simulate proof generation
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let proof_bytes = format!("proof_for_{}_{}", circuit_id, call_id).into_bytes();
    let public_inputs = vec![format!("input_for_{}", call_id)];

    // 6. Construct the proof result
    let proof_result = ProofResult {
        proof_bytes,
        public_inputs,
    };

    info!(%call_id, %circuit_id, %session_id, "Generated proof successfully.");
    Ok(proof_result)
}

/// Generates a unique session ID based on the call ID and participant keys.
fn generate_session_id<P: Hash>(call_id: u64, participants: &[P]) -> String {
    let mut hasher = DefaultHasher::new();
    call_id.hash(&mut hasher);
    participants.hash(&mut hasher);
    format!("mpc-session-{}", hasher.finish())
}
