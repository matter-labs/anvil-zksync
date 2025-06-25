use zksync_multivm::{interface::storage::WriteStorage, vm_latest::Vm, HistoryMode};

use super::boojumos::BoojumOsVM;

#[allow(clippy::large_enum_variant)]
pub enum AnvilVM<W: WriteStorage, H: HistoryMode> {
    BoojumOs(BoojumOsVM<W, H>),
    ZKSync(Vm<W, H>),
}

#[macro_export]
macro_rules! delegate_vm {
    ($variable:expr, $function:ident($($params:tt)*)) => {
        match &mut $variable {
            AnvilVM::BoojumOs(vm) => vm.$function($($params)*),
            AnvilVM::ZKSync(vm) => vm.$function($($params)*),
        }
    };
}
