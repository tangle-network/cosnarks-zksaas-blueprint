// Protocol for exchanging MPC-Net configuration details securely.

use crate::error::{Error as CoSnarksError, Result};
use mpc_net::config::{Address, NetworkPartyConfig};
use round_based::rounds_router::{RoundsRouter, simple_store::RoundInput};
use round_based::{Delivery, Mpc, MpcParty, MsgId, Outgoing, PartyIndex, ProtocolMessage, SinkExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;

/// Protocol messages for MPC configuration exchange
#[derive(Clone, Debug, PartialEq, ProtocolMessage, Serialize, Deserialize)]
pub enum ConfigExchangeMsg {
    /// Round 1: Commit to the configuration details
    Commit(CommitMsg),
    /// Round 2: Reveal the configuration details
    Reveal(RevealMsg),
}

/// Round 1: Commitment message
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommitMsg {
    pub commitment: [u8; 32],
}

/// Round 2: Reveal message containing the actual configuration part
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RevealMsg {
    // Contains the information needed for NetworkPartyConfig, excluding the ID
    pub dns_name: String,
    pub cert_path: PathBuf,
}

/// Executes the secure MPC configuration exchange protocol.
///
/// Each party commits to their `NetworkPartyConfig` info (excluding ID),
/// then reveals it. The protocol verifies consistency and returns a map
/// of `PartyIndex` to the verified `(ParticipantId, NetworkPartyConfig)`.
#[tracing::instrument(skip(party, local_config))]
pub async fn mpc_config_exchange<M>(
    party: M,
    i: PartyIndex,
    n: u16,
    local_config: RevealMsg,
) -> Result<HashMap<PartyIndex, NetworkPartyConfig>>
where
    M: Mpc<ProtocolMessage = ConfigExchangeMsg>,
{
    let MpcParty { delivery, .. } = party.into_party();
    let (incoming, mut outgoing) = delivery.split();

    // Define rounds
    let mut rounds = RoundsRouter::<ConfigExchangeMsg>::builder();
    let round1 = rounds.add_round(RoundInput::<CommitMsg>::broadcast(i, n));
    let round2 = rounds.add_round(RoundInput::<RevealMsg>::broadcast(i, n));
    let mut rounds = rounds.listen(incoming);

    // --- The Protocol ---

    // 1. Serialize local config for commitment
    let local_config_bytes = bincode::serialize(&local_config)?;

    // 2. Commit to the config (hash of serialized RevealMsg)
    let commitment = Sha256::digest(&local_config_bytes);
    tracing::debug!(commitment = %hex::encode(commitment), "Committed local config");
    outgoing
        .send(Outgoing::broadcast(ConfigExchangeMsg::Commit(CommitMsg {
            commitment: commitment.into(),
        })))
        .await
        .map_err(|e| CoSnarksError::ExchangeRoundBasedError(e.to_string()))?;

    tracing::debug!("Sent commitment, waiting for others...");

    // 3. Receive commitments from other parties
    let commitments = rounds
        .complete(round1)
        .await
        .map_err(|e| CoSnarksError::ExchangeRoundBasedError(e.to_string()))?;
    tracing::debug!("Received all commitments");

    // 4. Reveal local config
    tracing::debug!("Revealing local config");
    outgoing
        .send(Outgoing::broadcast(ConfigExchangeMsg::Reveal(
            local_config.clone(),
        )))
        .await
        .map_err(|e| CoSnarksError::ExchangeRoundBasedError(e.to_string()))?;
    tracing::debug!("Sent revealed config, waiting for others...");

    // 5. Receive revealed configs, verify against commitments
    let revealed_configs = rounds
        .complete(round2)
        .await
        .map_err(|e| CoSnarksError::ExchangeRoundBasedError(e.to_string()))?;
    tracing::debug!("Received all revealed configs");

    let mut guilty_parties = vec![];
    let mut party_configs = HashMap::with_capacity(n as usize);

    // Parse local dns_name into Address struct expected by NetworkPartyConfig
    let local_address = parse_dns_name(&local_config.dns_name)?;

    // Add self to the map first
    party_configs.insert(i, NetworkPartyConfig {
        id: i as usize,
        dns_name: local_address,
        cert_path: local_config.cert_path.clone(),
    });

    for ((party_idx, commit_msg_id, commit), (_, reveal_msg_id, revealed)) in commitments
        .into_iter_indexed()
        .zip(revealed_configs.into_iter_indexed())
    {
        // Skip self as we've already processed our own data
        if party_idx == i {
            continue;
        }

        // Verify commitment
        let revealed_bytes = bincode::serialize(&revealed)?;
        let commitment_expected = Sha256::digest(&revealed_bytes);

        if commit.commitment.to_vec() != commitment_expected.to_vec() {
            tracing::warn!(%party_idx, "Commitment mismatch");
            guilty_parties.push(Blame {
                guilty_party: party_idx,
                commitment_msg: commit_msg_id,
                reveal_msg: reveal_msg_id,
            });
            continue;
        }

        // Parse revealed dns_name into Address struct
        let revealed_address = parse_dns_name(&revealed.dns_name)?;

        // Store verified config
        let party_config = NetworkPartyConfig {
            id: party_idx as usize,
            dns_name: revealed_address,
            cert_path: revealed.cert_path,
        };
        party_configs.insert(party_idx, party_config);
    }

    if guilty_parties.is_empty() {
        tracing::info!("MPC Config Exchange protocol completed successfully.");
        Ok(party_configs)
    } else {
        tracing::error!(
            ?guilty_parties,
            "MPC Config Exchange failed due to cheating parties."
        );
        Err(CoSnarksError::CommitmentMismatch { guilty_parties })
    }
}

/// Helper to parse "hostname:port" string into mpc_net::config::Address
fn parse_dns_name(dns_name: &str) -> Result<Address> {
    let parts: Vec<&str> = dns_name.split(':').collect();
    if parts.len() != 2 {
        return Err(CoSnarksError::InvalidDnsName(format!(
            "Invalid format, expected hostname:port, got {}",
            dns_name
        )));
    }
    let hostname = parts[0].to_string();
    let port = parts[1]
        .parse::<u16>()
        .map_err(|e| CoSnarksError::InvalidDnsName(format!("Invalid port number: {}", e)))?;
    Ok(Address { hostname, port })
}

/// Blame information for a misbehaving party
#[derive(Debug, Serialize, Deserialize)]
pub struct Blame {
    pub guilty_party: PartyIndex,
    pub commitment_msg: MsgId,
    pub reveal_msg: MsgId,
}

// --- Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use blueprint_sdk::crypto::KeyType;
    use blueprint_sdk::crypto::sp_core::SpEcdsa;
    use blueprint_sdk::networking::AllowedKeys;
    use blueprint_sdk::networking::discovery::peers::VerificationIdentifierKey;
    use blueprint_sdk::networking::round_based_compat::RoundBasedNetworkAdapter;
    use blueprint_sdk::networking::test_utils::{TestNode, wait_for_peer_discovery};
    use blueprint_sdk::networking::types::ParticipantId;
    use blueprint_sdk::testing::utils::setup_log;
    use std::path::Path;
    use std::time::Duration;
    use tempfile::tempdir;
    use tokio::fs;

    // Helper to create dummy cert/key paths for testing
    async fn create_dummy_certs(dir: &Path, party_idx: usize) -> (PathBuf, PathBuf) {
        let key_path = dir.join(format!("key_{}.der", party_idx));
        let cert_path = dir.join(format!("cert_{}.der", party_idx));
        fs::write(&key_path, format!("key_data_{}", party_idx))
            .await
            .unwrap();
        fs::write(&cert_path, format!("cert_data_{}", party_idx))
            .await
            .unwrap();
        (key_path, cert_path)
    }

    #[tokio::test]
    async fn test_config_exchange_simulation() {
        setup_log();
        let n: u16 = 3;
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        let configs: Vec<_> = (0..n)
            .map(|i| {
                let keypair = SpEcdsa::generate_with_seed(None).unwrap();
                RevealMsg {
                    dns_name: format!("party_{}.example.com:900{}", i, i),
                    cert_path: base_path.join(format!("cert_{}.der", i)),
                }
            })
            .collect();

        let results: Vec<Result<HashMap<u16, NetworkPartyConfig>>> =
            round_based::sim::run_with_setup(
                configs.clone(), // Each party gets its own config to reveal
                |i, party, config| mpc_config_exchange(party, i, n, config),
            )
            .unwrap()
            .0;

        assert_eq!(results.len(), n as usize);
        for i in 0..n {
            let party_conf = results.get(i as usize).unwrap().clone().unwrap();
            let expected_address = parse_dns_name(&configs[i as usize].dns_name).unwrap();
            assert_eq!(party_conf.get(&i).unwrap().id, i as usize);
            assert_eq!(party_conf.get(&i).unwrap().dns_name, expected_address);
            assert_eq!(
                party_conf.get(&i).unwrap().cert_path,
                configs[i as usize].cert_path
            );
        }
        tracing::info!("Simulation test passed.");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_config_exchange_p2p() {
        setup_log();
        let n: u16 = 2;
        let network_name = "config-exchange-test-p2p"; // Unique network name
        let instance_id = "instance-p2p-1"; // Unique instance ID
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        // Create nodes and their keys/certs
        let mut nodes = Vec::new();
        let mut handles = Vec::new();
        let mut configs = Vec::new();

        for i in 0..n {
            let node_dir = base_path.join(format!("node_{}", i));
            fs::create_dir_all(&node_dir).await.unwrap();
            let (_key_path, cert_path) = create_dummy_certs(&node_dir, i as usize).await;
            let node = TestNode::<SpEcdsa>::new(
                network_name,
                instance_id,
                AllowedKeys::default(),
                vec![],
                false,
            );
            configs.push(RevealMsg {
                dns_name: format!("127.0.0.1:900{}", i),
                cert_path,
            });
            nodes.push(node);
        }

        // Start nodes
        for node in nodes.iter_mut() {
            let handle = node.start().await.expect("Failed to start node");
            handles.push(handle);
        }

        // Wait for initial peer discovery
        wait_for_peer_discovery(&handles, Duration::from_secs(10))
            .await
            .unwrap();
        tracing::info!("Peer discovery complete");

        // Setup round-based network adapters
        let parties: HashMap<PartyIndex, VerificationIdentifierKey<SpEcdsa>> = (0..n)
            .map(|i| {
                (
                    i,
                    VerificationIdentifierKey::InstancePublicKey(
                        nodes[i as usize].instance_key_pair.public(),
                    ),
                )
            })
            .collect();
        tracing::info!(?parties, "Party mapping setup");

        let mut tasks = vec![];
        for i in 0..n {
            let handle = handles[i as usize].clone();
            let parties_clone = parties.clone();
            let config_clone = configs[i as usize].clone();
            let task = tokio::spawn(async move {
                tracing::info!(party_index = i, "Spawning protocol task");
                let network = RoundBasedNetworkAdapter::new(handle, i, parties_clone, instance_id);
                let mpc_party = MpcParty::connected(network);
                mpc_config_exchange(mpc_party, i, n, config_clone).await
            });
            tasks.push(task);
        }

        // Wait for protocols to complete
        tracing::info!("Waiting for protocol tasks to complete...");
        let results = futures::future::join_all(tasks).await;
        tracing::info!("All protocol tasks completed");

        let mut final_configs = Vec::new();
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(Ok(config_map)) => {
                    tracing::info!(party_index = i, "Protocol completed successfully");
                    final_configs.push(config_map);
                }
                Ok(Err(e)) => panic!("Party {} protocol error: {:?}", i, e),
                Err(e) => panic!("Party {} task join error: {:?}", i, e),
            }
        }

        // Verify all parties have the same final config map
        assert!(
            !final_configs.is_empty(),
            "No configurations were collected"
        );
        let first_map = final_configs[0].clone();
        assert_eq!(
            first_map.len(),
            n as usize,
            "Expected {} parties in config map, found {}",
            n,
            first_map.len()
        );

        for (idx, other_map) in final_configs.iter().enumerate().skip(1) {
            assert_eq!(
                &first_map, other_map,
                "Config maps differ between party 0 and party {}",
                idx
            );
        }

        // Basic check on the content
        for i in 0..n {
            let party_conf = first_map
                .get(&i)
                .unwrap_or_else(|| panic!("Missing config for party {}", i));
            let expected_address = parse_dns_name(&configs[i as usize].dns_name).unwrap();
            assert_eq!(party_conf.id, i as usize, "MPC ID mismatch for party {}", i);
            assert_eq!(
                party_conf.dns_name, expected_address,
                "DNS name mismatch for party {}",
                i
            );
            assert_eq!(
                party_conf.cert_path, configs[i as usize].cert_path,
                "Cert path mismatch for party {}",
                i
            );
        }

        tracing::info!("P2P Config Exchange test passed.");
    }
}
