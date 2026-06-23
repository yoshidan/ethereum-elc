use crate::errors::Error;
use crate::internal_prelude::*;
use ethereum_consensus::types::{H256, U64};
use ethereum_consensus::{
    beacon::Slot, bls::PublicKey, compute::compute_sync_committee_period_at_slot,
    context::ChainContext, sync_protocol::SyncCommitteePeriod,
};
use ethereum_elc_proto::{
    google::protobuf::Timestamp as ProtoTimestamp,
    ibc::lightclients::ethereum::v1::ConsensusState as RawConsensusState,
};
use ethereum_light_client_proto::google::protobuf::Any as IBCAny;
use ethereum_light_client_types::consensus_state::ConsensusState as EthConsensusState;
use ethereum_light_client_types::update::TrustedSyncCommitteeInfo;
use light_client::types::{Any, Time};
use prost::Message;

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

impl Default for ConsensusState {
    fn default() -> Self {
        Self {
            slot: Default::default(),
            storage_root: Default::default(),
            timestamp: Time::from_unix_timestamp_nanos(0).unwrap(),
            current_sync_committee: Default::default(),
            next_sync_committee: Default::default(),
        }
    }
}

impl TrustedSyncCommitteeInfo for ConsensusState {
    fn current_period<C: ChainContext>(&self, ctx: &C) -> SyncCommitteePeriod {
        compute_sync_committee_period_at_slot(ctx, self.slot)
    }

    fn current_sync_committee(&self) -> PublicKey {
        self.current_sync_committee.clone()
    }

    fn next_sync_committee(&self) -> PublicKey {
        self.next_sync_committee.clone()
    }

    fn is_relevant_update(&self, update_finalized_slot: U64) -> bool {
        self.slot < update_finalized_slot
    }
}

impl EthConsensusState for ConsensusState {
    fn storage_root(&self) -> H256 {
        self.storage_root
    }
}

