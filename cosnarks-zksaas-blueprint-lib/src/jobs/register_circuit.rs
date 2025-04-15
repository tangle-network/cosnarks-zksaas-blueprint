// Placeholder for register_circuit job handler

use crate::context::CosnarksContext;
use crate::error::{Error, Result};
use crate::state::CircuitStore;
use crate::types::{CircuitId, CircuitInfo, CircuitType, OptionalJsonParams, ProvingBackend};
// use blueprint_sdk::macros::debug_job; // Macro doesn't support generics yet
use blueprint_sdk::crypto::KeyType;
use blueprint_sdk::crypto::hashing::blake3_256;
use blueprint_sdk::extract::Context;
use blueprint_sdk::tangle::extract::{CallId, TangleArgs4, TangleResult};
use reqwest;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tracing::{debug, error, info};
use url::Url;

const ARTIFACT_FILENAME: &str = "circuit_artifact"; // Generic name, extension added later
const PROVING_KEY_FILENAME: &str = "proving.key";
const VERIFICATION_KEY_FILENAME: &str = "verification.key";

// Example Input Arguments (adjust as needed):
// - circuit_name: String
// - circuit_description: Optional<String>
// - circuit_type: CircuitType (enum Circom/Noir)
// - proving_backend: ProvingBackend (enum Groth16/Plonk/UltraHonk)
// - circuit_artifact_url: String (URL to download .r1cs, .acir, etc.)
// - optional_setup_parameters: JSON (?) for backend-specific setup

/// Registers a new ZK circuit, downloads artifacts, generates keys, and stores metadata.
// #[debug_job] // Cannot use with generics
pub async fn register_circuit<K: KeyType>(
    Context(ctx): Context<CosnarksContext<K>>,
    CallId(call_id): CallId,
    TangleArgs4(name, circuit_type, proving_backend, artifact_url_str): TangleArgs4<
        String,
        CircuitType,
        ProvingBackend,
        String, // artifact_url
                // Add OptionalJsonParams here if TangleArgs5 is needed
    >,
    // setup_params: OptionalJsonParams,
) -> Result<TangleResult<([u8; 32], [u8; 20], Vec<u8>)>>
where
    K::Public: Ord + Unpin + std::hash::Hash + Send + Sync,
{
    // Return standard types
    info!(%call_id, %name, ?circuit_type, ?proving_backend, %artifact_url_str, "Registering circuit");

    // --- Validation ---
    validate_backend_compatibility(&circuit_type, &proving_backend)?;

    // --- Circuit ID Generation ---
    let circuit_id = generate_circuit_id(&name, &circuit_type, &proving_backend);
    let circuit_id_hex = hex::encode(circuit_id);
    info!(%circuit_id_hex, "Generated circuit ID");

    // --- Artifact Download ---
    let artifact_url = Url::parse(&artifact_url_str).map_err(Error::UrlParseError)?;
    debug!(url = %artifact_url, "Downloading artifact...");
    let artifact_data = download_artifact(&artifact_url).await?;
    debug!(
        "Artifact downloaded successfully ({} bytes)",
        artifact_data.len()
    );

    // --- Key Generation (Placeholder) ---
    // In a real implementation, this would call co-circom/co-noir based on type/backend
    // to generate PK and VK from the downloaded artifact_data.
    info!(%circuit_id_hex, "Generating proving and verification keys (Placeholder)...", );
    let (proving_key_data, verification_key_data, verifier_address) =
        generate_keys_placeholder(&circuit_type, &proving_backend, &artifact_data)?;
    debug!(
        "Keys generated (PK: {} bytes, VK: {} bytes)",
        proving_key_data.len(),
        verification_key_data.len()
    );

    // --- Artifact Storage ---
    let artifact_store = ctx.circuit_store();
    let artifacts_base_path = artifact_store.get_artifacts_base_path();
    let circuit_artifact_dir = artifacts_base_path.join(&circuit_id_hex);

    // Determine artifact file extension based on type
    let artifact_ext = match circuit_type {
        CircuitType::Circom => "r1cs", // Or .json, depends on compilation output
        CircuitType::Noir => "acir",
    };
    let artifact_filename = format!("{}.{}", ARTIFACT_FILENAME, artifact_ext);

    // Define relative paths for storing in CircuitInfo
    let artifact_rel_path = PathBuf::from(&artifact_filename);
    let pk_rel_path = PathBuf::from(PROVING_KEY_FILENAME);
    let vk_rel_path = PathBuf::from(VERIFICATION_KEY_FILENAME);

    let circuit_info = CircuitInfo {
        id: circuit_id,
        name: name.clone(),
        circuit_type,
        proving_backend,
        artifact_path: artifact_rel_path.clone(), // Store relative path
        proving_key_path: pk_rel_path.clone(),    // Store relative path
        verification_key_path: vk_rel_path.clone(), // Store relative path
        verifier_address,                         // Store optional verifier address
    };

    // Store artifacts and info
    debug!(dir = ?circuit_artifact_dir, "Storing artifacts...");
    artifact_store.store_circuit_artifacts(
        &circuit_id_hex,
        &artifact_filename,
        &artifact_data,
        PROVING_KEY_FILENAME,
        &proving_key_data,
        VERIFICATION_KEY_FILENAME,
        &verification_key_data,
    )?;
    artifact_store.store_circuit_info(&circuit_id_hex, &circuit_info)?;
    info!(%circuit_id_hex, "Circuit artifacts and info stored successfully.");

    // --- Prepare Result for Solidity ---
    let result_verifier_addr_bytes = verifier_address.unwrap_or_default(); // Use default if None

    Ok(TangleResult((
        circuit_id,
        result_verifier_addr_bytes,
        verification_key_data,
    )))
}

