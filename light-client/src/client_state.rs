use crate::consensus_state::{ConsensusState, TrustedConsensusState};
use crate::errors::Error;
use crate::header::Header;
use crate::misbehaviour::Misbehaviour;
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::time::Duration;
use ethereum_consensus::beacon::{Epoch, Root, Slot, Version};
use ethereum_consensus::fork::{ForkParameter, ForkParameters, ForkSpec, BELLATRIX_INDEX};
use ethereum_consensus::types::{Address, H256, U64};
use ethereum_elc_proto::ibc::lightclients::ethereum::v1::ClientState as RawClientState;
use ethereum_light_client_proto::google::protobuf::Any as IBCAny;
use ethereum_light_client_proto::ibc::lightclients::ethereum::v1::{
    Fork as RawFork, ForkSpec as RawForkSpec,
};
use ethereum_light_client_types::client_state::ClientState as EthClientState;
use ethereum_light_client_types::commitment::verify_account_storage;
use ethereum_light_client_types::errors::Error as EthError;
use ethereum_light_client_types::time::{
    validate_header_timestamp_not_future, validate_state_timestamp_within_trusting_period,
};
use ethereum_light_client_types::update::compute_sync_committees;
use ethereum_light_client_verifier::consensus::SyncProtocolVerifier;
use ethereum_light_client_verifier::context::{
    ChainConsensusVerificationContext, Fraction, LightClientContext,
};
use ethereum_light_client_verifier::execution::ExecutionVerifier;
use light_client::types::{Any, ClientId, Height, Time};
use prost::Message;
use serde::{Deserialize, Serialize};

/// The revision number for the Ethereum light client is always 0.
///
/// Therefore, in ethereum, the revision number is not used to determine the hard fork.
/// The current fork is determined by the client state's fork parameters.
pub const ETHEREUM_CLIENT_REVISION_NUMBER: u64 = 0;
pub const ETHEREUM_CLIENT_STATE_TYPE_URL: &str = "/ibc.lightclients.ethereum.v1.ClientState";
pub const ETHEREUM_ACCOUNT_STORAGE_ROOT_INDEX: usize = 2;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientState<const SYNC_COMMITTEE_SIZE: usize> {
    // Verification parameters
    /// `genesis_validators_root` of the target beacon chain's BeaconState
    pub genesis_validators_root: Root,
    /// https://github.com/ethereum/consensus-specs/blob/a09d0c321550c5411557674a981e2b444a1178c0/specs/altair/light-client/sync-protocol.md#misc
    pub min_sync_committee_participants: U64,
    /// `genesis_time` of the target beacon chain's BeaconState
    pub genesis_time: U64,
    /// fork parameters of the target beacon chain
    pub fork_parameters: ForkParameters,
    /// https://github.com/ethereum/consensus-specs/blob/a09d0c321550c5411557674a981e2b444a1178c0/configs/mainnet.yaml#L69
    pub seconds_per_slot: U64,
    /// https://github.com/ethereum/consensus-specs/blob/a09d0c321550c5411557674a981e2b444a1178c0/presets/mainnet/phase0.yaml#L36
    pub slots_per_epoch: Slot,
    /// https://github.com/ethereum/consensus-specs/blob/a09d0c321550c5411557674a981e2b444a1178c0/presets/mainnet/altair.yaml#L18
    pub epochs_per_sync_committee_period: Epoch,

    /// An address of IBC contract on execution layer
    pub ibc_address: Address,
    /// The IBC contract's base storage location for storing commitments
    /// https://github.com/hyperledger-labs/yui-ibc-solidity/blob/0e83dc7aadf71380dae6e346492e148685510663/docs/architecture.md#L46
    pub ibc_commitments_slot: H256,

    /// `trust_level` is threshold of sync committee participants to consider the attestation as valid. Highly recommended to be 2/3.
    pub trust_level: Fraction,
    /// `trusting_period` is the period in which the consensus state is considered trusted
    pub trusting_period: Duration,
    /// `max_clock_drift` defines how much new finalized header's time can drift into the future
    pub max_clock_drift: Duration,

    // State
    /// The latest block number of the stored consensus state
    pub latest_execution_block_number: U64,
    /// `frozen_height` is the height at which the client is considered frozen. If `None`, the client is unfrozen.
    pub frozen_height: Option<Height>,

    // Verifiers
    #[serde(skip)]
    pub consensus_verifier:
        SyncProtocolVerifier<SYNC_COMMITTEE_SIZE, TrustedConsensusState<SYNC_COMMITTEE_SIZE>>,
    #[serde(skip)]
    pub execution_verifier: ExecutionVerifier,
}

