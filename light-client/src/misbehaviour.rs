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
            consensus_update_1: Some(
                convert_consensus_update_to_proto(data.consensus_update_1)
                    .expect("failed to convert consensus_update_1 to proto"),
            ),
            consensus_update_2: Some(
                convert_consensus_update_to_proto(data.consensus_update_2)
                    .expect("failed to convert consensus_update_2 to proto"),
            ),
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
            consensus_update_1: Some(
                convert_consensus_update_to_proto(data.consensus_update_1)
                    .expect("failed to convert consensus_update_1 to proto"),
            ),
            consensus_update_2: Some(
                convert_consensus_update_to_proto(data.consensus_update_2)
                    .expect("failed to convert consensus_update_2 to proto"),
            ),
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
                type_url: raw.type_url,
            }),
        }
    }
}

fn decode_finalized_header_misbehaviour<const SYNC_COMMITTEE_SIZE: usize, B: Buf>(
    buf: B,
) -> Result<Misbehaviour<SYNC_COMMITTEE_SIZE>, Error> {
    RawFinalizedHeaderMisbehaviour::decode(buf)
        .map_err(Error::ProtoDecode)?
        .try_into()
}

fn decode_next_sync_committee_misbehaviour<const SYNC_COMMITTEE_SIZE: usize, B: Buf>(
    buf: B,
) -> Result<Misbehaviour<SYNC_COMMITTEE_SIZE>, Error> {
    RawNextSyncCommitteeMisbehaviour::decode(buf)
        .map_err(Error::ProtoDecode)?
        .try_into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{to_consensus_update_info, TestFixture, SYNC_COMMITTEE_SIZE};
    use alloc::string::ToString;
    use light_client::types::Height;
    use prost::Message;

    #[test]
    fn test_misbehaviour_from_any_unknown_type() {
        let any = IBCAny {
            type_url: "/unknown.misbehaviour".to_string(),
            value: vec![],
        };
        let result = Misbehaviour::<SYNC_COMMITTEE_SIZE>::try_from(any);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::UnknownMisbehaviourType { type_url } if type_url == "/unknown.misbehaviour"
        ));
    }

    #[test]
    fn test_misbehaviour_from_any_invalid_finalized_header() {
        let any = IBCAny {
            type_url: ETHEREUM_FINALIZED_HEADER_MISBEHAVIOUR_TYPE_URL.to_string(),
            value: vec![0x00, 0x01, 0x02], // invalid protobuf data
        };
        let result = Misbehaviour::<SYNC_COMMITTEE_SIZE>::try_from(any);
        assert!(result.is_err());
    }

    #[test]
    fn test_misbehaviour_from_any_invalid_next_sync_committee() {
        let any = IBCAny {
            type_url: ETHEREUM_NEXT_SYNC_COMMITTEE_MISBEHAVIOUR_TYPE_URL.to_string(),
            value: vec![0x00, 0x01, 0x02], // invalid protobuf data
        };
        let result = Misbehaviour::<SYNC_COMMITTEE_SIZE>::try_from(any);
        assert!(result.is_err());
    }

    #[test]
    fn test_misbehaviour_validate_success() {
        let fixture = TestFixture::new();

        let (update_1, _) = fixture.gen_update([1u8; 32].into(), 100);
        let (update_2, _) = fixture.gen_update([2u8; 32].into(), 100);

        let update_info_1 = to_consensus_update_info(update_1);
        let update_info_2 = to_consensus_update_info(update_2);

        let misbehaviour = Misbehaviour::<SYNC_COMMITTEE_SIZE> {
            client_id: ClientId::from_str("ethereum-0").unwrap(),
            trusted_sync_committee: TrustedSyncCommittee {
                height: Height::new(0, 1),
                sync_committee: fixture.current_sync_committee().to_committee(),
                is_next: false,
            },
            data: MisbehaviourData::FinalizedHeader(FinalizedHeaderMisbehaviour {
                consensus_update_1: update_info_1,
                consensus_update_2: update_info_2,
            }),
        };

        let result = misbehaviour.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_next_sync_committee_misbehaviour_proto_roundtrip() {
        let fixture = TestFixture::new();

        let (update_1, _) = fixture.gen_update([1u8; 32].into(), 100);
        // Use different next sync committee for misbehaviour
        let (update_2, _) = TestFixture::new().gen_update([1u8; 32].into(), 100);

        let update_info_1 = to_consensus_update_info(update_1);
        let update_info_2 = to_consensus_update_info(update_2);

        let misbehaviour = Misbehaviour::<SYNC_COMMITTEE_SIZE> {
            client_id: ClientId::from_str("ethereum-0").unwrap(),
            trusted_sync_committee: TrustedSyncCommittee {
                height: Height::new(0, 1),
                sync_committee: fixture.current_sync_committee().to_committee(),
                is_next: false,
            },
            data: MisbehaviourData::NextSyncCommittee(NextSyncCommitteeMisbehaviour {
                consensus_update_1: update_info_1,
                consensus_update_2: update_info_2,
            }),
        };

        // Convert to raw proto
        let raw: RawNextSyncCommitteeMisbehaviour = misbehaviour.clone().into();

        // Encode to bytes
        let mut buf = Vec::new();
        raw.encode(&mut buf).unwrap();

        // Decode from bytes
        let decoded_raw = RawNextSyncCommitteeMisbehaviour::decode(buf.as_slice()).unwrap();

        // Convert back to Misbehaviour
        let decoded = Misbehaviour::<SYNC_COMMITTEE_SIZE>::try_from(decoded_raw).unwrap();

        assert_eq!(misbehaviour, decoded);
    }

    #[test]
    fn test_finalized_header_misbehaviour_proto_roundtrip() {
        let fixture = TestFixture::new();

        // Two updates with different execution state roots (same slot) = finalized header misbehaviour
        let (update_1, _) = fixture.gen_update([1u8; 32].into(), 100);
        let (update_2, _) = fixture.gen_update([2u8; 32].into(), 100);

        let update_info_1 = to_consensus_update_info(update_1);
        let update_info_2 = to_consensus_update_info(update_2);

        let misbehaviour = Misbehaviour::<SYNC_COMMITTEE_SIZE> {
            client_id: ClientId::from_str("ethereum-0").unwrap(),
            trusted_sync_committee: TrustedSyncCommittee {
                height: Height::new(0, 1),
                sync_committee: fixture.current_sync_committee().to_committee(),
                is_next: false,
            },
            data: MisbehaviourData::FinalizedHeader(FinalizedHeaderMisbehaviour {
                consensus_update_1: update_info_1,
                consensus_update_2: update_info_2,
            }),
        };

        // Convert to raw proto
        let raw: RawFinalizedHeaderMisbehaviour = misbehaviour.clone().into();

        // Encode to bytes
        let mut buf = Vec::new();
        raw.encode(&mut buf).unwrap();

        // Decode from bytes
        let decoded_raw = RawFinalizedHeaderMisbehaviour::decode(buf.as_slice()).unwrap();

        // Convert back to Misbehaviour
        let decoded = Misbehaviour::<SYNC_COMMITTEE_SIZE>::try_from(decoded_raw).unwrap();

        assert_eq!(misbehaviour, decoded);
    }

    #[test]
    fn test_misbehaviour_from_ibc_any_finalized_header() {
        let fixture = TestFixture::new();

        let (update_1, _) = fixture.gen_update([1u8; 32].into(), 100);
        let (update_2, _) = fixture.gen_update([2u8; 32].into(), 100);

        let update_info_1 = to_consensus_update_info(update_1);
        let update_info_2 = to_consensus_update_info(update_2);

        let misbehaviour = Misbehaviour::<SYNC_COMMITTEE_SIZE> {
            client_id: ClientId::from_str("ethereum-0").unwrap(),
            trusted_sync_committee: TrustedSyncCommittee {
                height: Height::new(0, 1),
                sync_committee: fixture.current_sync_committee().to_committee(),
                is_next: false,
            },
            data: MisbehaviourData::FinalizedHeader(FinalizedHeaderMisbehaviour {
                consensus_update_1: update_info_1,
                consensus_update_2: update_info_2,
            }),
        };

        // Convert to IBCAny
        let raw: RawFinalizedHeaderMisbehaviour = misbehaviour.clone().into();
        let mut buf = Vec::new();
        raw.encode(&mut buf).unwrap();

        let any = IBCAny {
            type_url: ETHEREUM_FINALIZED_HEADER_MISBEHAVIOUR_TYPE_URL.to_string(),
            value: buf,
        };

        // Decode from IBCAny
        let decoded = Misbehaviour::<SYNC_COMMITTEE_SIZE>::try_from(any).unwrap();
        assert_eq!(misbehaviour, decoded);
    }

    #[test]
    fn test_misbehaviour_from_ibc_any_next_sync_committee() {
        let fixture = TestFixture::new();

        let (update_1, _) = fixture.gen_update([1u8; 32].into(), 100);
        let (update_2, _) = TestFixture::new().gen_update([1u8; 32].into(), 100);

        let update_info_1 = to_consensus_update_info(update_1);
        let update_info_2 = to_consensus_update_info(update_2);

        let misbehaviour = Misbehaviour::<SYNC_COMMITTEE_SIZE> {
            client_id: ClientId::from_str("ethereum-0").unwrap(),
            trusted_sync_committee: TrustedSyncCommittee {
                height: Height::new(0, 1),
                sync_committee: fixture.current_sync_committee().to_committee(),
                is_next: false,
            },
            data: MisbehaviourData::NextSyncCommittee(NextSyncCommitteeMisbehaviour {
                consensus_update_1: update_info_1,
                consensus_update_2: update_info_2,
            }),
        };

        // Convert to IBCAny
        let raw: RawNextSyncCommitteeMisbehaviour = misbehaviour.clone().into();
        let mut buf = Vec::new();
        raw.encode(&mut buf).unwrap();

        let any = IBCAny {
            type_url: ETHEREUM_NEXT_SYNC_COMMITTEE_MISBEHAVIOUR_TYPE_URL.to_string(),
            value: buf,
        };

        // Decode from IBCAny
        let decoded = Misbehaviour::<SYNC_COMMITTEE_SIZE>::try_from(any).unwrap();
        assert_eq!(misbehaviour, decoded);
    }
}
