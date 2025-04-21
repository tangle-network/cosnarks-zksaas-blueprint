use crate::error::{Error, Result};
use crate::p2p::{ConfigExchangeMsg, RevealMsg, mpc_config_exchange};
use blueprint_sdk::crypto::KeyType;
use blueprint_sdk::networking::discovery::peers::VerificationIdentifierKey;
use blueprint_sdk::networking::round_based_compat::RoundBasedNetworkAdapter;
use blueprint_sdk::networking::service_handle::NetworkServiceHandle;
use mpc_net::MpcNetworkHandler;
use mpc_net::config::{NetworkConfig, NetworkConfigFile, NetworkPartyConfig};
use round_based::{MpcParty, PartyIndex};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Manages the creation and lifecycle of MPC network sessions using round-based exchange.
pub struct MpcNetworkManager<K: KeyType + 'static>
where
    K::Public: Ord + Unpin,
{
    // Blueprint network handle for underlying p2p communication
    network_handle: NetworkServiceHandle<K>,
    // Local verification key for the current node
    local_verification_key: VerificationIdentifierKey<K>,
    // Base socket address to bind MPC-Net listeners to (hostname:port)
    // Use a publicly reachable address/DNS name in production
    mpc_listen_dns: SocketAddr,
    // Path to MPC-Net private key
    key_path: PathBuf,
    // Path to MPC-Net certificate
    cert_path: PathBuf,
    // Cache for established MPC handlers, keyed by a unique session identifier
    // (e.g., derived from participants + job id)
    established_handlers: Arc<RwLock<HashMap<String, Arc<MpcNetworkHandler>>>>,
}

impl<K: KeyType + 'static> MpcNetworkManager<K>
where
    K::Public: Ord + Unpin,
{
    /// Create a new MPC network manager
    pub fn new(
        network_handle: NetworkServiceHandle<K>,
        local_verification_key: VerificationIdentifierKey<K>,
        mpc_listen_dns: SocketAddr,
        key_path: PathBuf,
        cert_path: PathBuf,
    ) -> Self {
        Self {
            network_handle,
            local_verification_key,
            mpc_listen_dns,
            key_path,
            cert_path,
            established_handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Establishes an MPC-Net session with a given set of participants for a specific job.
    ///
    /// This method orchestrates the secure exchange of MPC-Net configuration details
    /// using the `mpc_config_exchange` round-based protocol before establishing
    /// the actual `mpc-net` connection.
    ///
    /// # Arguments
    ///
    /// * `session_instance_id`: A unique identifier for this specific MPC session instance
    ///                          (e.g., "proof_job_<job_id>_participants_hash").
    ///                          Used for namespacing the round-based protocol messages.
    /// * `ordered_participants`: A vector of the public keys of *all* participants
    ///                           (including the local node) in a deterministic order.
    ///                           This order determines the `PartyIndex` and MPC ID.
    ///
    /// # Returns
    ///
    /// Returns a shared handle to the established `MpcNetworkHandler`.
    pub async fn establish_mpc_session(
        &self,
        session_instance_id: &str,
        ordered_participants: Vec<K::Public>,
    ) -> Result<Arc<MpcNetworkHandler>> {
        // Check cache first
        if let Some(handler) = self
            .established_handlers
            .read()
            .await
            .get(session_instance_id)
        {
            info!(session_id = %session_instance_id, "Returning cached MPC handler");
            return Ok(handler.clone());
        }

        info!(session_id = %session_instance_id, num_participants = ordered_participants.len(), "Establishing new MPC session");

        // 1. Determine local party index and total number of parties
        let n = ordered_participants.len() as u16;
        if n < 2 {
            return Err(Error::ConfigError(
                "MPC requires at least 2 participants".to_string(),
            ));
        }

        let local_public_key = self.local_verification_key.clone();

        let local_party_index = ordered_participants
            .iter()
            .position(|pk| {
                VerificationIdentifierKey::InstancePublicKey(pk.clone()) == local_public_key
            })
            .ok_or_else(|| {
                Error::ConfigError("Local node not found in participant list".to_string())
            })? as PartyIndex;

        debug!(
            my_index = local_party_index,
            total_parties = n,
            "Determined party info"
        );

        // 2. Prepare local configuration reveal message
        let local_reveal_msg = RevealMsg {
            dns_name: self.mpc_listen_dns.to_string(),
            cert_path: self.cert_path.clone(),
        };

        // 3. Setup round-based network adapter
        // Map PartyIndex (0..n-1) to VerificationIdentifierKey for the adapter
        let party_mapping: HashMap<PartyIndex, VerificationIdentifierKey<K>> = ordered_participants
            .into_iter()
            .enumerate()
            .map(|(idx, pub_key)| {
                (
                    idx as PartyIndex,
                    VerificationIdentifierKey::InstancePublicKey(pub_key),
                )
            })
            .collect();

        let network_adapter = RoundBasedNetworkAdapter::new(
            self.network_handle.clone(),
            local_party_index,
            party_mapping.clone(),
            session_instance_id,
        );
        let mpc_party: MpcParty<ConfigExchangeMsg, _, _> = MpcParty::connected(network_adapter);

        // 4. Execute the configuration exchange protocol
        info!(session_id = %session_instance_id, "Starting MPC config exchange protocol...");
        let verified_configs =
            mpc_config_exchange(mpc_party, local_party_index, n, local_reveal_msg)
                .await
                .map_err(|e| Error::MpcProtocolError(format!("Config exchange failed: {:?}", e)))?;
        info!(session_id = %session_instance_id, "MPC config exchange complete.");

        // 5. Build the final NetworkConfigFile for mpc-net
        let mut parties: Vec<NetworkPartyConfig> = verified_configs
            .into_values()
            .map(|config| config)
            .collect();

        // Ensure parties are sorted by ID (which is the PartyIndex)
        parties.sort_by_key(|p| p.id);

        let mpc_net_config_file = NetworkConfigFile {
            my_id: local_party_index as usize,
            bind_addr: self.mpc_listen_dns.clone(),
            key_path: self.key_path.clone(),
            parties,
            timeout_secs: Some(60), // Increased timeout for potentially slower networks/setup
        };

        debug!(config = ?mpc_net_config_file, "Constructed MPC-Net config file");

        // 6. Establish the actual MPC-Net connection
        let handler = Self::establish_mpc_network_internal(mpc_net_config_file).await?;
        let handler_arc = Arc::new(handler);

        // 7. Cache the handler
        self.established_handlers
            .write()
            .await
            .insert(session_instance_id.to_string(), handler_arc.clone());

        info!(session_id = %session_instance_id, "Successfully established and cached MPC handler");
        Ok(handler_arc)
    }

    /// Internal helper to establish the MPC network connection.
    async fn establish_mpc_network_internal(
        config: NetworkConfigFile,
    ) -> Result<MpcNetworkHandler> {
        debug!("Converting NetworkConfigFile to NetworkConfig...");
        let network_config = NetworkConfig::try_from(config).map_err(|e| {
            Error::ConfigError(format!("Failed to create MPC network config: {}", e))
        })?;

        info!("Establishing MPC-Net connection...");
        let handler = MpcNetworkHandler::establish(network_config)
            .await
            .map_err(|e| Error::NetworkError(format!("Failed to establish MPC network: {}", e)))?;
        info!("MPC-Net connection established.");
        Ok(handler)
    }
}