impl ConsensusState {
    pub fn validate(&self) -> Result<(), Error> {
        if self.slot == Default::default() {
            Err(Error::UninitializedConsensusStateField("slot"))
        } else if self.storage_root.as_bytes().is_empty() {
            Err(Error::UninitializedConsensusStateField("storage_root"))
        } else if self.timestamp.as_unix_timestamp_nanos() == 0 {
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

fn timestamp_to_proto_timestamp(timestamp: Time) -> ProtoTimestamp {
    let nanos = timestamp.as_unix_timestamp_nanos();
    ProtoTimestamp {
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
        let timestamp = value
            .timestamp
            .ok_or_else(|| Error::InvalidRawConsensusState {
                reason: "timestamp is none".to_string(),
            })?;
        if value.storage_root.len() != 32 {
            return Err(Error::InvalidRawConsensusState {
                reason: format!("invalid storage_root length: {}", value.storage_root.len()),
            });
        }
        let nanos: u32 =
            timestamp
                .nanos
                .try_into()
                .map_err(|_| Error::InvalidRawConsensusState {
                    reason: format!("invalid timestamp nanos: {}", timestamp.nanos),
                })?;
        Ok(Self {
            slot: value.slot.into(),
            storage_root: H256::from_slice(value.storage_root.as_slice()),
            timestamp: Time::from_unix_timestamp(timestamp.seconds, nanos)?,
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
                .map_err(Error::ProtoDecode)?
                .try_into()
        }

        match raw.type_url.as_str() {
            ETHEREUM_CONSENSUS_STATE_TYPE_URL => decode_consensus_state(raw.value.deref()),
            _ => Err(Error::UnknownConsensusStateType {
                type_url: raw.type_url,
            }),
        }
    }
}

impl TryFrom<ConsensusState> for IBCAny {
    type Error = Error;

    fn try_from(value: ConsensusState) -> Result<Self, Self::Error> {
        let value: RawConsensusState = value.into();
        let mut v = Vec::new();
        value.encode(&mut v).map_err(Error::ProtoEncode)?;
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

impl TryFrom<Any> for ConsensusState {
    type Error = Error;

    fn try_from(any: Any) -> Result<Self, Self::Error> {
        IBCAny::from(any).try_into()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use hex_literal::hex;

    /// Creates a valid test consensus state for testing purposes.
    pub fn create_test_consensus_state() -> ConsensusState {
        // Create a valid 48-byte BLS public key (compressed G1 point)
        let pubkey_bytes: [u8; 48] = hex!(
            "a99a76ed7796f7be22d5b7e85deeb7c5677e88e511e0b337618f8c4eb61349b4bf2d153f649f7b53359fe8b94a38e44c"
        );
        let pubkey = PublicKey::try_from(pubkey_bytes.to_vec()).unwrap();

        ConsensusState {
            slot: Slot::from(100u64),
            storage_root: H256::from_slice(&[1u8; 32]),
            timestamp: Time::from_unix_timestamp_nanos(1_000_000_000_000_000_000).unwrap(),
            current_sync_committee: pubkey.clone(),
            next_sync_committee: pubkey,
        }
    }

    #[test]
    fn test_consensus_state_default() {
        let state = ConsensusState::default();
        assert_eq!(state.slot, Slot::default());
        assert_eq!(state.storage_root, H256::default());
        assert_eq!(state.timestamp.as_unix_timestamp_nanos(), 0);
        assert_eq!(state.current_sync_committee, PublicKey::default());
        assert_eq!(state.next_sync_committee, PublicKey::default());
    }

    #[test]
    fn test_consensus_state_validate_default_fails() {
        let state = ConsensusState::default();
        let result = state.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::UninitializedConsensusStateField("slot")
        ));
    }

    #[test]
    fn test_consensus_state_validate_success() {
        let state = create_test_consensus_state();
        let result = state.validate();
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn test_consensus_state_validate_missing_slot() {
        let state = ConsensusState {
            slot: Slot::default(),
            storage_root: H256::from_slice(&[1u8; 32]),
            timestamp: Time::from_unix_timestamp_nanos(1_000_000_000).unwrap(),
            current_sync_committee: PublicKey::default(),
            next_sync_committee: PublicKey::default(),
        };
        let result = state.validate();
        assert!(matches!(
            result.unwrap_err(),
            Error::UninitializedConsensusStateField("slot")
        ));
    }

    #[test]
    fn test_consensus_state_validate_missing_timestamp() {
        let state = ConsensusState {
            slot: Slot::from(100u64),
            storage_root: H256::from_slice(&[1u8; 32]),
            timestamp: Time::from_unix_timestamp_nanos(0).unwrap(),
            current_sync_committee: PublicKey::default(),
            next_sync_committee: PublicKey::default(),
        };
        let result = state.validate();
        assert!(matches!(
            result.unwrap_err(),
            Error::UninitializedConsensusStateField("timestamp")
        ));
    }

    #[test]
    fn test_consensus_state_validate_missing_current_sync_committee() {
        let state = ConsensusState {
            slot: Slot::from(100u64),
            storage_root: H256::from_slice(&[1u8; 32]),
            timestamp: Time::from_unix_timestamp_nanos(1_000_000_000).unwrap(),
            current_sync_committee: PublicKey::default(),
            next_sync_committee: PublicKey::default(),
        };
        let result = state.validate();
        assert!(matches!(
            result.unwrap_err(),
            Error::UninitializedConsensusStateField("current_sync_committee")
        ));
    }

    #[test]
    fn test_consensus_state_validate_missing_next_sync_committee() {
        let pubkey_bytes: [u8; 48] = hex!(
            "a99a76ed7796f7be22d5b7e85deeb7c5677e88e511e0b337618f8c4eb61349b4bf2d153f649f7b53359fe8b94a38e44c"
        );
        let pubkey = PublicKey::try_from(pubkey_bytes.to_vec()).unwrap();

        let state = ConsensusState {
            slot: Slot::from(100u64),
            storage_root: H256::from_slice(&[1u8; 32]),
            timestamp: Time::from_unix_timestamp_nanos(1_000_000_000).unwrap(),
            current_sync_committee: pubkey,
            next_sync_committee: PublicKey::default(),
        };
        let result = state.validate();
        assert!(matches!(
            result.unwrap_err(),
            Error::UninitializedConsensusStateField("next_sync_committee")
        ));
    }

    #[test]
    fn test_consensus_state_type_url() {
        assert_eq!(
            ETHEREUM_CONSENSUS_STATE_TYPE_URL,
            "/ibc.lightclients.ethereum.v1.ConsensusState"
        );
    }

    #[test]
    fn test_consensus_state_proto_conversion() {
        let consensus_state = create_test_consensus_state();

        // Convert to Any and back
        let any: Any = consensus_state.clone().try_into().unwrap();
        let consensus_state2 = ConsensusState::try_from(any).unwrap();

        assert_eq!(consensus_state, consensus_state2);
    }

    #[test]
    fn test_consensus_state_proto_conversion_ibc_any() {
        let consensus_state = create_test_consensus_state();

        // Convert to IBCAny and back
        let ibc_any: IBCAny = consensus_state.clone().try_into().unwrap();
        assert_eq!(ibc_any.type_url, ETHEREUM_CONSENSUS_STATE_TYPE_URL);

        let consensus_state2 = ConsensusState::try_from(ibc_any).unwrap();
        assert_eq!(consensus_state, consensus_state2);
    }

    #[test]
    fn test_consensus_state_from_any_unknown_type() {
        let any = IBCAny {
            type_url: "/unknown.type".to_string(),
            value: vec![],
        };
        let result = ConsensusState::try_from(any);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::UnknownConsensusStateType { type_url } if type_url == "/unknown.type"
        ));
    }

    #[test]
    fn test_consensus_state_storage_root() {
        let state = create_test_consensus_state();
        assert_eq!(state.storage_root(), state.storage_root);
    }

    #[test]
    fn test_timestamp_to_proto_and_back() {
        let original = create_test_consensus_state();
        let raw: RawConsensusState = original.clone().into();
        let converted = ConsensusState::try_from(raw).unwrap();
        assert_eq!(original.timestamp, converted.timestamp);
    }
}
