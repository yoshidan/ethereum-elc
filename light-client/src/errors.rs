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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_proto_missing() {
        let err = Error::proto_missing("test_field");
        assert!(matches!(err, Error::ProtoMissingField(ref s) if s == "test_field"));
        assert!(err.to_string().contains("test_field"));
    }

    #[test]
    fn test_error_display_client_state_errors() {
        let err = Error::CannotInitializeFrozenClient;
        assert_eq!(err.to_string(), "cannot initialize frozen client");

        let err = Error::UninitializedClientStateField("latest_slot");
        assert!(err.to_string().contains("latest_slot"));

        let err = Error::MissingBellatrixFork;
        assert_eq!(err.to_string(), "missing bellatrix fork");

        let err = Error::MissingTrustingPeriod;
        assert_eq!(err.to_string(), "missing trusting period");

        let err = Error::NegativeMaxClockDrift;
        assert_eq!(err.to_string(), "negative max clock drift");

        let err = Error::UnknownClientStateType("unknown_type".to_string());
        assert!(err.to_string().contains("unknown_type"));

        let err = Error::UnexpectedClientType("wrong_type".to_string());
        assert!(err.to_string().contains("wrong_type"));

        let err = Error::UnexpectedStoreAddress("0x1234".to_string());
        assert!(err.to_string().contains("0x1234"));
    }

    #[test]
    fn test_error_display_consensus_state_errors() {
        let err = Error::UninitializedConsensusStateField("slot");
        assert!(err.to_string().contains("slot"));

        let err = Error::InvalidRawConsensusState {
            reason: "invalid format".to_string(),
        };
        assert!(err.to_string().contains("invalid format"));

        let err = Error::UnknownConsensusStateType {
            type_url: "/unknown.type".to_string(),
        };
        assert!(err.to_string().contains("/unknown.type"));

        let err = Error::TimestampOverflow;
        assert_eq!(err.to_string(), "timestamp overflow");
    }

    #[test]
    fn test_error_display_header_errors() {
        let err = Error::UnknownMessageType("unknown_msg".to_string());
        assert!(err.to_string().contains("unknown_msg"));

        let err = Error::UnknownHeaderType {
            type_url: "/unknown.header".to_string(),
        };
        assert!(err.to_string().contains("/unknown.header"));

        let err = Error::ZeroTimestamp;
        assert_eq!(err.to_string(), "zero timestamp");

        let err = Error::ZeroBlockNumber;
        assert_eq!(err.to_string(), "zero block number");

        let err = Error::UnexpectedTimestamp {
            expected: 1000,
            actual: 2000,
        };
        let msg = err.to_string();
        assert!(msg.contains("1000"));
        assert!(msg.contains("2000"));
    }

    #[test]
    fn test_error_display_misbehaviour_errors() {
        let err = Error::UnknownMisbehaviourType {
            type_url: "/unknown.misbehaviour".to_string(),
        };
        assert!(err.to_string().contains("/unknown.misbehaviour"));
    }

    #[test]
    fn test_error_display_proto_errors() {
        let err = Error::ProtoMissingField("required_field".to_string());
        assert!(err.to_string().contains("required_field"));
    }
}
