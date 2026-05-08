use crate::errors::Error;
use crate::internal_prelude::*;
use ethereum_light_client_proto::google::protobuf::Any as IBCAny;
use ethereum_consensus::{
    beacon::Slot,
    bls::PublicKey,
    compute::compute_sync_committee_period_at_slot,
    context::ChainContext,
    sync_protocol::{SyncCommittee, SyncCommitteePeriod},
};
use ethereum_consensus::types::H256;
use ethereum_light_client_types::update::TrustedSyncCommitteeInfo;
use ethereum_elc_proto::{
    google::protobuf::Timestamp as ProtoTimestamp,
    ibc::lightclients::ethereum::v1::ConsensusState as RawConsensusState,
};
use ethereum_light_client_verifier::{state::LightClientStoreReader, updates::ConsensusUpdate};
use light_client::types::{Any, Time};
use prost::Message;
use prost_types::Timestamp;

pub const ETHEREUM_CONSENSUS_STATE_TYPE_URL: &str = "/ibc.lightclients.ethereum.v1.ConsensusState";

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConsensusState {
    /// finalized header's slot
    pub slot: Slot,
    /// the storage root of the IBC contract
    pub storage_root: H256,
    /// timestamp from execution payload
    pub timestamp: Time,
    /// aggregate public key of current sync committee
    /// "current" indicates a period corresponding to the `slot`
    pub current_sync_committee: PublicKey,
    /// aggregate public key of next sync committee
    /// "next" indicates `current + 1` period
    pub next_sync_committee: PublicKey,
}

impl <CC:ChainContext> TrustedSyncCommitteeInfo<CC> for ConsensusState {
    fn current_period(&self, ctx: &CC) -> SyncCommitteePeriod {
        compute_sync_committee_period_at_slot(ctx, self.slot)
    }

    fn current_sync_committee(&self) -> PublicKey {
        self.current_sync_committee.clone()
    }

    fn next_sync_committee(&self) -> PublicKey {
        self.next_sync_committee.clone()
    }
}

impl ConsensusState {
    pub fn validate(&self) -> Result<(), Error> {
        if self.slot == Default::default() {
            Err(Error::UninitializedConsensusStateField("slot"))
        } else if self.storage_root.as_bytes().is_empty() {
            Err(Error::UninitializedConsensusStateField("storage_root"))
        } else if self.timestamp.as_unix_timestamp_nanos() == 0  {
            Err(Error::UninitializedConsensusStateField("timestamp"))
        } else if self.current_sync_committee == PublicKey::default() {
            Err(Error::UninitializedConsensusStateField(
                "current_sync_committee",
            ))
        } else if self.next_sync_committee == PublicKey::default() {
            Err(Error::UninitializedConsensusStateField(
                "next_sync_committee",
            ))
        } else {
            Ok(())
        }
    }

    pub fn current_period<C: ChainContext>(&self, ctx: &C) -> SyncCommitteePeriod {
        compute_sync_committee_period_at_slot(ctx, self.slot)
    }
}

fn timestamp_to_proto_timestamp(timestamp: Time) -> Timestamp {
    let nanos = timestamp.as_unix_timestamp_nanos();
    Timestamp {
        seconds: (nanos / 1_000_000_000) as i64,
        nanos: (nanos % 1_000_000_000) as i32,
    }
}

impl TryFrom<RawConsensusState> for ConsensusState {
    type Error = Error;

    fn try_from(value: RawConsensusState) -> Result<Self, Self::Error> {
        let next_sync_committee = if value.next_sync_committee.is_empty() {
            return Err(Self::Error::InvalidRawConsensusState {
                reason: "next_sync_committee is empty".to_string(),
            });
        } else {
            PublicKey::try_from(value.next_sync_committee)?
        };
        let timestamp = value.timestamp.ok_or_else(|| {
            Error::InvalidRawConsensusState {
                reason: "timestamp is none".to_string(),
            }
        })?;
        Ok(Self {
            slot: value.slot.into(),
            storage_root: H256::from_slice(value.storage_root.as_slice()),
            timestamp: Time::from_unix_timestamp(timestamp.seconds, timestamp.nanos as u32)?,
            current_sync_committee: PublicKey::try_from(value.current_sync_committee)?,
            next_sync_committee,
        })
    }
}

impl From<ConsensusState> for RawConsensusState {
    fn from(value: ConsensusState) -> Self {
        Self {
            slot: value.slot.into(),
            storage_root: value.storage_root.0.to_vec(),
            timestamp: Some(timestamp_to_proto_timestamp(value.timestamp)),
            current_sync_committee: value.current_sync_committee.to_vec(),
            next_sync_committee: value.next_sync_committee.to_vec(),
        }
    }
}

