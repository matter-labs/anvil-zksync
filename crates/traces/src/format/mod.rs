pub mod numbers;

use anvil_zksync_common::utils::format::write_interspersed;
use anvil_zksync_types::{numbers::SignedU256, traces::DecodedValue};
use numbers::PrettyNumberExponentialRepr;

pub struct PrettyDecodedValue<'a>(pub &'a DecodedValue);

impl std::fmt::Display for PrettyDecodedValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            DecodedValue::Int(inner) => PrettyNumberExponentialRepr(&inner.clone().into()).fmt(f),
            DecodedValue::Uint(inner) => {
                let su256: SignedU256 = (*inner).into();
                PrettyNumberExponentialRepr(&su256.into()).fmt(f)
            }
            DecodedValue::String(inner) => f.write_fmt(format_args!("{:?}", inner)),
            DecodedValue::CustomStruct {
                name,
                prop_names,
                tuple,
            } => f.write_str(&{
                let mut s = String::new();

                s.push_str(name);

                if prop_names.len() == tuple.len() {
                    s.push_str("({ ");

                    for (i, (prop_name, value)) in std::iter::zip(prop_names, tuple).enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        s.push_str(prop_name);
                        s.push_str(": ");
                        s.push_str(&PrettyDecodedValue(value).to_string());
                    }

                    s.push_str(" })");
                } else {
                    s.push_str(
                        &tuple
                            .iter()
                            .map(|x| PrettyDecodedValue(x).to_string())
                            .collect::<Vec<_>>()
                            .join(","),
                    );
                }
                s
            }),

            DecodedValue::Array(vec) | DecodedValue::FixedArray(vec) => {
                f.write_str("[")?;
                write_interspersed(f, vec.iter().map(PrettyDecodedValue), ", ")?;
                f.write_str("]")
            }
            DecodedValue::Tuple(vec) => {
                f.write_str("(")?;
                write_interspersed(f, vec.iter().map(PrettyDecodedValue), ", ")?;
                f.write_str(")")
            }
            other => other.fmt(f),
        }
    }
}
