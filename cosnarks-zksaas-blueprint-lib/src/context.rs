use crate::error::Result;
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

#[derive(Clone, KeystoreContext, ServicesContext, TangleClientContext)]
pub struct CosnarksContext<K: KeyType> {
    /// The shared Blueprint environment
    #[config]
    env: BlueprintEnvironment,
    /// Store for circuit metadata
    circuit_store: CircuitStore,
    /// The MPC network manager for coordinating multi-party computations
    mpc_network_manager: Arc<Mutex<MpcNetworkManager<K>>>,
}

impl<K: KeyType> CosnarksContext<K> {
    /// Create a new CosnarksContext
    pub async fn new(
        env: BlueprintEnvironment,
        network_handle: NetworkServiceHandle<K>,
    ) -> Result<Self> {
        // Create circuit store
        let base_path = env.data_dir.clone().unwrap_or_else(|| PathBuf::from("."));
        let circuit_store = CircuitStore::new(base_path.clone())?;

        // Set up network manager with default values
        // Use a base port of 9000 for MPC-Net
        let base_bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9000);

        // Ensure certificates directory exists
        let cert_dir = base_path.join("mpc_certs");
        std::fs::create_dir_all(&cert_dir)?;

        // Paths to key and cert
        let key_path = cert_dir.join("mpc_key.der");
        let cert_path = cert_dir.join("mpc_cert.der");

        // Create the MPC network manager
        let mpc_network_manager = MpcNetworkManager::new(
            network_handle,
            base_bind_addr.to_string(),
            key_path,
            cert_path,
        );

        Ok(Self {
            env,
            circuit_store,
            mpc_network_manager: Arc::new(Mutex::new(mpc_network_manager)),
        })
    }

    /// Access the data directory
    pub fn data_dir(&self) -> Option<PathBuf> {
        self.env.data_dir.clone()
    }

    /// Get the circuit store
    pub fn circuit_store(&self) -> &CircuitStore {
        &self.circuit_store
    }

    /// Get the MPC network manager
    pub fn mpc_network_manager(&self) -> &Arc<Mutex<MpcNetworkManager<K>>> {
        &self.mpc_network_manager
    }

    /// Get allowed operators for a circuit
    pub async fn get_operators(&self) -> Result<Vec<K::Public>> {
        let operators = self
            .env
            .tangle_client()
            .await?
            .get_operators()
            .await?
            .values()
            .map(|k| K::Public::from_bytes(&k.0).unwrap())
            .collect();

        Ok(operators)
    }
}
