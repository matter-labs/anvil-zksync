pub mod execution;
pub mod model;
pub mod storage;
mod tx_conversions;
mod util;
mod zkos_conversions;

pub const BLOCK_REPLAY_WAL_PATH: &str = "../chains/era/db/main/block_replay_wal";
pub const STATE_STORAGE_PATH: &str = "../chains/era/db/main/state";
pub const PREIMAGES_STORAGE_PATH: &str = "../chains/era/db/main/preimages";
const CHAIN_ID: u64 = 270;

// Maximum number of per-block information stored in memory - and thus returned from API.
// Older blocks are discarded (or, in case of state diffs, compacted)
const BLOCKS_TO_RETAIN: usize = 128;

const JSON_RPC_ADDR: &str = "127.0.0.1:3050";

const BLOCK_TIME_MS: u64 = 150;

const MAX_TX_SIZE: usize = 100000;

const MAX_NONCE_AHEAD: u32 = 1000;

const DEFAULT_ETH_CALL_GAS: u32 = 10000000;
