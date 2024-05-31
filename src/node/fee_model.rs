use std::fmt::Debug;
use zksync_node_fee_model::BatchFeeModelInputProvider;
use zksync_types::fee_model::{FeeModelConfigV2, FeeParams, FeeParamsV2};
use zksync_types::L1_GAS_PER_PUBDATA_BYTE;

#[derive(Debug, Clone, PartialEq)]
pub struct TestNodeFeeInputProvider {
    pub l1_gas_price: u64,
    pub l2_gas_price: u64,
    /// L1 Gas Price Scale Factor for gas estimation.
    pub estimate_gas_price_scale_factor: f64,
    /// The factor by which to scale the gasLimit.
    pub estimate_gas_scale_factor: f32,
}

impl TestNodeFeeInputProvider {
    pub fn new(
        l1_gas_price: u64,
        l2_gas_price: u64,
        estimate_gas_price_scale_factor: f64,
        estimate_gas_scale_factor: f32,
    ) -> Self {
        Self {
            l1_gas_price,
            l2_gas_price,
            estimate_gas_price_scale_factor,
            estimate_gas_scale_factor,
        }
    }

    pub fn get_fee_model_config(&self) -> FeeModelConfigV2 {
        FeeModelConfigV2 {
            minimal_l2_gas_price: self.l2_gas_price,
            compute_overhead_part: 0.0,
            pubdata_overhead_part: 1.0,
            batch_overhead_l1_gas: 800000,
            max_gas_per_batch: 200000000,
            max_pubdata_per_batch: 100000,
        }
    }
}

impl BatchFeeModelInputProvider for TestNodeFeeInputProvider {
    fn get_fee_model_params(&self) -> FeeParams {
        // TODO: consider using old fee model for the olds blocks, when forking
        FeeParams::V2(FeeParamsV2 {
            config: self.get_fee_model_config(),
            l1_gas_price: self.l1_gas_price,
            l1_pubdata_price: self.l1_gas_price * L1_GAS_PER_PUBDATA_BYTE as u64,
        })
    }
}
