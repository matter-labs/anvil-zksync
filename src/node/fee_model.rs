use zksync_types::fee_model::{BatchFeeInput, FeeModelConfigV2, FeeParams, FeeParamsV2};
use zksync_types::L1_GAS_PER_PUBDATA_BYTE;

use crate::config::gas::{
    GasConfig, DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
    DEFAULT_L1_GAS_PRICE, DEFAULT_L2_GAS_PRICE,
};
use crate::utils::to_human_size;

#[derive(Debug, Clone, PartialEq)]
pub struct TestNodeFeeInputProvider {
    pub l1_gas_price: u64,
    pub l1_pubdata_price: u64,
    pub l2_gas_price: u64,
    pub compute_overhead_part: f64,
    pub pubdata_overhead_part: f64,
    pub batch_overhead_l1_gas: u64,
    pub max_gas_per_batch: u64,
    pub max_pubdata_per_batch: u64,
    /// L1 Gas Price Scale Factor for gas estimation.
    pub estimate_gas_price_scale_factor: f64,
    /// The factor by which to scale the gasLimit.
    pub estimate_gas_scale_factor: f32,
}

impl TestNodeFeeInputProvider {
    pub fn from_fee_params_and_estimate_scale_factors(
        fee_params: FeeParams,
        estimate_gas_price_scale_factor: f64,
        estimate_gas_scale_factor: f32,
    ) -> Self {
        match fee_params {
            FeeParams::V1(_) => todo!(),
            FeeParams::V2(fee_params) => Self {
                l1_gas_price: fee_params.l1_gas_price(),
                l1_pubdata_price: fee_params.l1_pubdata_price(),
                l2_gas_price: fee_params.config().minimal_l2_gas_price,
                compute_overhead_part: fee_params.config().compute_overhead_part,
                pubdata_overhead_part: fee_params.config().pubdata_overhead_part,
                batch_overhead_l1_gas: fee_params.config().batch_overhead_l1_gas,
                max_gas_per_batch: fee_params.config().max_gas_per_batch,
                max_pubdata_per_batch: fee_params.config().max_pubdata_per_batch,
                estimate_gas_price_scale_factor,
                estimate_gas_scale_factor,
            },
        }
    }

    pub fn from_estimate_scale_factors(
        estimate_gas_price_scale_factor: f64,
        estimate_gas_scale_factor: f32,
    ) -> Self {
        Self {
            estimate_gas_price_scale_factor,
            estimate_gas_scale_factor,
            ..Default::default()
        }
    }

    pub fn with_overrides(mut self, gas_config: Option<GasConfig>) -> Self {
        let Some(gas_config) = gas_config else {
            return self;
        };

        if let Some(l1_gas_price) = gas_config.l1_gas_price {
            tracing::info!(
                "L1 gas price set to {} (overridden from {})",
                to_human_size(l1_gas_price.into()),
                to_human_size(self.l1_gas_price.into())
            );
            self.l1_gas_price = l1_gas_price;
        }
        if let Some(l2_gas_price) = gas_config.l2_gas_price {
            tracing::info!(
                "L2 gas price set to {} (overridden from {})",
                to_human_size(l2_gas_price.into()),
                to_human_size(self.l2_gas_price.into())
            );
            self.l2_gas_price = l2_gas_price;
        }

        if let Some(estimation) = gas_config.estimation {
            if let Some(factor) = estimation.price_scale_factor {
                self.estimate_gas_price_scale_factor = factor;
            }
            if let Some(factor) = estimation.limit_scale_factor {
                self.estimate_gas_scale_factor = factor;
            }
        }

        self
    }

    pub fn get_fee_model_config(&self) -> FeeModelConfigV2 {
        FeeModelConfigV2 {
            minimal_l2_gas_price: self.l2_gas_price,
            compute_overhead_part: self.compute_overhead_part,
            pubdata_overhead_part: self.pubdata_overhead_part,
            batch_overhead_l1_gas: self.batch_overhead_l1_gas,
            max_gas_per_batch: self.max_gas_per_batch,
            max_pubdata_per_batch: self.max_pubdata_per_batch,
        }
    }

    fn get_params(&self) -> FeeParams {
        // TODO: consider using old fee model for the olds blocks, when forking
        FeeParams::V2(FeeParamsV2::new(
            self.get_fee_model_config(),
            self.l1_gas_price,
            self.l1_pubdata_price,
            Default::default(),
        ))
    }

    pub(crate) fn get_batch_fee_input(&self) -> BatchFeeInput {
        self.get_params().scale(1.0, 1.0)
    }

    pub(crate) fn get_batch_fee_input_scaled(&self) -> BatchFeeInput {
        let scale_factor = self.estimate_gas_price_scale_factor;
        self.get_params().scale(scale_factor, scale_factor)
    }
}

impl Default for TestNodeFeeInputProvider {
    fn default() -> Self {
        Self {
            l1_gas_price: DEFAULT_L1_GAS_PRICE,
            l1_pubdata_price: DEFAULT_L1_GAS_PRICE * L1_GAS_PER_PUBDATA_BYTE as u64,
            l2_gas_price: DEFAULT_L2_GAS_PRICE,
            compute_overhead_part: 0.0,
            pubdata_overhead_part: 1.0,
            batch_overhead_l1_gas: 800000,
            max_gas_per_batch: 200000000,
            max_pubdata_per_batch: 100000,
            estimate_gas_price_scale_factor: DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
            estimate_gas_scale_factor: DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
        }
    }
}
