#![cfg(test)]
use blueprint_sdk::{
    crypto::sp_core::SpSr25519,
    runner::config::BlueprintEnvironment,
    tangle::serde::{from_field, to_field},
    testing::{
        chain_setup::tangle::transactions::wait_for_completion_of_tangle_job,
        utils::tangle::TangleTestHarness,
    },
};
use cosnarks_zksaas_blueprint_lib::{
    context::CosnarksContext,
    error::{Error, Result},
    jobs::{
        GENERATE_PROOF_JOB_ID, REGISTER_CIRCUIT_JOB_ID, generate_proof::generate_proof_job,
        register_circuit::register_circuit,
    },
    types::{CircuitId, CircuitType, ProofResult, ProvingBackend, WitnessInput},
};
use httpmock::prelude::*;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;
use url::Url;

// --- Test Setup ---
// Helper to create a dummy artifact file server
async fn setup_mock_artifact_server(mock_server: &MockServer, path: &str, content: &[u8]) {
    mock_server.mock(|when, then| {
        when.method(GET).path(path);
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body(content);
    });
}

// --- E2E Test ---

#[tokio::test]
async fn test_e2e_circuit_registration_and_proof() -> Result<(), Error> {
    blueprint_sdk::testing::utils::setup_log();

    // 1. Setup Test Environment
    let env = BlueprintEnvironment::default();
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let data_dir = temp_dir.path().to_path_buf();
    let harness = TangleTestHarness::<SpSr25519>::setup(temp_dir)
        .await
        .map_err(|e| blueprint_sdk::Error::Other(e.to_string()))?;

    // Mock artifact server
    let server = MockServer::start();
    let artifact_content = b"dummy circuit artifact data";
    let artifact_path = "/test_circuit.r1cs";
    setup_mock_artifact_server(&server, artifact_path, artifact_content).await;
    let artifact_url = server.url(artifact_path);

    // Setup MPC Network Env Vars (Required by CosnarksContext::new)
    let mpc_listen_dns = "127.0.0.1:9001".to_string(); // Dummy listener for test
    let mpc_key_path_rel = "mpc_certs/test_key.der";
    let mpc_cert_path_rel = "mpc_certs/test_cert.der";
    // Use unsafe block for setting env vars in test
    unsafe {
        std::env::set_var("MPC_LISTEN_DNS", &mpc_listen_dns);
        std::env::set_var("MPC_KEY_PATH", mpc_key_path_rel);
        std::env::set_var("MPC_CERT_PATH", mpc_cert_path_rel);
    }

    // Create dummy cert/key files in the data dir
    let cert_dir = data_dir.join("mpc_certs");
    fs::create_dir_all(&cert_dir).expect("Failed to create dummy cert dir");
    fs::write(cert_dir.join("test_key.der"), b"dummy key").expect("Failed to write dummy key");
    fs::write(cert_dir.join("test_cert.der"), b"dummy cert").expect("Failed to write dummy cert");

    // Setup context and test environment
    let mut context = CosnarksContext::<SpSr25519>::new(Arc::new(env))
        .await
        .map_err(|e| blueprint_sdk::Error::Other(e.to_string()))?;
    const N: usize = 3;
    let (mut test_env, service_id, _) = harness.setup_services::<N>(false).await?;

    // Add routes
    test_env
        .add_job(REGISTER_CIRCUIT_JOB_ID, register_circuit::<SpSr25519>)
        .await;
    test_env
        .add_job(GENERATE_PROOF_JOB_ID, generate_proof_job::<SpSr25519>)
        .await;

    test_env.start(()).await?;

    // 2. Register Circuit Job
    let name = "test_circuit".to_string();
    let circuit_type = CircuitType::Circom;
    let backend = ProvingBackend::Groth16;

    let register_inputs = vec![
        to_field(&name)?,         // name
        to_field(&circuit_type)?, // circuit_type
        to_field(&backend)?,      // proving_backend
        to_field(&artifact_url)?, // artifact_url
    ];

    println!("Submitting register_circuit job...");
    let call = harness
        .submit_job(service_id, REGISTER_CIRCUIT_JOB_ID, register_inputs)
        .await?;
    println!("Waiting for register_circuit job execution...");
    let result =
        wait_for_completion_of_tangle_job(harness.client(), service_id, call.call_id, 1).await?;
    println!("register_circuit job completed.");

    assert!(
        result.output.is_some(),
        "Register circuit job failed to produce output"
    );
    let output_fields = result.output.unwrap();
    assert_eq!(output_fields.len(), 3, "Expected 3 output fields");

    let circuit_id_bytes: Vec<u8> = from_field(&output_fields[0])?;
    let verifier_addr_bytes: Vec<u8> = from_field(&output_fields[1])?;
    let vk_bytes: Vec<u8> = from_field(&output_fields[2])?;

    assert_eq!(circuit_id_bytes.len(), 32, "Circuit ID should be 32 bytes");
    assert_eq!(
        verifier_addr_bytes.len(),
        20,
        "Verifier address should be 20 bytes"
    );
    assert!(!vk_bytes.is_empty(), "Verification key should not be empty");
    println!(
        "Circuit registered successfully: ID={}",
        hex::encode(&circuit_id_bytes)
    );

    let circuit_id: CircuitId = circuit_id_bytes
        .try_into()
        .expect("Invalid circuit ID length");

    // Verify artifacts were stored (basic check)
    let circuit_id_hex = hex::encode(circuit_id);
    let artifact_store_path = data_dir.join("artifacts").join(&circuit_id_hex);
    assert!(
        artifact_store_path.exists(),
        "Artifact directory not created"
    );
    assert!(
        artifact_store_path.join("circuit_artifact.r1cs").exists(),
        "Circuit artifact not found"
    );
    assert!(
        artifact_store_path.join("proving.key").exists(),
        "Proving key not found"
    );
    assert!(
        artifact_store_path.join("verification.key").exists(),
        "Verification key not found"
    );

    // 3. Generate Proof Job
    let witness_json = serde_json::json!({ "a": 1, "b": 2 }).to_string();
    let witness_input = WitnessInput::Json(witness_json);

    let proof_inputs = vec![
        to_field(&circuit_id)?,    // circuit_id ([u8; 32])
        to_field(&witness_input)?, // witness_input (enum)
    ];

    println!("Submitting generate_proof job...");
    let proof_call = harness
        .submit_job(service_id, GENERATE_PROOF_JOB_ID, proof_inputs)
        .await?;
    println!("Waiting for generate_proof job execution...");
    let proof_result_exec =
        wait_for_completion_of_tangle_job(harness.client(), service_id, proof_call.call_id, 1)
            .await?;
    println!("generate_proof job completed.");

    assert!(
        proof_result_exec.output.is_some(),
        "Generate proof job failed to produce output"
    );
    let proof_output_fields = proof_result_exec.output.unwrap();
    // ProofResult encodes to a single field (struct)
    assert_eq!(
        proof_output_fields.len(),
        1,
        "Expected 1 output field for ProofResult"
    );

    let proof_result: ProofResult = from_field(&proof_output_fields[0])?;

    assert!(
        !proof_result.proof_bytes.is_empty(),
        "Proof bytes should not be empty"
    );
    assert!(
        !proof_result.public_inputs.is_empty(),
        "Public inputs should not be empty"
    );
    println!("Proof generated successfully.");

    // Clean up env vars
    // Use unsafe block for removing env vars in test
    unsafe {
        std::env::remove_var("MPC_LISTEN_DNS");
        std::env::remove_var("MPC_KEY_PATH");
        std::env::remove_var("MPC_CERT_PATH");
    }

    Ok(())
}
