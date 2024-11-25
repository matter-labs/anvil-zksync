use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use zksync_types::Address;

/// Manages impersonated accounts across the system.
///
/// Clones always agree on the set of impersonated accounts and updating one affects all other
/// instances.
#[derive(Clone, Debug, Default)]
pub struct ImpersonationManager {
    state: Arc<RwLock<ImpersonationState>>,
}

impl ImpersonationManager {
    /// Sets the auto impersonation flag, when `true` it makes all accounts impersonated by default.
    /// Setting to `false` disabled this behavior.
    pub fn set_auto_impersonation(&self, enabled: bool) {
        tracing::trace!(enabled, "auto impersonation status set");
        self.state
            .write()
            .expect("ImpersonationManager lock is poisoned")
            .auto = enabled
    }

    /// Starts impersonation for the provided account.
    ///
    /// Returns `true` if the account was not impersonated before.
    pub fn impersonate(&self, addr: Address) -> bool {
        tracing::trace!(?addr, "start impersonation");
        let mut state = self
            .state
            .write()
            .expect("ImpersonationManager lock is poisoned");
        state.accounts.insert(addr)
    }

    /// Stops impersonation for the provided account.
    ///
    /// Returns `true` if the account was impersonated before.
    pub fn stop_impersonating(&self, addr: &Address) -> bool {
        tracing::trace!(?addr, "stop impersonation");
        self.state
            .write()
            .expect("ImpersonationManager lock is poisoned")
            .accounts
            .remove(addr)
    }

    /// Returns whether the provided account is currently impersonated.
    pub fn is_impersonating(&self, addr: &Address) -> bool {
        let state = self
            .state
            .read()
            .expect("ImpersonationManager lock is poisoned");
        state.auto || state.accounts.contains(addr)
    }

    /// Returns internal state representation.
    pub fn state(&self) -> ImpersonationState {
        self.state
            .read()
            .expect("ImpersonationManager lock is poisoned")
            .clone()
    }

    /// Overrides current internal state with the provided value.
    pub fn set_state(&self, state: ImpersonationState) {
        *self
            .state
            .write()
            .expect("ImpersonationManager lock is poisoned") = state;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImpersonationState {
    /// If `true` then all accounts are impersonated regardless of `accounts` contents
    pub auto: bool,
    /// Accounts that are currently impersonated
    pub accounts: HashSet<Address>,
}
