use crate::error::{Error, Result};
use crate::types::{CircuitId, CircuitInfo};
use hex;
use sled::Db;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DB_SUBDIR: &str = "circuit_db";
const ARTIFACTS_SUBDIR: &str = "artifacts";
const INFO_TREE_NAME: &[u8] = b"circuit_info";

/// Manages persistent storage for circuit information and artifacts.
#[derive(Debug, Clone)]
pub struct CircuitStore {
    db: Db,
    base_path: PathBuf,
    artifacts_path: PathBuf,
    info_tree: sled::Tree,
}

impl CircuitStore {
    /// Creates or opens a `CircuitStore` rooted at the given base path.
    pub fn new(base_path: PathBuf) -> Result<Self> {
        let db_path = base_path.join(DB_SUBDIR);
        let artifacts_path = base_path.join(ARTIFACTS_SUBDIR);

        fs::create_dir_all(&db_path)?;
        fs::create_dir_all(&artifacts_path)?;

        let db = sled::open(&db_path)
            .map_err(|e| Error::StateError(format!("Failed to open sled DB: {}", e)))?;
        let info_tree = db
            .open_tree(INFO_TREE_NAME)
            .map_err(|e| Error::StateError(format!("Failed to open sled tree: {}", e)))?;

        Ok(Self {
            db,
            base_path,
            artifacts_path,
            info_tree,
        })
    }

    /// Returns the base path where artifacts are stored.
    pub fn get_artifacts_base_path(&self) -> &Path {
        &self.artifacts_path
    }

    /// Stores circuit artifact files in a dedicated directory.
    pub fn store_circuit_artifacts(
        &self,
        circuit_id_hex: &str,
        artifact_filename: &str,
        artifact_data: &[u8],
        pk_filename: &str,
        proving_key_data: &[u8],
        vk_filename: &str,
        verification_key_data: &[u8],
    ) -> Result<()> {
        let circuit_artifact_dir = self.artifacts_path.join(circuit_id_hex);
        fs::create_dir_all(&circuit_artifact_dir)?;

        fs::write(circuit_artifact_dir.join(artifact_filename), artifact_data)?;
        fs::write(circuit_artifact_dir.join(pk_filename), proving_key_data)?;
        fs::write(
            circuit_artifact_dir.join(vk_filename),
            verification_key_data,
        )?;

        Ok(())
    }

    /// Stores circuit information (metadata) in the database.
    /// Uses the hex representation of the CircuitId as the key.
    pub fn store_circuit_info(&self, circuit_id_hex: &str, info: &CircuitInfo) -> Result<()> {
        // Ensure the ID in the info matches the key being used
        if hex::encode(info.id) != circuit_id_hex {
            return Err(Error::Internal(
                "Circuit ID mismatch during storage".to_string(),
            ));
        }

        let info_bytes = bincode::serialize(info)?;
        self.info_tree
            .insert(circuit_id_hex.as_bytes(), info_bytes)
            .map_err(|e| Error::StateError(format!("Failed to insert into sled tree: {}", e)))?;

        self.db
            .flush()
            .map_err(|e| Error::StateError(format!("Failed to flush sled DB: {}", e)))?;
        Ok(())
    }

    /// Retrieves circuit information by its ID (hex representation) from Sled.
    pub fn get_circuit_info(&self, id_hex: &str) -> Result<Option<CircuitInfo>> {
        let info_bytes_opt = self
            .info_tree
            .get(id_hex.as_bytes())
            .map_err(|e| Error::StateError(format!("Failed to read from sled tree: {}", e)))?;

        match info_bytes_opt {
            Some(info_bytes) => {
                let info: CircuitInfo = bincode::deserialize(&info_bytes)?;
                // Optional: Verify info.id matches id_hex if paranoid
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    /// Retrieves the artifact data for a given circuit.
    pub fn get_artifact_data(&self, info: &CircuitInfo) -> Result<Vec<u8>> {
        let full_path = self
            .artifacts_path
            .join(hex::encode(info.id))
            .join(&info.artifact_path);
        fs::read(&full_path).map_err(Error::IoError)
    }

    /// Retrieves the proving key data.
    pub fn get_proving_key_data(&self, info: &CircuitInfo) -> Result<Vec<u8>> {
        let full_path = self
            .artifacts_path
            .join(hex::encode(info.id))
            .join(&info.proving_key_path);
        fs::read(&full_path).map_err(Error::IoError)
    }

    /// Retrieves the verification key data.
    pub fn get_verification_key_data(&self, info: &CircuitInfo) -> Result<Vec<u8>> {
        let full_path = self
            .artifacts_path
            .join(hex::encode(info.id))
            .join(&info.verification_key_path);
        fs::read(&full_path).map_err(Error::IoError)
    }

    // Optional: Add methods for listing circuits (iterating over the tree), removing circuits, etc.
    pub fn list_circuit_ids(&self) -> impl Iterator<Item = Result<CircuitId>> + '_ {
        self.info_tree.iter().keys().map(|key_result| {
            key_result
                .map_err(|e| Error::StateError(format!("Sled key iteration failed: {}", e)))
                .and_then(|key_bytes| {
                    String::from_utf8(key_bytes.to_vec())
                        .map_err(|e| Error::StateError(format!("Invalid UTF8 key in DB: {}", e)))
                })
        })
    }

    pub fn remove_circuit(&self, id: &CircuitId) -> Result<Option<CircuitInfo>> {
        match self
            .info_tree
            .remove(hex::encode(id).as_bytes())
            .map_err(|e| Error::StateError(format!("Sled remove failed: {}", e)))?
        {
            Some(info_bytes) => {
                let info: CircuitInfo =
                    bincode::deserialize(&info_bytes).map_err(|e| Error::BincodeError(e))?;
                // Remove associated artifact files
                let circuit_artifact_dir = self.artifacts_path.join(hex::encode(id));
                if circuit_artifact_dir.exists() {
                    fs::remove_dir_all(&circuit_artifact_dir)?;
                }
                self.db
                    .flush()
                    .map_err(|e| Error::StateError(format!("Failed to flush sled DB: {}", e)))?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }
}
