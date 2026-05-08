use crate::errors::Error;
use crate::misbehaviour::{
    Misbehaviour, ETHEREUM_FINALIZED_HEADER_MISBEHAVIOUR_TYPE_URL,
    ETHEREUM_NEXT_SYNC_COMMITTEE_MISBEHAVIOUR_TYPE_URL,
};
use bytes::Buf;
use ethereum_consensus::compute::compute_timestamp_at_slot;
use ethereum_consensus::context::ChainContext;
use ethereum_consensus::types::U64;
use ethereum_elc_proto::ibc::lightclients::ethereum::v1::Header as RawHeader;
use ethereum_light_client_proto::google::protobuf::Any as IBCAny;
use ethereum_light_client_types::consensus::{
    convert_proto_to_consensus_update, convert_proto_to_execution_update, AccountUpdateInfo,
    ConsensusUpdateInfo, ExecutionUpdateInfo, TrustedSyncCommittee,
};
use ethereum_light_client_types::time::new_timestamp;
use ethereum_light_client_verifier::updates::ConsensusUpdate;
use light_client::types::Time;
use prost::Message;

pub const ETHEREUM_HEADER_TYPE_URL: &str = "/ibc.lightclients.ethereum.v1.Header";

#[allow(clippy::large_enum_variant)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum ClientMessage<const SYNC_COMMITTEE_SIZE: usize> {
    Header(Header<SYNC_COMMITTEE_SIZE>),
    Misbehaviour(Misbehaviour<SYNC_COMMITTEE_SIZE>),
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<IBCAny> for ClientMessage<SYNC_COMMITTEE_SIZE> {
    type Error = Error;

    fn try_from(raw: IBCAny) -> Result<Self, Self::Error> {
        match raw.type_url.as_str() {
            ETHEREUM_HEADER_TYPE_URL => {
                let header = Header::<SYNC_COMMITTEE_SIZE>::try_from(raw)?;
                Ok(Self::Header(header))
            }
            ETHEREUM_FINALIZED_HEADER_MISBEHAVIOUR_TYPE_URL
            | ETHEREUM_NEXT_SYNC_COMMITTEE_MISBEHAVIOUR_TYPE_URL => {
                let misbehaviour = Misbehaviour::<SYNC_COMMITTEE_SIZE>::try_from(raw)?;
                Ok(Self::Misbehaviour(misbehaviour))
            }
            _ => Err(Error::UnknownMessageType(raw.type_url)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Header<const SYNC_COMMITTEE_SIZE: usize> {
    /// trusted sync committee corresponding to the period of the signature slot of the `consensus_update`
    pub trusted_sync_committee: TrustedSyncCommittee<SYNC_COMMITTEE_SIZE>,
    /// consensus update attested by the `trusted_sync_committee`
    pub consensus_update: ConsensusUpdateInfo<SYNC_COMMITTEE_SIZE>,
    /// execution update based on the `consensus_update.finalized_header`
    pub execution_update: ExecutionUpdateInfo,
    /// account update based on the `execution_update.state_root`
    pub account_update: AccountUpdateInfo,
    /// timestamp of the `consensus_update.finalized_header`
    pub timestamp: Time,
}

pub fn decode_header<const SYNC_COMMITTEE_SIZE: usize, B: Buf>(
    buf: B,
) -> Result<Header<SYNC_COMMITTEE_SIZE>, Error> {
    RawHeader::decode(buf).map_err(Error::Decode)?.try_into()
}

impl<const SYNC_COMMITTEE_SIZE: usize> Header<SYNC_COMMITTEE_SIZE> {
    pub fn validate<C: ChainContext>(&self, ctx: &C) -> Result<(), Error> {
        self.trusted_sync_committee.validate()?;
        if self.timestamp.as_unix_timestamp_nanos() == 0 {
            return Err(Error::ZeroTimestampError);
        }
        if self.execution_update.block_number == U64(0) {
            return Err(Error::ZeroBlockNumberError);
        }
        let header_timestamp_nanos = self.timestamp.as_unix_timestamp_nanos();
        let spec_timestamp_nanos = new_timestamp(
            compute_timestamp_at_slot(ctx, self.consensus_update.finalized_beacon_header().slot).0,
        )?
        .as_unix_timestamp_nanos();
        if header_timestamp_nanos != spec_timestamp_nanos {
            return Err(Error::UnexpectedTimestamp(
                spec_timestamp_nanos,
                header_timestamp_nanos,
            ));
        }
        Ok(())
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<RawHeader> for Header<SYNC_COMMITTEE_SIZE> {
    type Error = Error;
    fn try_from(value: RawHeader) -> Result<Self, Self::Error> {
        let trusted_sync_committee = value
            .trusted_sync_committee
            .ok_or(Error::proto_missing("trusted_sync_committee"))?;
        let consensus_update = value
            .consensus_update
            .ok_or(Error::proto_missing("consensus_update"))?;
        let execution_update = value
            .execution_update
            .ok_or(Error::proto_missing("execution_update"))?;
        let account_update = value
            .account_update
            .ok_or(Error::proto_missing("account_update"))?;
        let timestamp = new_timestamp(value.timestamp)?;
        Ok(Self {
            trusted_sync_committee: trusted_sync_committee.try_into()?,
            consensus_update: convert_proto_to_consensus_update(consensus_update)?,
            execution_update: convert_proto_to_execution_update(execution_update),
            account_update: account_update.try_into()?,
            timestamp,
        })
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<IBCAny> for Header<SYNC_COMMITTEE_SIZE> {
    type Error = Error;

    fn try_from(raw: IBCAny) -> Result<Self, Self::Error> {
        use core::ops::Deref;

        match raw.type_url.as_str() {
            ETHEREUM_HEADER_TYPE_URL => decode_header(raw.value.deref()),
            _ => Err(Error::UnknownHeaderType {
                header_type: raw.type_url,
            }),
        }
    }
}
