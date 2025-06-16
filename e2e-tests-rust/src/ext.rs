//! Module containing extensions of existing alloy abstractions.

use alloy::network::{ReceiptResponse, TxSigner};
use alloy::primitives::{Address, BlockHash, PrimitiveSignature};
use alloy::providers::WalletProvider;
use alloy::signers::local::PrivateKeySigner;
use alloy_zksync::network::Zksync;
use alloy_zksync::wallet::ZksyncWallet;

pub trait ReceiptExt: ReceiptResponse {
    fn block_number_ext(&self) -> anyhow::Result<u64> {
        self.block_number().ok_or_else(|| {
            anyhow::anyhow!(
                "receipt (hash={}) does not have block number",
                self.transaction_hash()
            )
        })
    }

    fn block_hash_ext(&self) -> anyhow::Result<BlockHash> {
        self.block_hash().ok_or_else(|| {
            anyhow::anyhow!(
                "receipt (hash={}) does not have block hash",
                self.transaction_hash()
            )
        })
    }

    fn sender(&self) -> anyhow::Result<Address> {
        self.to().ok_or_else(|| {
            anyhow::anyhow!(
                "receipt (hash={}) does not have `to` address",
                self.transaction_hash()
            )
        })
    }

    /// Asserts that receipts belong to a block and that block is the same for both of them.
    fn assert_same_block(&self, other: &Self) -> anyhow::Result<()> {
        let lhs_number = self.block_number_ext()?;
        let rhs_number = other.block_number_ext()?;
        let lhs_hash = self.block_hash_ext()?;
        let rhs_hash = other.block_hash_ext()?;

        if lhs_number == rhs_number && lhs_hash == rhs_hash {
            Ok(())
        } else {
            anyhow::bail!(
                "receipt (hash={}, block={}) is not from the same block as receipt (hash={}, block={})",
                self.transaction_hash(),
                lhs_number,
                other.transaction_hash(),
                rhs_number
            )
        }
    }

    /// Asserts that receipt is successful.
    fn assert_successful(&self) -> anyhow::Result<()> {
        if !self.status() {
            anyhow::bail!(
                "receipt (hash={}, block={:?}) is not successful",
                self.transaction_hash(),
                self.block_number(),
            );
        }
        Ok(())
    }

    /// Asserts that receipt is successful.
    fn assert_reverted(&self, msg: &str) -> anyhow::Result<()> {
        if self.status() {
            anyhow::bail!(
                "receipt (hash={}, block={:?}) is successful",
                self.transaction_hash(),
                self.block_number(),
            );
        }
        Ok(())
    }
}

impl<T: ReceiptResponse> ReceiptExt for T {}

pub trait ZksyncWalletProviderExt: WalletProvider<Zksync, Wallet = ZksyncWallet> {
    /// Creates and registers a random signer. Returns new signer's address.
    fn register_random_signer(&mut self) -> Address {
        let signer = PrivateKeySigner::random();
        let address = signer.address();
        self.wallet_mut().register_signer(signer);
        address
    }

    /// Registers provider signer.
    fn register_signer<T: TxSigner<PrimitiveSignature> + Send + Sync + 'static>(
        &mut self,
        signer: T,
    ) {
        self.wallet_mut().register_signer(signer);
    }
}

impl<T: WalletProvider<Zksync, Wallet = ZksyncWallet>> ZksyncWalletProviderExt for T {}
