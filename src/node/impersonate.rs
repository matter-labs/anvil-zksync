use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use zksync_types::Address;

/// Manages impersonated accounts across the system.
///
/// Clones always agree on the set of impersonated accounts and updating one affects all other
/// instances.
#[derive(Clone, Debug, Default)]
pub struct ImpersonationManager {
    state: Arc<RwLock<HashSet<Address>>>,
}

impl ImpersonationManager {
    /// Starts impersonation for the provided account.
    ///
    /// Returns `true` if the account was not impersonated before.
    pub fn impersonate(&self, addr: Address) -> bool {
        tracing::trace!(?addr, "start impersonation");
        let mut state = self
            .state
            .write()
            .expect("ImpersonationManager lock is poisoned");
        state.insert(addr)
    }

    /// Stops impersonation for the provided account.
    ///
    /// Returns `true` if the account was impersonated before.
    pub fn stop_impersonating(&self, addr: &Address) -> bool {
        tracing::trace!(?addr, "stop impersonation");
        self.state
            .write()
            .expect("ImpersonationManager lock is poisoned")
            .remove(addr)
    }

    /// Returns whether the provided account is currently impersonated.
    pub fn is_impersonating(&self, addr: &Address) -> bool {
        self.state
            .read()
            .expect("ImpersonationManager lock is poisoned")
            .contains(addr)
    }

    /// Returns all accounts that are currently being impersonated.
    pub fn impersonated_accounts(&self) -> HashSet<Address> {
        self.state
            .read()
            .expect("ImpersonationManager lock is poisoned")
            .clone()
    }

    /// Overrides currently impersonated accounts with the provided value.
    pub fn set_impersonated_accounts(&self, accounts: HashSet<Address>) {
        *self
            .state
            .write()
            .expect("ImpersonationManager lock is poisoned") = accounts;
    }
}
