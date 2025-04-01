use zksync_multivm::zk_evm_latest::ethereum_types::U256;
use zksync_types::api::TransactionRequest;
use zksync_types::fee::Fee;
use zksync_types::l2::L2Tx;
use zksync_types::transaction_request::PaymasterParams;
use zksync_types::{Address, K256PrivateKey, L2ChainId, Nonce, PackedEthSignature};

pub struct L2TxBuilder {
    from: Address,
    to: Option<Address>,
    nonce: Nonce,
    value: U256,
    gas_limit: U256,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
    calldata: Vec<u8>,
    factory_deps: Vec<Vec<u8>>,
    chain_id: L2ChainId,
}

impl L2TxBuilder {
    pub fn new(
        from: Address,
        nonce: Nonce,
        gas_limit: U256,
        max_fee_per_gas: U256,
        chain_id: L2ChainId,
    ) -> Self {
        Self {
            from,
            to: None,
            nonce,
            value: U256::zero(),
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas: max_fee_per_gas,
            calldata: vec![],
            factory_deps: vec![],
            chain_id,
        }
    }

    pub fn with_to(mut self, to: Address) -> Self {
        self.to = Some(to);
        self
    }

    pub fn with_calldata(mut self, calldata: Vec<u8>) -> Self {
        self.calldata = calldata;
        self
    }

    pub fn with_max_priority_fee_per_gas(mut self, max_priority_fee_per_gas: U256) -> Self {
        self.max_priority_fee_per_gas = max_priority_fee_per_gas;
        self
    }

    pub fn build_impersonated(self) -> L2Tx {
        let mut tx = L2Tx::new(
            self.to,
            self.calldata,
            self.nonce,
            Fee {
                gas_limit: self.gas_limit,
                max_fee_per_gas: self.max_fee_per_gas,
                max_priority_fee_per_gas: self.max_priority_fee_per_gas,
                gas_per_pubdata_limit: U256::from(50_000),
            },
            self.from,
            self.value,
            self.factory_deps,
            PaymasterParams::default(),
        );
        let mut req: TransactionRequest = tx.clone().into();
        req.chain_id = Some(self.chain_id.as_u64());
        let data = req.get_default_signed_message().unwrap();
        let sig = PackedEthSignature::sign_raw(&K256PrivateKey::random(), &data).unwrap();
        let raw = req.get_signed_bytes(&sig).unwrap();
        let (_, hash) = TransactionRequest::from_bytes_unverified(&raw).unwrap();

        tx.set_input(raw, hash);
        tx.common_data.signature = sig.serialize_packed().into();
        tx
    }
}
