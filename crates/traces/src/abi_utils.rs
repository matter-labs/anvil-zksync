//! ABI related helper functions.
//////////////////////////////////////////////////////////////////////////////////////
// Attribution: File adapted from the `foundry-common` crate                        //
//                                                                                  //
// Full credit goes to its authors. See the original implementation here:           //
// https://github.com/foundry-rs/foundry/blob/master/crates/common/src/abi.rs.      //
//                                                                                  //
// Note: These methods are used under the terms of the original project's license.  //
//////////////////////////////////////////////////////////////////////////////////////

use super::types::LogData;
use alloy::dyn_abi::{DynSolType, DynSolValue, FunctionExt, JsonAbiExt};
use alloy::json_abi::{Error, Event, Function, Param};
use alloy::primitives::hex;
use eyre::{Context, Result};

pub fn encode_args<I, S>(inputs: &[Param], args: I) -> Result<Vec<DynSolValue>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    std::iter::zip(inputs, args)
        .map(|(input, arg)| coerce_value(&input.selector_type(), arg.as_ref()))
        .collect()
}

/// Given a function and a vector of string arguments, it proceeds to convert the args to alloy
/// [DynSolValue]s and then ABI encode them.
pub fn encode_function_args<I, S>(func: &Function, args: I) -> Result<Vec<u8>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(func.abi_encode_input(&encode_args(&func.inputs, args)?)?)
}

/// Given a function and a vector of string arguments, it proceeds to convert the args to alloy
/// [DynSolValue]s and encode them using the packed encoding.
pub fn encode_function_args_packed<I, S>(func: &Function, args: I) -> Result<Vec<u8>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let params: Vec<Vec<u8>> = std::iter::zip(&func.inputs, args)
        .map(|(input, arg)| coerce_value(&input.selector_type(), arg.as_ref()))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(|v| v.abi_encode_packed())
        .collect();

    Ok(params.concat())
}

/// Decodes the calldata of the function
pub fn abi_decode_calldata(
    sig: &str,
    calldata: &str,
    input: bool,
    fn_selector: bool,
) -> Result<Vec<DynSolValue>> {
    let func = get_func(sig)?;
    let calldata = hex::decode(calldata)?;

    let mut calldata = calldata.as_slice();
    // If function selector is prefixed in "calldata", remove it (first 4 bytes)
    if input && fn_selector && calldata.len() >= 4 {
        calldata = &calldata[4..];
    }

    let res = if input {
        func.abi_decode_input(calldata, false)
    } else {
        func.abi_decode_output(calldata, false)
    }?;

    // in case the decoding worked but nothing was decoded
    if res.is_empty() {
        eyre::bail!("no data was decoded")
    }

    Ok(res)
}

/// Given a function signature string, it tries to parse it as a `Function`
pub fn get_func(sig: &str) -> Result<Function> {
    Function::parse(sig).wrap_err("could not parse function signature")
}

/// Given an event signature string, it tries to parse it as a `Event`
pub fn get_event(sig: &str) -> Result<Event> {
    Event::parse(sig).wrap_err("could not parse event signature")
}

/// Given an error signature string, it tries to parse it as a `Error`
pub fn get_error(sig: &str) -> Result<Error> {
    Error::parse(sig).wrap_err("could not parse event signature")
}

/// Given an event without indexed parameters and a rawlog, it tries to return the event with the
/// proper indexed parameters. Otherwise, it returns the original event.
pub fn get_indexed_event(mut event: Event, raw_log: &LogData) -> Event {
    if !event.anonymous && raw_log.topics().len() > 1 {
        let indexed_params = raw_log.topics().len() - 1;
        let num_inputs = event.inputs.len();
        let num_address_params = event.inputs.iter().filter(|p| p.ty == "address").count();

        event
            .inputs
            .iter_mut()
            .enumerate()
            .for_each(|(index, param)| {
                if param.name.is_empty() {
                    param.name = format!("param{index}");
                }
                if num_inputs == indexed_params
                    || (num_address_params == indexed_params && param.ty == "address")
                {
                    param.indexed = true;
                }
            })
    }
    event
}

/// Helper function to coerce a value to a [DynSolValue] given a type string
pub fn coerce_value(ty: &str, arg: &str) -> Result<DynSolValue> {
    let ty = DynSolType::parse(ty)?;
    Ok(DynSolType::coerce_str(&ty, arg)?)
}

#[cfg(test)]
mod tests {
    use crate::decode::{get_indexed_event_from_vm_event, vm_event_to_log_data};

    use super::*;
    use alloy::dyn_abi::EventExt;
    use alloy::primitives::{Address, B256, U256};
    use zksync_multivm::interface::VmEvent;
    use zksync_types::H256;

    #[test]
    fn test_get_func() {
        let func = get_func("function foo(uint256 a, uint256 b) returns (uint256)");
        assert!(func.is_ok());
        let func = func.unwrap();
        assert_eq!(func.name, "foo");
        assert_eq!(func.inputs.len(), 2);
        assert_eq!(func.inputs[0].ty, "uint256");
        assert_eq!(func.inputs[1].ty, "uint256");

        // Stripped down function, which [Function] can parse.
        let func = get_func("foo(bytes4 a, uint8 b)(bytes4)");
        assert!(func.is_ok());
        let func = func.unwrap();
        assert_eq!(func.name, "foo");
        assert_eq!(func.inputs.len(), 2);
        assert_eq!(func.inputs[0].ty, "bytes4");
        assert_eq!(func.inputs[1].ty, "uint8");
        assert_eq!(func.outputs[0].ty, "bytes4");
    }

    #[test]
    fn test_indexed_only_address_vm() {
        let event = get_event("event Ev(address,uint256,address)").unwrap();

        let param0 = B256::new([0u8; 32]);
        let param2 = B256::new([0u8; 32]);
        let param1_data = vec![3u8; 32];

        let vm_event = VmEvent {
            indexed_topics: vec![
                H256::from_slice(&event.selector().0),
                H256::from_slice(&param0.0),
                H256::from_slice(&param2.0),
            ],
            value: param1_data.clone(),
            ..Default::default()
        };

        // Convert the `Event` into its indexed form, matching the number of topics in `VmEvent`.
        let updated_event = get_indexed_event_from_vm_event(event, &vm_event);
        assert_eq!(updated_event.inputs.len(), 3);

        // Now convert the VmEvent into a `LogData`
        let log_data = vm_event_to_log_data(&vm_event);
        let decoded = updated_event.decode_log(&log_data, false).unwrap();

        assert_eq!(
            updated_event
                .inputs
                .iter()
                .filter(|param| param.indexed)
                .count(),
            2
        );
        assert_eq!(
            decoded.indexed[0],
            DynSolValue::Address(Address::from_word(param0))
        );
        assert_eq!(
            decoded.body[0],
            DynSolValue::Uint(U256::from_be_bytes([3u8; 32]), 256)
        );
        assert_eq!(
            decoded.indexed[1],
            DynSolValue::Address(Address::from_word(param2))
        );
    }
}
