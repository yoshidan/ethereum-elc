use core::fmt::{Display, Formatter};
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
    Time(#[from] light_client::types::TimeError),
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
    EthereumConsensusError(#[from] ethereum_consensus::errors::Error),
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
    #[error("UnexpectedStoreAddress({0:?})")]
    UnexpectedStoreAddress(ethereum_consensus::types::AddressError),
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
    #[error("BlsError({0:?})")]
    BlsError(#[from] ethereum_consensus::bls::Error),
}

impl Error {
    pub fn proto_missing(field: &str) -> Self {
        Error::MissingProtoField(field.to_string())
    }
}

impl LightClientSpecificError for Error {}

