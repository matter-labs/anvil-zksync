use crate::fork::{SerializableForkStorage, SerializableStorage};
use crate::node::TransactionResult;
use serde::{Deserialize, Serialize};
use zksync_types::api::{Block, TransactionVariant};
use zksync_types::web3::Bytes;
use zksync_types::H256;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VersionedState {
    V1 {
        version: StateVersion<1>,
        #[serde(flatten)]
        state: StateV1,
    },
    Unknown {
        version: u8,
    },
}

impl VersionedState {
    pub fn v1(state: StateV1) -> Self {
        VersionedState::V1 {
            version: StateVersion::<1>,
            state,
        }
    }
}

/// Workaround while serde does not allow integer tags in enums (see https://github.com/serde-rs/serde/issues/745).
#[derive(Copy, Clone, Debug)]
pub struct StateVersion<const V: u8>;

impl<const V: u8> Serialize for StateVersion<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(V)
    }
}

impl<'de, const V: u8> Deserialize<'de> for StateVersion<V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        if value == V {
            Ok(StateVersion::<V>)
        } else {
            Err(serde::de::Error::custom("unknown state version"))
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateV1 {
    /// All blocks sealed on this node up to the current moment.
    pub blocks: Vec<Block<TransactionVariant>>,
    /// All transactions executed on this node up to the current moment.
    pub transactions: Vec<TransactionResult>,
    /// Current node's storage state.
    #[serde(flatten)]
    pub fork_storage: SerializableForkStorage,
    /// Historical states of storage at particular block hashes.
    pub historical_states: Vec<(H256, SerializableStorage)>,
}
