use zksync_core::api_server::web3::backend_jsonrpc::error::into_jsrpc_error;
use zksync_web3_decl::error::Web3Error;

use crate::{
    fork::ForkSource,
    namespaces::{ConfigurationApiNamespaceT, Result},
    node::InMemoryNode,
    observability::LogLevel,
};

use super::{ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> ConfigurationApiNamespaceT
    for InMemoryNode<S>
{
    fn config_get_show_calls(&self) -> Result<String> {
        self.get_inner()
            .read()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .map(|reader| reader.show_calls.to_string())
    }

    fn config_get_current_timestamp(&self) -> Result<u64> {
        self.get_inner()
            .read()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .map(|reader| reader.current_timestamp)
    }

    fn config_set_show_calls(&self, value: String) -> Result<String> {
        let show_calls = match value.parse::<ShowCalls>() {
            Ok(value) => value,
            Err(_) => return self.config_get_show_calls(),
        };

        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .map(|mut writer| {
                writer.show_calls = show_calls;
                writer.show_calls.to_string()
            })
    }

    fn config_set_show_storage_logs(&self, value: String) -> Result<String> {
        let show_storage_logs = match value.parse::<ShowStorageLogs>() {
            Ok(value) => value,
            Err(_) => {
                return self
                    .get_inner()
                    .read()
                    .map_err(|err| {
                        tracing::error!("failed acquiring lock: {:?}", err);
                        into_jsrpc_error(Web3Error::InternalError)
                    })
                    .map(|reader| reader.show_storage_logs.to_string())
            }
        };

        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .map(|mut writer| {
                writer.show_storage_logs = show_storage_logs;
                writer.show_storage_logs.to_string()
            })
    }

    fn config_set_show_vm_details(&self, value: String) -> Result<String> {
        let show_vm_details = match value.parse::<ShowVMDetails>() {
            Ok(value) => value,
            Err(_) => {
                return self
                    .get_inner()
                    .read()
                    .map_err(|err| {
                        tracing::error!("failed acquiring lock: {:?}", err);
                        into_jsrpc_error(Web3Error::InternalError)
                    })
                    .map(|reader| reader.show_vm_details.to_string())
            }
        };

        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .map(|mut writer| {
                writer.show_vm_details = show_vm_details;
                writer.show_vm_details.to_string()
            })
    }

    fn config_set_show_gas_details(&self, value: String) -> Result<String> {
        let show_gas_details = match value.parse::<ShowGasDetails>() {
            Ok(value) => value,
            Err(_) => {
                return self
                    .get_inner()
                    .read()
                    .map_err(|err| {
                        tracing::error!("failed acquiring lock: {:?}", err);
                        into_jsrpc_error(Web3Error::InternalError)
                    })
                    .map(|reader| reader.show_gas_details.to_string())
            }
        };

        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .map(|mut writer| {
                writer.show_gas_details = show_gas_details;
                writer.show_gas_details.to_string()
            })
    }

    fn config_set_resolve_hashes(&self, value: bool) -> Result<bool> {
        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })
            .map(|mut writer| {
                writer.resolve_hashes = value;
                writer.resolve_hashes
            })
    }

    fn config_set_log_level(&self, level: LogLevel) -> Result<bool> {
        if let Some(observability) = &self
            .get_inner()
            .read()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })?
            .observability
        {
            match observability.set_log_level(level.clone()) {
                Ok(_) => tracing::info!("set log level to '{}'", level),
                Err(err) => {
                    tracing::error!("failed setting log level {:?}", err);
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn config_set_logging(&self, directive: String) -> Result<bool> {
        if let Some(observability) = &self
            .get_inner()
            .read()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError)
            })?
            .observability
        {
            match observability.set_logging(&directive) {
                Ok(_) => tracing::info!("set logging to '{}'", directive),
                Err(err) => {
                    tracing::error!("failed setting logging to '{}': {:?}", directive, err);
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}
