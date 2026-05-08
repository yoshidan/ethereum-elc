use crate::internal_prelude::*;
use light_client::LightClientSpecificError;
use light_client::types::ClientId;
use ethereum_consensus::bls::PublicKey;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// unexpected client type: `{0}`
    #[error("UnexpectedClientType(client_type={0})")]
    UnexpectedClientType(String),
    /// time conversion error: `{0}`
    #[error("TimeConversionError(0={0})")]
    Time(light_client::types::TimeError),
    #[error("CannotInitializeFrozenClient")]
    CannotInitializeFrozenClient,
    #[error("EthType error: {0:?}")]
    TypeError(#[from] ethereum_light_client_types::errors::Error),
    #[error("UninitializedClientStateField({0})")]
    UninitializedClientStateField(&'static str),
    #[error("MissingBellatrixFork")]
    MissingBellatrixFork,
    #[error("VerificationError({0:?})")]
    VerificationError(ethereum_light_client_verifier::errors::Error),
    #[error("EthereumConsensusError({0:?})")]
    EthereumConsensusError(ethereum_consensus::errors::Error),
    #[error("MissingTrustingPeriod")]
    MissingTrustingPeriod,
    #[error("NegativeMaxClockDrift")]
    NegativeMaxClockDrift,
    #[error("UnknownClientStateType({0})")]
    UnknownClientStateType(String),
    #[error("ProtoDecodeError({0:?})")]
    ProtoDecodeError(prost::DecodeError),
    #[error("ProtoEncodeError({0:?})")]
    ProtoEncodeError(prost::EncodeError),
    #[error("UnexpectedClientIdInMisbehaviour(expected={0}, actual={1})")]
    UnexpectedClientIdInMisbehaviour(ClientId, ClientId),
    #[error("MissingProtoField({0})")]
    MissingProtoField(String),
    #[error("UnexpectedStoreAddress({0})")]
    UnexpectedStoreAddress(String),
    // ConsensusState errors
    #[error("UninitializedConsensusStateField({0})")]
    UninitializedConsensusStateField(&'static str),
    #[error("InvalidRawConsensusState(reason={reason})")]
    InvalidRawConsensusState { reason: String },
    #[error("TimestampOverflowError")]
    TimestampOverflowError,
    #[error("Decode({0:?})")]
    Decode(prost::DecodeError),
    #[error("UnknownConsensusStateType(consensus_state_type={consensus_state_type})")]
    UnknownConsensusStateType { consensus_state_type: String },
    #[error("InvalidCurrentSyncCommitteeKeys(expected={0:?}, actual={1:?})")]
    InvalidCurrentSyncCommitteeKeys(PublicKey, PublicKey),
    #[error("InvalidNextSyncCommitteeKeys(expected={0:?}, actual={1:?})")]
    InvalidNextSyncCommitteeKeys(PublicKey, PublicKey),
    // Header errors
    #[error("UnknownMessageType({0})")]
    UnknownMessageType(String),
    #[error("ZeroTimestampError")]
    ZeroTimestampError,
    #[error("ZeroBlockNumberError")]
    ZeroBlockNumberError,
    #[error("UnexpectedTimestamp(expected={0}, actual={1})")]
    UnexpectedTimestamp(u128, u128),
    #[error("UnknownHeaderType(header_type={header_type})")]
    UnknownHeaderType { header_type: String },
    // State errors
    #[error("CommitmentError({0})")]
    CommitmentError(light_client::commitments::Error),
    // Misbehaviour errors
    #[error("UnknownMisbehaviourType(misbehaviour_type={misbehaviour_type})")]
    UnknownMisbehaviourType { misbehaviour_type: String },
    #[error("ClientIdParseError({0})")]
    ClientIdParseError(light_client::types::TypeError),
}

impl Error {
    pub fn proto_missing(field: &str) -> Self {
        Error::MissingProtoField(field.to_string())
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
        Error::EthereumConsensusError(e)
    }
}

impl From<light_client::types::TypeError> for Error {
    fn from(e: light_client::types::TypeError) -> Self {
        Error::ClientIdParseError(e)
    }
}

