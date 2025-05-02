#[derive(thiserror::Error, Debug)]
pub enum DecodingError {
    #[error("Invalid address: {address}")]
    InvalidAddress { address: String },
}
