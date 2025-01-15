use zksync_multivm::{interface::storage::WriteStorage, vm_latest::Vm, HistoryMode};

use super::zkos::ZKOsVM;

pub enum AnvilVM<W: WriteStorage, H: HistoryMode> {
    ZKOs(ZKOsVM<W, H>),
    ZKSync(Vm<W, H>),
}

#[macro_export]
macro_rules! delegate_vm {
    ($variable:expr, $function:ident($($params:tt)*)) => {
        match &mut $variable {
            AnvilVM::ZKOs(vm) => vm.$function($($params)*),
            AnvilVM::ZKSync(vm) => vm.$function($($params)*),
        }
    };
}
