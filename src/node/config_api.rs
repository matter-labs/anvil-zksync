use zksync_web3_decl::error::Web3Error;

use crate::{
    config::node::{ShowCalls, ShowGasDetails, ShowStorageLogs, ShowVMDetails},
    fork::ForkSource,
    namespaces::{ConfigurationApiNamespaceT, Result},
    node::InMemoryNode,
    observability::LogLevel,
    utils::into_jsrpc_error,
};

impl<S: ForkSource + std::fmt::Debug + Clone + Send + Sync + 'static> ConfigurationApiNamespaceT
    for InMemoryNode<S>
{
    fn config_get_show_calls(&self) -> Result<String> {
        self.get_inner()
            .read()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire read lock for inner node state.",
                )))
            })
            .map(|reader| reader.config.show_calls.to_string())
    }

    fn config_get_show_outputs(&self) -> Result<bool> {
        self.get_inner()
            .read()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire read lock for inner node state.",
                )))
            })
            .map(|reader| reader.config.show_outputs)
    }

    fn config_get_current_timestamp(&self) -> Result<u64> {
        self.get_inner()
            .read()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire read lock for inner node state.",
                )))
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
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for inner node state.",
                )))
            })
            .map(|mut writer| {
                writer.config.show_calls = show_calls;
                writer.config.show_calls.to_string()
            })
    }

    fn config_set_show_outputs(&self, value: bool) -> Result<bool> {
        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for inner node state.",
                )))
            })
            .map(|mut writer| {
                writer.config.show_outputs = value;
                writer.config.show_outputs
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
                        into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                            "Failed to acquire read lock for inner node state.",
                        )))
                    })
                    .map(|reader| reader.config.show_storage_logs.to_string())
            }
        };

        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for inner node state.",
                )))
            })
            .map(|mut writer| {
                writer.config.show_storage_logs = show_storage_logs;
                writer.config.show_storage_logs.to_string()
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
                        into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                            "Failed to acquire read lock for inner node state.",
                        )))
                    })
                    .map(|reader| reader.config.show_vm_details.to_string())
            }
        };

        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for inner node state.",
                )))
            })
            .map(|mut writer| {
                writer.config.show_vm_details = show_vm_details;
                writer.config.show_vm_details.to_string()
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
                        into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                            "Failed to acquire read lock for inner node state.",
                        )))
                    })
                    .map(|reader| reader.config.show_gas_details.to_string())
            }
        };

        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for inner node state.",
                )))
            })
            .map(|mut writer| {
                writer.config.show_gas_details = show_gas_details;
                writer.config.show_gas_details.to_string()
            })
    }

    fn config_set_resolve_hashes(&self, value: bool) -> Result<bool> {
        self.get_inner()
            .write()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire write lock for inner node state.",
                )))
            })
            .map(|mut writer| {
                writer.config.resolve_hashes = value;
                writer.config.resolve_hashes
            })
    }

    fn config_set_log_level(&self, level: LogLevel) -> Result<bool> {
        if let Some(observability) = &self
            .get_inner()
            .read()
            .map_err(|err| {
                tracing::error!("failed acquiring lock: {:?}", err);
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire read lock for inner node state.",
                )))
            })?
            .observability
        {
            match observability.set_log_level(level) {
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
                into_jsrpc_error(Web3Error::InternalError(anyhow::Error::msg(
                    "Failed to acquire read lock for inner node state.",
                )))
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
