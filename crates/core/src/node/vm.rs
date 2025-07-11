use zksync_multivm::{HistoryMode, interface::storage::WriteStorage, vm_latest::Vm};

use super::zksync_os::ZKsyncOsVM;

#[allow(clippy::large_enum_variant)]
pub enum AnvilVM<W: WriteStorage, H: HistoryMode> {
    ZKsyncOs(ZKsyncOsVM<W, H>),
    Era(Vm<W, H>),
}

#[macro_export]
macro_rules! delegate_vm {
    ($variable:expr, $function:ident($($params:tt)*)) => {
        match &mut $variable {
            AnvilVM::ZKsyncOs(vm) => vm.$function($($params)*),
            AnvilVM::Era(vm) => vm.$function($($params)*),
        }
    };
}
