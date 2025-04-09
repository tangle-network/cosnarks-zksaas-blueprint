use crate::error::{Error, Result};
use crate::types::{CircuitId, CircuitInfo};
use sled::Db;
use std::fs;
use std::path::PathBuf;

const DB_SUBDIR: &str = "circuit_db";
const ARTIFACTS_SUBDIR: &str = "artifacts";
const INFO_TREE_NAME: &[u8] = b"circuit_info";

/// Manages persistent storage for circuit information and artifacts using Sled.
///
/// Stores circuit metadata in a Sled database and large artifacts/keys on the filesystem
/// within the blueprint's designated data directory.
#[derive(Debug, Clone)] // Clone is cheap because Db is Arc-based
pub struct CircuitStore {
    db: Db,
    artifacts_path: PathBuf,
    info_tree: sled::Tree,
}

impl CircuitStore {
    /// Creates or opens a `CircuitStore` rooted at the given base path.
    ///
    /// Creates necessary subdirectories and opens the Sled database.
    pub fn new(base_path: PathBuf) -> Result<Self> {
        let db_path = base_path.join(DB_SUBDIR);
        let artifacts_path = base_path.join(ARTIFACTS_SUBDIR);

        fs::create_dir_all(&db_path).map_err(Error::IoError)?;
        fs::create_dir_all(&artifacts_path).map_err(Error::IoError)?;

        let db = sled::open(&db_path)
            .map_err(|e| Error::StateError(format!("Failed to open sled DB: {}", e)))?;
        let info_tree = db
            .open_tree(INFO_TREE_NAME)
            .map_err(|e| Error::StateError(format!("Failed to open sled tree: {}", e)))?;

        Ok(Self {
            db,
            artifacts_path,
            info_tree,
        })
    }

    /// Stores circuit information and writes associated data files.
    ///
    /// Metadata is stored in Sled. Artifact/Key data is written to the filesystem.
    pub fn store_circuit(
        &self,
        info: &CircuitInfo,
        artifact_data: &[u8],
        proving_key_data: &[u8],
        verification_key_data: &[u8],
    ) -> Result<()> {
        // Define paths for the files within the artifacts directory
        let circuit_artifact_dir = self.get_circuit_artifact_dir(&info.id);
        fs::create_dir_all(&circuit_artifact_dir)?;

        // Note: We modify the paths within the info struct to be relative to the artifacts dir
        // before saving the info struct itself.
        let mut info_to_store = info.clone();

        let artifact_filename = info
            .artifact_path
            .file_name()
            .ok_or_else(|| Error::InvalidInput("Artifact path must have a filename".to_string()))?;
        let pk_filename = info.proving_key_path.file_name().ok_or_else(|| {
            Error::InvalidInput("Proving key path must have a filename".to_string())
        })?;
        let vk_filename = info.verification_key_path.file_name().ok_or_else(|| {
            Error::InvalidInput("Verification key path must have a filename".to_string())
        })?;

        let artifact_rel_path = PathBuf::from(info.id.clone()).join(artifact_filename);
        let pk_rel_path = PathBuf::from(info.id.clone()).join(pk_filename);
        let vk_rel_path = PathBuf::from(info.id.clone()).join(vk_filename);

        info_to_store.artifact_path = artifact_rel_path.clone();
        info_to_store.proving_key_path = pk_rel_path.clone();
        info_to_store.verification_key_path = vk_rel_path.clone();

        // Write files to disk
        fs::write(self.artifacts_path.join(&artifact_rel_path), artifact_data)?;
        fs::write(self.artifacts_path.join(&pk_rel_path), proving_key_data)?;
        fs::write(
            self.artifacts_path.join(&vk_rel_path),
            verification_key_data,
        )?;

        // Store metadata in Sled
        let info_bytes = bincode::serialize(&info_to_store).map_err(|e| Error::BincodeError(e))?; // Use From impl in error.rs
        self.info_tree
            .insert(info.id.as_bytes(), info_bytes)
            .map_err(|e| Error::StateError(format!("Failed to insert into sled tree: {}", e)))?;

        // Ensure data is written to disk
        self.db
            .flush()
            .map_err(|e| Error::StateError(format!("Failed to flush sled DB: {}", e)))?;

        Ok(())
    }

    /// Retrieves circuit information by its ID from Sled.
    pub fn get_circuit_info(&self, id: &CircuitId) -> Result<Option<CircuitInfo>> {
        let info_bytes_opt = self
            .info_tree
            .get(id.as_bytes())
            .map_err(|e| Error::StateError(format!("Failed to read from sled tree: {}", e)))?;

        match info_bytes_opt {
            Some(info_bytes) => {
                let info: CircuitInfo =
                    bincode::deserialize(&info_bytes).map_err(|e| Error::BincodeError(e))?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    /// Retrieves the artifact data for a given circuit by reading from the filesystem path stored in its info.
    pub fn get_artifact_data(&self, info: &CircuitInfo) -> Result<Vec<u8>> {
        let full_path = self.artifacts_path.join(&info.artifact_path);
        fs::read(&full_path).map_err(Error::IoError)
    }

    /// Retrieves the proving key data.
    pub fn get_proving_key_data(&self, info: &CircuitInfo) -> Result<Vec<u8>> {
        let full_path = self.artifacts_path.join(&info.proving_key_path);
        fs::read(&full_path).map_err(Error::IoError)
    }

    /// Retrieves the verification key data.
    pub fn get_verification_key_data(&self, info: &CircuitInfo) -> Result<Vec<u8>> {
        let full_path = self.artifacts_path.join(&info.verification_key_path);
        fs::read(&full_path).map_err(Error::IoError)
    }

    /// Helper to get the directory path for a specific circuit's artifacts.
    fn get_circuit_artifact_dir(&self, id: &CircuitId) -> PathBuf {
        // Use a simple directory structure based on the ID. Consider hashing if IDs can be very long or complex.
        self.artifacts_path.join(id)
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
            .remove(id.as_bytes())
            .map_err(|e| Error::StateError(format!("Sled remove failed: {}", e)))?
        {
            Some(info_bytes) => {
                let info: CircuitInfo =
                    bincode::deserialize(&info_bytes).map_err(|e| Error::BincodeError(e))?;
                // Remove associated artifact files
                let circuit_artifact_dir = self.get_circuit_artifact_dir(id);
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

// Note: Error handling for sled operations might need refinement (e.g., mapping sled::Error variants).
// Ensure proper handling of potential data corruption or inconsistencies.