impl TryFrom<IBCAny> for ConsensusState {
    type Error = Error;

    fn try_from(raw: IBCAny) -> Result<Self, Self::Error> {
        use bytes::Buf;
        use core::ops::Deref;
        use prost::Message;

        fn decode_consensus_state<B: Buf>(buf: B) -> Result<ConsensusState, Error> {
            RawConsensusState::decode(buf)
                .map_err(Error::Decode)?
                .try_into()
        }

        match raw.type_url.as_str() {
            ETHEREUM_CONSENSUS_STATE_TYPE_URL => {
                decode_consensus_state(raw.value.deref()).map_err(Into::into)
            }
            _ => Err(Error::UnknownConsensusStateType {
                consensus_state_type: raw.type_url,
            }),
        }
    }
}

impl TryFrom<ConsensusState> for IBCAny {
    type Error = Error;

    fn try_from(value: ConsensusState) -> Result<Self, Self::Error> {
        let value: RawConsensusState = value.into();
        let mut v = Vec::new();
        value.encode(&mut v).map_err(Error::ProtoEncodeError)?;
        Ok(Self {
            type_url: ETHEREUM_CONSENSUS_STATE_TYPE_URL.to_string(),
            value: v,
        })
    }
}

impl TryFrom<ConsensusState> for Any {
    type Error = Error;
    fn try_from(value: ConsensusState) -> Result<Self, Error> {
        Ok(IBCAny::try_from(value)?.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedConsensusState<const SYNC_COMMITTEE_SIZE: usize> {
    state: ConsensusState,
    current_sync_committee: Option<SyncCommittee<SYNC_COMMITTEE_SIZE>>,
    next_sync_committee: Option<SyncCommittee<SYNC_COMMITTEE_SIZE>>,
}

impl<const SYNC_COMMITTEE_SIZE: usize> TrustedConsensusState<SYNC_COMMITTEE_SIZE> {
    pub fn new(
        consensus_state: ConsensusState,
        sync_committee: SyncCommittee<SYNC_COMMITTEE_SIZE>,
        is_next: bool,
    ) -> Result<Self, Error> {
        sync_committee.validate()?;
        if !is_next {
            return if sync_committee.aggregate_pubkey == consensus_state.current_sync_committee {
                Ok(Self {
                    state: consensus_state,
                    current_sync_committee: Some(sync_committee),
                    next_sync_committee: None,
                })
            } else {
                Err(Error::InvalidCurrentSyncCommitteeKeys(
                    sync_committee.aggregate_pubkey,
                    consensus_state.current_sync_committee,
                ))
            };
        }

        if sync_committee.aggregate_pubkey == consensus_state.next_sync_committee {
            Ok(Self {
                state: consensus_state,
                current_sync_committee: None,
                next_sync_committee: Some(sync_committee),
            })
        } else {
            Err(Error::InvalidNextSyncCommitteeKeys(
                sync_committee.aggregate_pubkey,
                consensus_state.next_sync_committee,
            ))
        }
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> LightClientStoreReader<SYNC_COMMITTEE_SIZE>
    for TrustedConsensusState<SYNC_COMMITTEE_SIZE>
{
    fn current_period<C: ChainContext>(&self, ctx: &C) -> SyncCommitteePeriod {
        self.state.current_period(ctx)
    }

    fn current_sync_committee(&self) -> Option<SyncCommittee<SYNC_COMMITTEE_SIZE>> {
        self.current_sync_committee.clone()
    }

    fn next_sync_committee(&self) -> Option<SyncCommittee<SYNC_COMMITTEE_SIZE>> {
        self.next_sync_committee.clone()
    }

    fn ensure_relevant_update<CC: ChainContext, C: ConsensusUpdate<SYNC_COMMITTEE_SIZE>>(
        &self,
        _ctx: &CC,
        update: &C,
    ) -> Result<(), ethereum_light_client_verifier::errors::Error> {
        if self.state.slot >= update.finalized_beacon_header().slot {
            Err(
                ethereum_light_client_verifier::errors::Error::IrrelevantConsensusUpdates(
                    "finalized header slot is not greater than current slot".to_string(),
                ),
            )
        } else {
            Ok(())
        }
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> From<TrustedConsensusState<SYNC_COMMITTEE_SIZE>>
    for ConsensusState
{
    fn from(value: TrustedConsensusState<SYNC_COMMITTEE_SIZE>) -> Self {
        value.state
    }
}
