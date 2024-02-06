use zksync_types::L1_GAS_PER_PUBDATA_BYTE;
use zksync_types::fee_model::{BatchFeeInput, FeeModelConfigV2, PubdataIndependentBatchFeeModelInput};
use zksync_basic_types::U256;
use zksync_utils::ceil_div_u256;

const CONFIG: FeeModelConfigV2 = FeeModelConfigV2 {
    minimal_l2_gas_price: 100000000,
    compute_overhead_part: 0.0,
    pubdata_overhead_part: 1.0,
    batch_overhead_l1_gas: 800000,
    max_gas_per_batch: 200000000,
    max_pubdata_per_batch: 100000,
};

pub fn compute_batch_fee_model_input(
    l1_gas_price: u64,
    l1_gas_price_scale_factor: f64,
    l1_pubdata_price_scale_factor: f64,
) -> BatchFeeInput {
    let l1_pubdata_price = l1_gas_price * L1_GAS_PER_PUBDATA_BYTE as u64;

    let FeeModelConfigV2 {
        minimal_l2_gas_price,
        compute_overhead_part,
        pubdata_overhead_part,
        batch_overhead_l1_gas,
        max_gas_per_batch,
        max_pubdata_per_batch,
    } = CONFIG;

    // Firstly, we scale the gas price and pubdata price in case it is needed.
    let l1_gas_price = (l1_gas_price as f64 * l1_gas_price_scale_factor) as u64;
    let l1_pubdata_price = (l1_pubdata_price as f64 * l1_pubdata_price_scale_factor) as u64;

    // While the final results of the calculations are not expected to have any overflows, the intermediate computations
    // might, so we use U256 for them.
    let l1_batch_overhead_wei = U256::from(l1_gas_price) * U256::from(batch_overhead_l1_gas);

    let fair_l2_gas_price = {
        // Firstly, we calculate which part of the overall overhead overhead each unit of L2 gas should cover.
        let l1_batch_overhead_per_gas =
            ceil_div_u256(l1_batch_overhead_wei, U256::from(max_gas_per_batch));

        // Then, we multiply by the `compute_overhead_part` to get the overhead for the computation for each gas.
        // Also, this means that if we almost never close batches because of compute, the `compute_overhead_part` should be zero and so
        // it is possible that the computation costs include for no overhead.
        let gas_overhead_wei =
            (l1_batch_overhead_per_gas.as_u64() as f64 * compute_overhead_part) as u64;

        // We sum up the minimal L2 gas price (i.e. the raw prover/compute cost of a single L2 gas) and the overhead for batch being closed.
        minimal_l2_gas_price + gas_overhead_wei
    };

    let fair_pubdata_price = {
        // Firstly, we calculate which part of the overall overhead overhead each pubdata byte should cover.
        let l1_batch_overhead_per_pubdata =
            ceil_div_u256(l1_batch_overhead_wei, U256::from(max_pubdata_per_batch));

        // Then, we multiply by the `pubdata_overhead_part` to get the overhead for each pubdata byte.
        // Also, this means that if we almost never close batches because of pubdata, the `pubdata_overhead_part` should be zero and so
        // it is possible that the pubdata costs include no overhead.
        let pubdata_overhead_wei =
            (l1_batch_overhead_per_pubdata.as_u64() as f64 * pubdata_overhead_part) as u64;

        // We sum up the raw L1 pubdata price (i.e. the expected price of publishing a single pubdata byte) and the overhead for batch being closed.
        l1_pubdata_price + pubdata_overhead_wei
    };

    BatchFeeInput::PubdataIndependent(PubdataIndependentBatchFeeModelInput {
        l1_gas_price,
        fair_l2_gas_price,
        fair_pubdata_price,
    })
}
