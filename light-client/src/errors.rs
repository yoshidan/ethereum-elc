//! Error types for Ethereum light client operations.

use crate::internal_prelude::*;
use ethereum_consensus::bls::PublicKey;
use light_client::types::ClientId;
use light_client::LightClientSpecificError;

/// Error type for Ethereum light client operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // ========================================================================
    // Client state errors
    // ========================================================================
    #[error("client is frozen: client_id={0}")]
    ClientFrozen(ClientId),
    #[error("cannot initialize frozen client")]
    CannotInitializeFrozenClient,
    #[error("uninitialized client state field: {0}")]
    UninitializedClientStateField(&'static str),
    #[error("missing bellatrix fork")]
    MissingBellatrixFork,
    #[error("missing trusting period")]
    MissingTrustingPeriod,
    #[error("negative max clock drift")]
    NegativeMaxClockDrift,
    #[error("unknown client state type: {0}")]
    UnknownClientStateType(String),
    #[error("unexpected client type: {0}")]
    UnexpectedClientType(String),
    #[error("unexpected store address: {0}")]
    UnexpectedStoreAddress(String),

    // ========================================================================
    // Consensus state errors
    // ========================================================================
    #[error("uninitialized consensus state field: {0}")]
    UninitializedConsensusStateField(&'static str),
    #[error("invalid raw consensus state: reason={reason}")]
    InvalidRawConsensusState { reason: String },
    #[error("unknown consensus state type: type_url={type_url}")]
    UnknownConsensusStateType { type_url: String },
    #[error("invalid current sync committee keys: expected={expected:?} actual={actual:?}")]
    InvalidCurrentSyncCommitteeKeys {
        expected: PublicKey,
        actual: PublicKey,
    },
    #[error("invalid next sync committee keys: expected={expected:?} actual={actual:?}")]
    InvalidNextSyncCommitteeKeys {
        expected: PublicKey,
        actual: PublicKey,
    },
    #[error("timestamp overflow")]
    TimestampOverflow,

    // ========================================================================
    // Header/Message errors
    // ========================================================================
    #[error("unknown message type: {0}")]
    UnknownMessageType(String),
    #[error("unknown header type: type_url={type_url}")]
    UnknownHeaderType { type_url: String },
    #[error("zero timestamp")]
    ZeroTimestamp,
    #[error("zero block number")]
    ZeroBlockNumber,
    #[error("unexpected timestamp: expected={expected} actual={actual}")]
    UnexpectedTimestamp { expected: u128, actual: u128 },

    // ========================================================================
    // Misbehaviour errors
    // ========================================================================
    #[error("unknown misbehaviour type: type_url={type_url}")]
    UnknownMisbehaviourType { type_url: String },
    #[error("unexpected client id in misbehaviour: expected={expected} actual={actual}")]
    UnexpectedClientIdInMisbehaviour {
        expected: ClientId,
        actual: ClientId,
    },

    // ========================================================================
    // Proto/Serialization errors
    // ========================================================================
    #[error("proto missing field: {0}")]
    ProtoMissingField(String),
    #[error("proto decode error: {0:?}")]
    ProtoDecode(prost::DecodeError),
    #[error("proto encode error: {0:?}")]
    ProtoEncode(prost::EncodeError),

    // ========================================================================
    // External library errors (with impl From)
    // ========================================================================
    #[error("ethereum light client types error: {0:?}")]
    EthereumLightClientTypes(#[from] ethereum_light_client_types::errors::Error),
    #[error("ethereum consensus error: {0:?}")]
    EthereumConsensus(ethereum_consensus::errors::Error),
    #[error("verification error: {0:?}")]
    Verification(ethereum_light_client_verifier::errors::Error),
    #[error("commitment error: {0:?}")]
    Commitment(light_client::commitments::Error),
    #[error("time error: {0:?}")]
    Time(light_client::types::TimeError),
    #[error("type error: {0:?}")]
    Type(light_client::types::TypeError),
    #[error("light client error: {0:?}")]
    LightClient(light_client::Error),
}

impl Error {
    pub fn proto_missing(field: &str) -> Self {
        Error::ProtoMissingField(field.to_string())
    }
}

impl LightClientSpecificError for Error {}

impl From<light_client::types::TimeError> for Error {
    fn from(e: light_client::types::TimeError) -> Self {
        Error::Time(e)
    }
}

impl From<ethereum_consensus::errors::Error> for Error {
    fn from(e: ethereum_consensus::errors::Error) -> Self {
        Error::EthereumConsensus(e)
    }
}

impl From<light_client::types::TypeError> for Error {
    fn from(e: light_client::types::TypeError) -> Self {
        Error::Type(e)
    }
}

impl From<light_client::Error> for Error {
    fn from(e: light_client::Error) -> Self {
        Error::LightClient(e)
    }
}

