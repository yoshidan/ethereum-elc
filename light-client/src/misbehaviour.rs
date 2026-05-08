use crate::errors::Error;
use alloc::string::ToString;
use bytes::Buf;
use core::str::FromStr;
use ethereum_elc_proto::ibc::lightclients::ethereum::v1::{
    FinalizedHeaderMisbehaviour as RawFinalizedHeaderMisbehaviour,
    NextSyncCommitteeMisbehaviour as RawNextSyncCommitteeMisbehaviour,
};
use ethereum_light_client_proto::google::protobuf::Any as IBCAny;
use ethereum_light_client_types::consensus::{
    convert_consensus_update_to_proto, convert_proto_to_consensus_update, ConsensusUpdateInfo,
    TrustedSyncCommittee,
};
use ethereum_light_client_verifier::misbehaviour::{
    FinalizedHeaderMisbehaviour, Misbehaviour as MisbehaviourData, NextSyncCommitteeMisbehaviour,
};
use light_client::types::ClientId;
use prost::Message;
use serde::{Deserialize, Serialize};

pub const ETHEREUM_FINALIZED_HEADER_MISBEHAVIOUR_TYPE_URL: &str =
    "/ibc.lightclients.ethereum.v1.FinalizedHeaderMisbehaviour";
pub const ETHEREUM_NEXT_SYNC_COMMITTEE_MISBEHAVIOUR_TYPE_URL: &str =
    "/ibc.lightclients.ethereum.v1.NextSyncCommitteeMisbehaviour";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Misbehaviour<const SYNC_COMMITTEE_SIZE: usize> {
    /// The client identifier
    pub client_id: ClientId,
    /// The sync committee related to the misbehaviour
    pub trusted_sync_committee: TrustedSyncCommittee<SYNC_COMMITTEE_SIZE>,
    /// The misbehaviour data
    pub data: MisbehaviourData<SYNC_COMMITTEE_SIZE, ConsensusUpdateInfo<SYNC_COMMITTEE_SIZE>>,
}

impl<const SYNC_COMMITTEE_SIZE: usize> Misbehaviour<SYNC_COMMITTEE_SIZE> {
    pub fn validate(&self) -> Result<(), Error> {
        self.trusted_sync_committee.validate()?;
        Ok(())
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<RawFinalizedHeaderMisbehaviour>
    for Misbehaviour<SYNC_COMMITTEE_SIZE>
{
    type Error = Error;
    fn try_from(value: RawFinalizedHeaderMisbehaviour) -> Result<Self, Self::Error> {
        Ok(Self {
            client_id: ClientId::from_str(&value.client_id)?,
            trusted_sync_committee: value
                .trusted_sync_committee
                .ok_or(Error::proto_missing("trusted_sync_committee"))?
                .try_into()?,
            data: MisbehaviourData::FinalizedHeader(FinalizedHeaderMisbehaviour {
                consensus_update_1: convert_proto_to_consensus_update(
                    value
                        .consensus_update_1
                        .ok_or(Error::proto_missing("consensus_update_1"))?,
                )?,
                consensus_update_2: convert_proto_to_consensus_update(
                    value
                        .consensus_update_2
                        .ok_or(Error::proto_missing("consensus_update_2"))?,
                )?,
            }),
        })
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<RawNextSyncCommitteeMisbehaviour>
    for Misbehaviour<SYNC_COMMITTEE_SIZE>
{
    type Error = Error;
    fn try_from(value: RawNextSyncCommitteeMisbehaviour) -> Result<Self, Self::Error> {
        Ok(Self {
            client_id: ClientId::from_str(&value.client_id)?,
            trusted_sync_committee: value
                .trusted_sync_committee
                .ok_or(Error::proto_missing("trusted_sync_committee"))?
                .try_into()?,
            data: MisbehaviourData::NextSyncCommittee(NextSyncCommitteeMisbehaviour {
                consensus_update_1: convert_proto_to_consensus_update(
                    value
                        .consensus_update_1
                        .ok_or(Error::proto_missing("consensus_update_1"))?,
                )?,
                consensus_update_2: convert_proto_to_consensus_update(
                    value
                        .consensus_update_2
                        .ok_or(Error::proto_missing("consensus_update_2"))?,
                )?,
            }),
        })
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> From<Misbehaviour<SYNC_COMMITTEE_SIZE>>
    for RawFinalizedHeaderMisbehaviour
{
    fn from(value: Misbehaviour<SYNC_COMMITTEE_SIZE>) -> Self {
        let data = match value.data {
            MisbehaviourData::FinalizedHeader(data) => data,
            _ => panic!("unexpected misbehaviour type"),
        };
        Self {
            client_id: value.client_id.as_str().to_string(),
            trusted_sync_committee: Some(value.trusted_sync_committee.into()),
            consensus_update_1: Some(convert_consensus_update_to_proto(data.consensus_update_1)),
            consensus_update_2: Some(convert_consensus_update_to_proto(data.consensus_update_2)),
        }
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> From<Misbehaviour<SYNC_COMMITTEE_SIZE>>
    for RawNextSyncCommitteeMisbehaviour
{
    fn from(value: Misbehaviour<SYNC_COMMITTEE_SIZE>) -> Self {
        let data = match value.data {
            MisbehaviourData::NextSyncCommittee(data) => data,
            _ => panic!("unexpected misbehaviour type"),
        };
        Self {
            client_id: value.client_id.as_str().to_string(),
            trusted_sync_committee: Some(value.trusted_sync_committee.into()),
            consensus_update_1: Some(convert_consensus_update_to_proto(data.consensus_update_1)),
            consensus_update_2: Some(convert_consensus_update_to_proto(data.consensus_update_2)),
        }
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<IBCAny> for Misbehaviour<SYNC_COMMITTEE_SIZE> {
    type Error = Error;

    fn try_from(raw: IBCAny) -> Result<Self, Self::Error> {
        use core::ops::Deref;

        match raw.type_url.as_str() {
            ETHEREUM_FINALIZED_HEADER_MISBEHAVIOUR_TYPE_URL => {
                decode_finalized_header_misbehaviour(raw.value.deref())
            }
            ETHEREUM_NEXT_SYNC_COMMITTEE_MISBEHAVIOUR_TYPE_URL => {
                decode_next_sync_committee_misbehaviour(raw.value.deref())
            }
            _ => Err(Error::UnknownMisbehaviourType {
                misbehaviour_type: raw.type_url,
            }),
        }
    }
}

fn decode_finalized_header_misbehaviour<const SYNC_COMMITTEE_SIZE: usize, B: Buf>(
    buf: B,
) -> Result<Misbehaviour<SYNC_COMMITTEE_SIZE>, Error> {
    RawFinalizedHeaderMisbehaviour::decode(buf)
        .map_err(Error::Decode)?
        .try_into()
}

fn decode_next_sync_committee_misbehaviour<const SYNC_COMMITTEE_SIZE: usize, B: Buf>(
    buf: B,
) -> Result<Misbehaviour<SYNC_COMMITTEE_SIZE>, Error> {
    RawNextSyncCommitteeMisbehaviour::decode(buf)
        .map_err(Error::Decode)?
        .try_into()
}
