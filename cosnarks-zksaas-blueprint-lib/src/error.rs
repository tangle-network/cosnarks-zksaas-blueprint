use co_noir::ParseAddressError;
use thiserror::Error;

use crate::p2p::Blame;

/// Comprehensive error type for the zkSaaS Blueprint.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Blueprint SDK error: {0}")]
    BlueprintSdkError(#[from] blueprint_sdk::Error),

    #[error("Blueprint SDK clients error: {0}")]
    BlueprintSdkClientsError(#[from] blueprint_sdk::clients::Error),

    #[error("Blueprint SDK tangle client error: {0}")]
    BlueprintSdkTangleClientError(#[from] blueprint_sdk::clients::tangle::error::Error),

    #[error("Exchange round-based error: {0}")]
    ExchangeRoundBasedError(String),

    #[error("Exchange commitment mismatch: {guilty_parties:?}")]
    CommitmentMismatch { guilty_parties: Vec<Blame> },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Configuration parsing error (TOML): {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Configuration serialization error (TOML): {0}")]
    TomlSerError(#[from] toml::ser::Error),

    #[error("Serialization error (JSON): {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Serialization error (Bincode): {0}")]
    BincodeError(#[from] Box<bincode::ErrorKind>),

    #[error("Hex decoding error: {0}")]
    HexError(#[from] hex::FromHexError),

    #[error("URL parsing error: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Networking error: {0}")]
    NetworkError(String),

    #[error("MPC Networking error: {0}")]
    MpcNetParseError(#[from] ParseAddressError),

    // --- General ZK/MPC errors ---
    #[error("State management error: {0}")]
    StateError(String),

    #[error("Proof generation failed: {0}")]
    ProofGenerationError(String),

    #[error("Key generation failed: {0}")]
    KeyGenerationError(String),

    #[error("Circuit compilation/processing failed: {0}")]
    CircuitProcessingError(String),

    #[error("Circuit registration failed: {0}")]
    CircuitRegistrationError(String),

    #[error("Proof verification failed: {0}")]
    VerificationError(String),

    #[error("Invalid job input: {0}")]
    InvalidInput(String),

    #[error("Keystore error: {0}")]
    KeystoreError(String),

    #[error("Missing required configuration: {0}")]
    MissingConfiguration(String),

    #[error("Incompatible coSNARK backend for circuit: {0}")]
    IncompatibleBackend(String),

    #[error("Invalid circuit definition: {0}")]
    InvalidCircuitDefinition(String),

    #[error("MPC Protocol error: {0}")]
    MpcProtocolError(String),

    #[error("Arithmetic error during computation: {0}")]
    ArithmeticError(String),

    #[error("Could not acquire lock: {0}")]
    LockError(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Invalid DNS name: {0}")]
    InvalidDnsName(String),
}

// Helper macro for lock errors (if using std::sync::Mutex/RwLock)
impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        Error::LockError(format!("Mutex/RwLock poisoned: {}", e))
    }
}

pub type Result<T> = std::result::Result<T, Error>;
