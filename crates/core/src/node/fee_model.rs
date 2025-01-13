use zksync_multivm::utils::derive_base_fee_and_gas_per_pubdata;
use zksync_multivm::VmVersion;
use zksync_types::fee_model::{
    BaseTokenConversionRatio, BatchFeeInput, FeeModelConfigV2, FeeParams, FeeParamsV2,
};

use super::inner::fork::ForkDetails;
use anvil_zksync_config::constants::{
    DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR, DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
    DEFAULT_FAIR_PUBDATA_PRICE, DEFAULT_L1_GAS_PRICE, DEFAULT_L2_GAS_PRICE,
};

#[derive(Debug, Clone)]
pub struct TestNodeFeeInputProvider {
    /// L1 Gas Price Scale Factor for gas estimation.
    pub estimate_gas_price_scale_factor: f64,
    /// The factor by which to scale the gasLimit.
    pub estimate_gas_scale_factor: f32,

    fee_params: FeeParamsV2,
    forced_base_fee: Option<u64>,
}

// TODO: Derive PartialEq for `FeeParamsV2` in upstream
impl PartialEq for TestNodeFeeInputProvider {
    fn eq(&self, other: &Self) -> bool {
        fn eq_config(a: FeeModelConfigV2, b: FeeModelConfigV2) -> bool {
            a.minimal_l2_gas_price == b.minimal_l2_gas_price
                && a.compute_overhead_part == b.compute_overhead_part
                && a.pubdata_overhead_part == b.pubdata_overhead_part
                && a.batch_overhead_l1_gas == b.batch_overhead_l1_gas
                && a.max_gas_per_batch == b.max_gas_per_batch
                && a.max_pubdata_per_batch == b.max_pubdata_per_batch
        }

        self.estimate_gas_price_scale_factor == other.estimate_gas_price_scale_factor
            && self.estimate_gas_scale_factor == other.estimate_gas_scale_factor
            && self.fee_params.l1_gas_price() == other.fee_params.l1_gas_price()
            && self.fee_params.l1_pubdata_price() == other.fee_params.l1_pubdata_price()
            && eq_config(self.fee_params.config(), other.fee_params.config())
    }
}

impl TestNodeFeeInputProvider {
    pub fn from_fork(fork: Option<&ForkDetails>) -> Self {
        if let Some(fork) = fork {
            if let Some(params) = fork.fee_params {
                TestNodeFeeInputProvider::from_fee_params_and_estimate_scale_factors(
                    params,
                    fork.estimate_gas_price_scale_factor,
                    fork.estimate_gas_scale_factor,
                )
            } else {
                TestNodeFeeInputProvider::from_estimate_scale_factors(
                    fork.estimate_gas_price_scale_factor,
                    fork.estimate_gas_scale_factor,
                )
            }
        } else {
            TestNodeFeeInputProvider::default()
        }
    }

    pub fn from_fee_params_and_estimate_scale_factors(
        fee_params: FeeParams,
        estimate_gas_price_scale_factor: f64,
        estimate_gas_scale_factor: f32,
    ) -> Self {
        match fee_params {
            FeeParams::V1(_) => todo!(),
            FeeParams::V2(fee_params) => Self {
                estimate_gas_price_scale_factor,
                estimate_gas_scale_factor,
                fee_params,
                forced_base_fee: None,
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

    pub fn get_fee_model_config(&self) -> FeeModelConfigV2 {
        self.fee_params.config()
    }

    fn get_params(&self) -> FeeParams {
        // TODO: consider using old fee model for the olds blocks, when forking
        FeeParams::V2(self.fee_params)
    }

    fn enforce_base_fee(&self, mut fee_input: BatchFeeInput) -> BatchFeeInput {
        if let Some(base_fee) = self.forced_base_fee {
            let mut pubdata_fee_input = fee_input.into_pubdata_independent();
            pubdata_fee_input.fair_l2_gas_price = base_fee;
            fee_input = BatchFeeInput::PubdataIndependent(pubdata_fee_input);
        }
        fee_input
    }

    pub(crate) fn get_batch_fee_input(&self) -> BatchFeeInput {
        let fee_input = self.get_params().scale(1.0, 1.0);
        self.enforce_base_fee(fee_input)
    }

    pub(crate) fn get_batch_fee_input_scaled(&self) -> BatchFeeInput {
        let scale_factor = self.estimate_gas_price_scale_factor;
        let fee_input = self.get_params().scale(scale_factor, scale_factor);
        self.enforce_base_fee(fee_input)
    }

    pub fn gas_price(&self) -> u64 {
        let (base_fee, _) = derive_base_fee_and_gas_per_pubdata(
            self.get_batch_fee_input_scaled(),
            VmVersion::latest(),
        );
        base_fee
    }

    pub fn fair_pubdata_price(&self) -> u64 {
        self.get_batch_fee_input_scaled().fair_pubdata_price()
    }

    pub fn set_base_fee(&mut self, base_fee: u64) {
        self.forced_base_fee = Some(base_fee);
    }
}

impl Default for TestNodeFeeInputProvider {
    fn default() -> Self {
        Self {
            estimate_gas_price_scale_factor: DEFAULT_ESTIMATE_GAS_PRICE_SCALE_FACTOR,
            estimate_gas_scale_factor: DEFAULT_ESTIMATE_GAS_SCALE_FACTOR,
            fee_params: FeeParamsV2::new(
                FeeModelConfigV2 {
                    minimal_l2_gas_price: DEFAULT_L2_GAS_PRICE,
                    compute_overhead_part: 0.0,
                    pubdata_overhead_part: 1.0,
                    batch_overhead_l1_gas: 800000,
                    max_gas_per_batch: 200000000,
                    max_pubdata_per_batch: 500000,
                },
                DEFAULT_L1_GAS_PRICE,
                DEFAULT_FAIR_PUBDATA_PRICE,
                BaseTokenConversionRatio::default(),
            ),
            forced_base_fee: None,
        }
    }
}
