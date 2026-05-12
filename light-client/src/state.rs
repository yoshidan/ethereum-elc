use crate::client_state::ClientState;
use crate::consensus_state::ConsensusState;
use crate::errors::Error;
use ethereum_light_client_types::client_state::ClientState as _;
use light_client::commitments::{gen_state_id_from_any, StateID};
use light_client::types::Any;

pub fn gen_state_id<const SYNC_COMMITTEE_SIZE: usize>(
    client_state: ClientState<SYNC_COMMITTEE_SIZE>,
    consensus_state: ConsensusState,
) -> Result<StateID, Error> {
    let client_state = Any::try_from(client_state.canonicalize())?;
    let consensus_state = Any::try_from(consensus_state)?;
    gen_state_id_from_any(&client_state, &consensus_state).map_err(Error::Commitment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_state::tests::create_test_client_state;
    use crate::consensus_state::tests::create_test_consensus_state;

    #[test]
    fn test_gen_state_id() {
        let client_state = create_test_client_state();
        let consensus_state = create_test_consensus_state();
        let result = gen_state_id(client_state, consensus_state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_gen_state_id_deterministic() {
        let client_state1 = create_test_client_state();
        let consensus_state1 = create_test_consensus_state();
        let state_id1 = gen_state_id(client_state1, consensus_state1).unwrap();

        let client_state2 = create_test_client_state();
        let consensus_state2 = create_test_consensus_state();
        let state_id2 = gen_state_id(client_state2, consensus_state2).unwrap();

        assert_eq!(state_id1, state_id2);
    }

    #[test]
    fn test_gen_state_id_canonicalizes_client_state() {
        let mut client_state1 = create_test_client_state();
        client_state1.latest_execution_block_number = 100u64.into();
        let consensus_state1 = create_test_consensus_state();
        let state_id1 = gen_state_id(client_state1, consensus_state1).unwrap();

        let mut client_state2 = create_test_client_state();
        client_state2.latest_execution_block_number = 200u64.into();
        let consensus_state2 = create_test_consensus_state();
        let state_id2 = gen_state_id(client_state2, consensus_state2).unwrap();

        // State IDs should be equal because canonicalize resets latest_execution_block_number
        assert_eq!(state_id1, state_id2);
    }
}