/// Validates if the chosen proving backend is compatible with the circuit type.
fn validate_backend_compatibility(
    circuit_type: &CircuitType,
    proving_backend: &ProvingBackend,
) -> Result<()> {
    match (circuit_type, proving_backend) {
        (CircuitType::Circom, ProvingBackend::Groth16) => Ok(()),
        (CircuitType::Circom, ProvingBackend::Plonk) => Ok(()),
        (CircuitType::Noir, ProvingBackend::UltraHonk) => Ok(()),
        _ => Err(Error::IncompatibleBackend(format!(
            "Proving backend {:?} is not compatible with circuit type {:?}",
            proving_backend, circuit_type
        ))),
    }
}

/// Generates a unique CircuitId based on metadata.
fn generate_circuit_id(
    name: &str,
    circuit_type: &CircuitType,
    proving_backend: &ProvingBackend,
) -> CircuitId {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update(format!("{:?}", circuit_type).as_bytes());
    hasher.update(format!("{:?}", proving_backend).as_bytes());
    hasher.finalize().into()
}

/// Downloads artifact data from a given URL.
async fn download_artifact(url: &Url) -> Result<Vec<u8>> {
    let response = reqwest::get(url.clone()).await?;
    if !response.status().is_success() {
        return Err(Error::NetworkError(format!(
            "Failed to download artifact from {}: Status {}",
            url,
            response.status()
        )));
    }
    let bytes = response.bytes().await?.to_vec();
    Ok(bytes)
}

/// Placeholder function for generating keys.
/// TODO: Replace with actual calls to co-circom/co-noir setup functions.
fn generate_keys_placeholder(
    _circuit_type: &CircuitType,
    _proving_backend: &ProvingBackend,
    _artifact_data: &[u8],
) -> Result<(Vec<u8>, Vec<u8>, Option<[u8; 20]>)> {
    // Simulate key generation
    info!("Simulating key generation...");
    let proving_key_data = b"fake_proving_key_data".to_vec();
    let verification_key_data = b"fake_verification_key_data".to_vec();
    // Optionally simulate generating/finding a verifier contract address
    let verifier_address = Some([0u8; 20]);
    Ok((proving_key_data, verification_key_data, verifier_address))
}

// Placeholder/Helper function signatures (implementations needed)
// async fn download_artifact(url: &str) -> Result<Vec<u8>> { ... }
// fn generate_keys(ct: &CircuitType, pb: &ProvingBackend, artifact: &[u8], params: Option<serde_json::Value>) -> Result<(Vec<u8>, Vec<u8>)> { ... }
