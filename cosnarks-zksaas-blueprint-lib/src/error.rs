use crate::p2p::Blame;
use thiserror::Error;

// Alias for Result used throughout the library
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Main error type for the CoSNARKs zkSaaS Blueprint.
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Blueprint Error: {0}")]
    BlueprintError(#[from] blueprint_sdk::Error),

    #[error("Configuration Error: {0}")]
    ConfigError(String),

    #[error("Missing Configuration: {0}")]
    MissingConfiguration(String),

    #[error("State Error (Database/Storage): {0}")]
    StateError(String),

    #[error("Serialization/Deserialization Error (bincode): {0}")]
    BincodeError(#[from] bincode::Error),

    #[error("Serialization/Deserialization Error (serde_json): {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Networking Error: {0}")]
    NetworkError(String),

    #[error("Invalid Input: {0}")]
    InvalidInput(String),

    #[error("HTTP Request Error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Invalid URL: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Incompatible Circuit Type/Proving Backend: {0}")]
    IncompatibleBackend(String),

    #[error("MPC Protocol Error: {0}")]
    MpcProtocolError(String),

    #[error("Commitment Mismatch - Cheating Detected: {guilty_parties:?}")]
    CommitmentMismatch { guilty_parties: Vec<Blame> },

    #[error("Invalid DNS Name Format: {0}")]
    InvalidDnsName(String),

    #[error("Round-based Protocol Error: {0}")]
    ExchangeRoundBasedError(String),

    #[error("Internal Error: {0}")]
    Internal(String),
}