impl<const SYNC_COMMITTEE_SIZE: usize> EthClientState for ClientState<SYNC_COMMITTEE_SIZE> {
    fn is_frozen(&self) -> bool {
        self.frozen_height.is_some()
    }

    fn latest_height(&self) -> Height {
        Height::new(
            ETHEREUM_CLIENT_REVISION_NUMBER,
            self.latest_execution_block_number.into(),
        )
    }

    fn ibc_commitments_slot(&self) -> H256 {
        self.ibc_commitments_slot
    }

    fn canonicalize(self) -> Self {
        let mut client_state = self;
        client_state.latest_execution_block_number = 0u64.into();
        client_state.frozen_height = None;
        client_state
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> ClientState<SYNC_COMMITTEE_SIZE> {
    pub fn with_frozen_height(self, h: Height) -> Self {
        Self {
            frozen_height: Some(h),
            ..self
        }
    }

    pub fn build_context(&self, current_timestamp: Time) -> impl ChainConsensusVerificationContext {
        LightClientContext::new(
            self.fork_parameters.clone(),
            self.seconds_per_slot,
            self.slots_per_epoch,
            self.epochs_per_sync_committee_period,
            self.genesis_time,
            self.genesis_validators_root,
            self.min_sync_committee_participants.0 as usize,
            self.trust_level.clone(),
            current_timestamp.as_unix_timestamp_secs().into(),
        )
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.genesis_validators_root == Root::default() {
            Err(Error::UninitializedClientStateField(
                "genesis_validators_root",
            ))
        } else if self.min_sync_committee_participants == U64::default() {
            Err(Error::UninitializedClientStateField(
                "min_sync_committee_participants",
            ))
        } else if self.genesis_time == U64::default() {
            Err(Error::UninitializedClientStateField("genesis_time"))
        } else if self.fork_parameters == ForkParameters::default() {
            Err(Error::UninitializedClientStateField("fork_parameters"))
        } else if self.fork_parameters.forks().len() <= BELLATRIX_INDEX {
            Err(Error::MissingBellatrixFork)
        } else if self.seconds_per_slot == U64::default() {
            Err(Error::UninitializedClientStateField("seconds_per_slot"))
        } else if self.slots_per_epoch == Slot::default() {
            Err(Error::UninitializedClientStateField("slots_per_epoch"))
        } else if self.epochs_per_sync_committee_period == U64::default() {
            Err(Error::UninitializedClientStateField(
                "epochs_per_sync_committee_period",
            ))
        } else if self.ibc_address == Address::default() {
            Err(Error::UninitializedClientStateField("ibc_address"))
        } else if self.trust_level == Fraction::default() {
            Err(Error::UninitializedClientStateField("trust_level"))
        } else if self.trusting_period == Duration::default() {
            Err(Error::UninitializedClientStateField("trusting_period"))
        } else if self.latest_execution_block_number == U64::default() {
            Err(Error::UninitializedClientStateField(
                "latest_execution_block_number",
            ))
        } else {
            Ok(())
        }
    }

    pub fn check_header_and_update_state(
        &self,
        now: Time,
        consensus_state: &ConsensusState,
        header: Header<SYNC_COMMITTEE_SIZE>,
    ) -> Result<(ClientState<SYNC_COMMITTEE_SIZE>, ConsensusState), Error> {
        let cc = self.build_context(now);
        header.validate(&cc)?;

        let trusted_sync_committee = header.trusted_sync_committee;
        let trusted_consensus_state = TrustedConsensusState::new(
            consensus_state.clone(),
            trusted_sync_committee.sync_committee,
            trusted_sync_committee.is_next,
        )?;
        let consensus_update = header.consensus_update;
        let execution_update = header.execution_update;
        let account_update = header.account_update;
        let header_timestamp = header.timestamp;

        self.consensus_verifier
            .validate_updates(
                &cc,
                &trusted_consensus_state,
                &consensus_update,
                &execution_update,
            )
            .map_err(Error::VerificationError)?;

        verify_account_storage(
            &self.execution_verifier,
            execution_update.state_root,
            &self.ibc_address,
            &account_update,
        )?;

        // check if the current timestamp is within the trusting period
        validate_state_timestamp_within_trusting_period(
            now,
            self.trusting_period,
            consensus_state.timestamp,
        )?;
        // check if the header timestamp does not indicate a future time
        validate_header_timestamp_not_future(now, self.max_clock_drift, header_timestamp)?;

        let new_sync_committee = compute_sync_committees(&cc, consensus_state, consensus_update)?;

        // apply updates to state
        let mut new_client_state: ClientState<SYNC_COMMITTEE_SIZE> = self.clone();
        if new_client_state.latest_execution_block_number < execution_update.block_number {
            new_client_state.latest_execution_block_number = execution_update.block_number;
        }
        let mut new_consensus_state = consensus_state.clone();
        new_consensus_state.storage_root = execution_update.state_root;
        new_consensus_state.timestamp = header_timestamp;
        new_consensus_state.current_sync_committee = new_sync_committee.current_sync_committee;
        new_consensus_state.next_sync_committee = new_sync_committee.next_sync_committee;

        Ok((new_client_state, new_consensus_state))
    }

    pub fn check_misbehaviour_and_update_state(
        &self,
        now: Time,
        client_id: &ClientId,
        consensus_state: &ConsensusState,
        misbehaviour: Misbehaviour<SYNC_COMMITTEE_SIZE>,
    ) -> Result<ClientState<SYNC_COMMITTEE_SIZE>, Error> {
        misbehaviour.validate()?;
        if &misbehaviour.client_id != client_id {
            return Err(Error::UnexpectedClientIdInMisbehaviour(
                client_id.clone(),
                misbehaviour.client_id,
            ));
        }

        let cc = self.build_context(now);
        let trusted_consensus_state = TrustedConsensusState::new(
            consensus_state.clone(),
            misbehaviour.trusted_sync_committee.sync_committee,
            misbehaviour.trusted_sync_committee.is_next,
        )?;

        self.consensus_verifier
            .validate_misbehaviour(&cc, &trusted_consensus_state, misbehaviour.data)
            .map_err(Error::VerificationError)?;

        validate_state_timestamp_within_trusting_period(
            now,
            self.trusting_period,
            consensus_state.timestamp,
        )?;

        // found misbehaviour
        Ok(self
            .clone()
            .with_frozen_height(misbehaviour.trusted_sync_committee.height))
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<RawClientState>
    for ClientState<SYNC_COMMITTEE_SIZE>
{
    type Error = Error;

    fn try_from(value: RawClientState) -> Result<Self, Self::Error> {
        fn bytes_to_version(bz: Vec<u8>) -> Version {
            assert_eq!(bz.len(), 4);
            let mut version = Version::default();
            version.0.copy_from_slice(&bz);
            version
        }

        fn convert_fork_spec(idx: usize, spec: Option<RawForkSpec>) -> Result<ForkSpec, Error> {
            if let Some(spec) = spec {
                Ok(ForkSpec {
                    finalized_root_gindex: spec.finalized_root_gindex,
                    current_sync_committee_gindex: spec.current_sync_committee_gindex,
                    next_sync_committee_gindex: spec.next_sync_committee_gindex,
                    execution_payload_gindex: spec.execution_payload_gindex,
                    execution_payload_state_root_gindex: spec.execution_payload_state_root_gindex,
                    execution_payload_block_number_gindex: spec
                        .execution_payload_block_number_gindex,
                    execution_block_hash_gindex: spec.execution_block_hash_gindex,
                })
            } else {
                Err(EthError::proto_missing(&format!("forks[{}].spec", idx)).into())
            }
        }

        let raw_fork_parameters = value
            .fork_parameters
            .ok_or(Error::proto_missing("fork_parameters"))?;
        let fork_parameters: ForkParameters = ForkParameters::new(
            bytes_to_version(raw_fork_parameters.genesis_fork_version),
            raw_fork_parameters
                .forks
                .into_iter()
                .enumerate()
                .map(|(i, f)| -> Result<_, Error> {
                    Ok(ForkParameter::new(
                        bytes_to_version(f.version),
                        f.epoch.into(),
                        convert_fork_spec(i, f.spec)?,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(Error::EthereumConsensusError)?;
        let trust_level = value
            .trust_level
            .ok_or(Error::proto_missing("trust_level"))?;
        let frozen_height = value
            .frozen_height
            .map(|h| Height::new(h.revision_number, h.revision_height));
        Ok(Self {
            genesis_validators_root: H256::from_slice(&value.genesis_validators_root),
            min_sync_committee_participants: value.min_sync_committee_participants.into(),
            genesis_time: value.genesis_time.into(),
            fork_parameters,
            seconds_per_slot: value.seconds_per_slot.into(),
            slots_per_epoch: value.slots_per_epoch.into(),
            epochs_per_sync_committee_period: value.epochs_per_sync_committee_period.into(),
            ibc_address: value
                .ibc_address
                .as_slice()
                .try_into()
                .map_err(|e| Error::UnexpectedStoreAddress(format!("{:?}", e)))?,
            ibc_commitments_slot: H256::from_slice(&value.ibc_commitments_slot),
            trust_level: Fraction::new(trust_level.numerator, trust_level.denominator)
                .map_err(Error::VerificationError)?,
            trusting_period: value
                .trusting_period
                .ok_or(Error::MissingTrustingPeriod)?
                .try_into()
                .map_err(|_| Error::MissingTrustingPeriod)?,
            max_clock_drift: value
                .max_clock_drift
                .ok_or(Error::NegativeMaxClockDrift)?
                .try_into()
                .map_err(|_| Error::NegativeMaxClockDrift)?,
            latest_execution_block_number: value.latest_execution_block_number.into(),
            frozen_height,
            consensus_verifier: Default::default(),
            execution_verifier: Default::default(),
        })
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> From<ClientState<SYNC_COMMITTEE_SIZE>> for RawClientState {
    fn from(value: ClientState<SYNC_COMMITTEE_SIZE>) -> Self {
        use ethereum_light_client_proto::ibc::core::client::v1::Height as ProtoHeight;
        use ethereum_light_client_proto::ibc::lightclients::ethereum::v1::{
            ForkParameters as ProtoForkParameters, Fraction as ProtoFraction,
        };

        fn make_fork(version: &Version, epoch: U64, spec: ForkSpec) -> RawFork {
            RawFork {
                version: version_to_bytes(version),
                epoch: epoch.into(),
                spec: Some(RawForkSpec {
                    finalized_root_gindex: spec.finalized_root_gindex,
                    current_sync_committee_gindex: spec.current_sync_committee_gindex,
                    next_sync_committee_gindex: spec.next_sync_committee_gindex,
                    execution_payload_gindex: spec.execution_payload_gindex,
                    execution_payload_state_root_gindex: spec.execution_payload_state_root_gindex,
                    execution_payload_block_number_gindex: spec
                        .execution_payload_block_number_gindex,
                    execution_block_hash_gindex: spec.execution_block_hash_gindex,
                }),
            }
        }

        fn version_to_bytes(version: &Version) -> Vec<u8> {
            version.0.to_vec()
        }

        let fork_parameters = value.fork_parameters;

        Self {
            genesis_validators_root: value.genesis_validators_root.as_bytes().to_vec(),
            min_sync_committee_participants: value.min_sync_committee_participants.into(),
            genesis_time: value.genesis_time.into(),
            fork_parameters: Some(ProtoForkParameters {
                genesis_fork_version: version_to_bytes(fork_parameters.genesis_version()),
                forks: fork_parameters
                    .forks()
                    .iter()
                    .map(|f| make_fork(&f.version, f.epoch, f.spec.clone()))
                    .collect(),
            }),
            seconds_per_slot: value.seconds_per_slot.into(),
            slots_per_epoch: value.slots_per_epoch.into(),
            epochs_per_sync_committee_period: value.epochs_per_sync_committee_period.into(),
            ibc_address: value.ibc_address.0.to_vec(),
            ibc_commitments_slot: value.ibc_commitments_slot.as_bytes().to_vec(),
            trust_level: Some(ProtoFraction {
                numerator: value.trust_level.numerator(),
                denominator: value.trust_level.denominator(),
            }),
            trusting_period: Some(value.trusting_period.into()),
            max_clock_drift: Some(value.max_clock_drift.into()),
            latest_execution_block_number: value.latest_execution_block_number.into(),
            frozen_height: value.frozen_height.map(|h| ProtoHeight {
                revision_number: h.revision_number(),
                revision_height: h.revision_height(),
            }),
        }
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<IBCAny> for ClientState<SYNC_COMMITTEE_SIZE> {
    type Error = Error;

    fn try_from(any: IBCAny) -> Result<Self, Self::Error> {
        if any.type_url != ETHEREUM_CLIENT_STATE_TYPE_URL {
            return Err(Error::UnknownClientStateType(any.type_url));
        }
        RawClientState::decode(any.value.as_slice())
            .map_err(Error::ProtoDecodeError)?
            .try_into()
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<ClientState<SYNC_COMMITTEE_SIZE>> for IBCAny {
    type Error = Error;

    fn try_from(value: ClientState<SYNC_COMMITTEE_SIZE>) -> Result<Self, Self::Error> {
        let value: RawClientState = value.into();
        let mut v = Vec::new();
        value.encode(&mut v).map_err(Error::ProtoEncodeError)?;
        Ok(Self {
            type_url: ETHEREUM_CLIENT_STATE_TYPE_URL.to_string(),
            value: v,
        })
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<ClientState<SYNC_COMMITTEE_SIZE>> for Any {
    type Error = Error;
    fn try_from(value: ClientState<SYNC_COMMITTEE_SIZE>) -> Result<Self, Error> {
        Ok(IBCAny::try_from(value)?.into())
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<Any> for ClientState<SYNC_COMMITTEE_SIZE> {
    type Error = Error;

    fn try_from(any: Any) -> Result<Self, Self::Error> {
        IBCAny::from(any).try_into()
    }
}
