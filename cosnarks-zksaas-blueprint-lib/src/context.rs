use crate::error::{Error, Result};
use crate::network::MpcNetworkManager;
use crate::state::CircuitStore;
use blueprint_sdk::clients::GadgetServicesClient;
use blueprint_sdk::contexts::tangle::TangleClientContext;
use blueprint_sdk::crypto::{BytesEncoding, KeyType};
use blueprint_sdk::macros::context::{KeystoreContext, ServicesContext, TangleClientContext};
use blueprint_sdk::networking::service_handle::NetworkServiceHandle;
use blueprint_sdk::runner::config::BlueprintEnvironment;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

/// Main context for the zkSaaS Blueprint service
pub struct CosnarksContext<K: KeyType>
where
    K::Public: Unpin,
{
    /// The shared Blueprint environment
    pub environment: Arc<BlueprintEnvironment>,
    /// Store for circuit metadata and artifact paths
    pub circuit_store: CircuitStore,
    /// The MPC network manager for coordinating multi-party computations
    pub mpc_network_manager: Arc<MpcNetworkManager<K>>,
}

impl<K: KeyType> CosnarksContext<K>
where
    K::Public: Unpin,
{
    /// Create a new CosnarksContext
    pub async fn new(environment: Arc<BlueprintEnvironment>) -> Result<Self> {
        let data_dir = environment.data_dir.as_ref().ok_or_else(|| {
            Error::MissingConfiguration(
                "Data directory (data_dir) must be set in Blueprint environment".to_string(),
            )
        })?;

        // Create circuit store
        let circuit_store = CircuitStore::new(data_dir.clone())?;

        // -- Networking Setup --
        // Define a unique protocol name for this service
        let protocol_name = "/cosnarks-zksaas/mpc/1.0.0"; // Example protocol ID
        let network_config = environment
            .libp2p_network_config(protocol_name, false)
            .map_err(Into::<blueprint_sdk::Error>::into)?;

        // TODO: Fetch allowed keys dynamically if needed, e.g., from Tangle
        // For now, assume AllowAll or configuration via environment
        let allowed_keys = blueprint_sdk::networking::AllowedKeys::default();
        let (allowed_keys_tx, allowed_keys_rx) = crossbeam_channel::unbounded(); // Required by libp2p_start_network

        let network_handle = environment
            .libp2p_start_network(network_config, allowed_keys, allowed_keys_rx)
            .map_err(Into::<blueprint_sdk::Error>::into)?;

        // -- MPC Network Manager Setup --
        // These should ideally come from secure configuration
        let mpc_listen_dns: SocketAddr = std::env::var("MPC_LISTEN_DNS")
            .map_err(|_| {
                Error::MissingConfiguration(
                    "MPC_LISTEN_DNS environment variable not set".to_string(),
                )
            })?
            .parse()
            .map_err(|_| Error::InvalidInput("Invalid MPC_LISTEN_DNS format".to_string()))?;
        let key_path = data_dir.join(
            std::env::var("MPC_KEY_PATH").unwrap_or_else(|_| "mpc_certs/mpc_key.der".to_string()),
        );
        let cert_path = data_dir.join(
            std::env::var("MPC_CERT_PATH").unwrap_or_else(|_| "mpc_certs/mpc_cert.der".to_string()),
        );

        // Ensure certificates directory exists
        if let Some(parent) = key_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = cert_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // TODO: Add check if key/cert files actually exist?

        let mpc_network_manager = Arc::new(MpcNetworkManager::new(
            network_handle,
            mpc_listen_dns,
            key_path,
            cert_path,
        ));

        Ok(Self {
            environment,
            circuit_store,
            mpc_network_manager,
        })
    }

    /// Provides immutable access to the CircuitStore.
    pub fn circuit_store(&self) -> &CircuitStore {
        &self.circuit_store
    }

    /// Provides immutable access to the MpcNetworkManager.
    pub fn mpc_network_manager(&self) -> &Arc<MpcNetworkManager<K>> {
        &self.mpc_network_manager
    }

    /// Provides access to the configured data directory.
    pub fn data_dir(&self) -> Option<PathBuf> {
        self.environment.data_dir.clone()
    }

    /// Retrieves the list of registered operator public keys for the service.
    /// TODO: Implement actual fetching from Tangle state.
    pub async fn get_operators(&self) -> Result<Vec<K::Public>> {
        let operators = self
            .environment
            .tangle_client()
            .await
            .map_err(Into::<blueprint_sdk::Error>::into)?
            .get_operators()
            .await
            .map_err(Into::<blueprint_sdk::Error>::into)?
            .values()
            .map(|k| K::Public::from_bytes(&k.0).unwrap())
            .collect();

        Ok(operators)
    }
}
