use blueprint_sdk::env::BlueprintEnvironment;
use blueprint_sdk::logger;
use blueprint_sdk::runner::BlueprintRunner;
use blueprint_sdk::tangle::config::TangleConfig;
use blueprint_sdk::tangle::context::TangleLayer;
use blueprint_sdk::tangle::crypto::TanglePairSigner;
use blueprint_sdk::tangle::net::{TangleConsumer, TangleProducer};
use blueprint_sdk::tangle::router::Router;
use color_eyre::{Result, eyre::Context};
use cosnarks_zksaas_blueprint_lib::context::CosnarksContext;
use cosnarks_zksaas_blueprint_lib::jobs::{
    GENERATE_PROOF_JOB_ID, REGISTER_CIRCUIT_JOB_ID, generate_proof, register_circuit,
};
use rcgen::CertifiedKey;
use sp_core::sr25519::Pair;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

// Define default paths relative to the config/data directory
const MPC_CERT_FILENAME: &str = "mpc_cert.der";
const MPC_KEY_FILENAME: &str = "mpc_key.der";
// Default base port for MPC net binding
const DEFAULT_MPC_BASE_PORT: u16 = 10000;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging and error handling
    logger::init(Default::default())?;
    color_eyre::install()?; // Optional: Better panic messages

    // Load environment variables (Tangle RPC, keystore path, data dir, etc.)
    let env = BlueprintEnvironment::load()?;

    // Initialize the signing key from the keystore
    let signer_key = env
        .keystore()
        .first_local::<Pair>() // Use sr25519::Pair
        .map_err(|e| eyre::eyre!("Failed to get local signer key: {}", e))?;
    let secret_pair = env
        .keystore()
        .get_secret::<Pair>(&signer_key)
        .map_err(|e| eyre::eyre!("Failed to get secret for signer key: {}", e))?;
    let signer = TanglePairSigner::new(secret_pair.0);

    // Determine MPC Net paths and generate cert/key if needed
    let mpc_net_dir = env
        .config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mpc_net");
    std::fs::create_dir_all(&mpc_net_dir).context("Creating mpc_net directory")?;
    let cert_path = mpc_net_dir.join(MPC_CERT_FILENAME);
    let key_path = mpc_net_dir.join(MPC_KEY_FILENAME);
    let base_bind_addr_str = env
        .protocol_settings
        .experimental
        .get("mpc_base_bind_addr") // Example: Get base bind addr from experimental config
        .map(|v| v.as_str().unwrap_or("0.0.0.0"))
        .unwrap_or("0.0.0.0");
    let base_port = env
        .protocol_settings
        .experimental
        .get("mpc_base_port")
        .and_then(|v| v.as_integer()) // Expecting integer port
        .map(|p| p as u16)
        .unwrap_or(DEFAULT_MPC_BASE_PORT);

    let base_bind_addr: SocketAddr = format!("{}:{}", base_bind_addr_str, base_port)
        .parse()
        .context("Parsing base MPC bind address")?;

    // Generate cert/key only if they don't exist
    if !cert_path.exists() || !key_path.exists() {
        generate_mpc_cert(&cert_path, &key_path)?;
    }

    // Initialize Tangle client, producer, and consumer
    let client = env.tangle_client().await?;
    let producer = TangleProducer::finalized_blocks(client.rpc_client.clone()).await?;
    let consumer = TangleConsumer::new(client.rpc_client.clone(), signer);

    // Initialize the custom context
    let context = CosnarksContext::new(env.clone()).await?;

    // Configure the router, mapping job IDs to handlers
    let router = Router::new()
        // Apply TangleLayer to enforce standard Tangle job context requirements
        .route(REGISTER_CIRCUIT_JOB_ID, register_circuit.layer(TangleLayer))
        .route(GENERATE_PROOF_JOB_ID, generate_proof.layer(TangleLayer))
        .with_context(context); // Pass the shared context to all routes

    // Build and run the Blueprint
    tracing::info!("Starting CoSNARKs zkSaaS Blueprint...");
    BlueprintRunner::builder(TangleConfig::default(), env)
        .router(router)
        .producer(producer)
        .consumer(consumer)
        // Add .background_service or .with_shutdown_handler if needed later
        .run()
        .await?;

    Ok(())
}

/// Generates a self-signed certificate and private key for MPC-Net TLS.
fn generate_mpc_cert(cert_path: &Path, key_path: &Path) -> Result<()> {
    tracing::info!(cert_path = %cert_path.display(), key_path = %key_path.display(), "Generating self-signed MPC certificate and key...");
    // Use common names relevant to the service, or just localhost for simple cases
    // SANS are important for TLS verification
    let sans = vec!["localhost".to_string()];
    let CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(sans).context("generating self-signed cert")?;
    let key = key_pair.serialize_der();
    std::fs::write(key_path, key).context("writing key file")?;
    let cert_pem = cert.pem(); // Save PEM for easier inspection if needed
    std::fs::write(cert_path.with_extension("pem"), cert_pem).context("writing cert PEM file")?;
    let cert_der = cert.der(); // Save DER as expected by mpc-net
    std::fs::write(cert_path, cert_der).context("writing certificate DER file")?;
    tracing::info!("MPC certificate and key generated successfully.");
    Ok(())
}
