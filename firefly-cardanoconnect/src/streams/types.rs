use std::{cmp::Ordering, collections::BTreeMap, time::Duration};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::strong_id;

strong_id!(StreamId, String);

/// Represents a stream of some sort of event.
/// Consumers can create a Listener to listen to events from the stream.
#[derive(Clone, Debug)]
pub struct Stream {
    pub id: StreamId,
    pub name: String,
    pub batch_size: usize,
    pub batch_timeout: Duration,
}

strong_id!(ListenerId, String);

/// Represents some consumer listening on a stream
#[derive(Clone, Debug)]
pub struct Listener {
    pub id: ListenerId,
    pub name: String,
    pub listener_type: ListenerType,
    pub stream_id: StreamId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ListenerType {
    Events,
    Blocks,
}

#[derive(Clone, Debug)]
pub struct StreamCheckpoint {
    pub stream_id: StreamId,
    pub listeners: BTreeMap<ListenerId, BlockReference>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockReference {
    Origin,
    Point(u64, String),
}
impl PartialOrd for BlockReference {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self == other {
            return Some(Ordering::Equal);
        }
        match (self, other) {
            (Self::Origin, Self::Origin) => Some(Ordering::Equal),
            (Self::Origin, _) => Some(Ordering::Less),
            (_, Self::Origin) => Some(Ordering::Greater),
            (
                Self::Point(block_number_l, block_hash_l),
                Self::Point(block_number_r, block_hash_r),
            ) => {
                if block_number_l != block_number_r {
                    block_number_l.partial_cmp(block_number_r)
                } else if block_hash_l != block_hash_r {
                    None
                } else {
                    Some(Ordering::Equal)
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlockInfo {
    pub block_number: u64,
    pub block_hash: String,
    pub parent_hash: String,
    pub transaction_hashes: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct BlockEvent {
    pub listener_id: ListenerId,
    pub block_info: BlockInfo,
}