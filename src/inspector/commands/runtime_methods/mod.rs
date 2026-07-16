use std::fmt;

use anyhow::Result;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::modules::logos_core::{SharedModuleTransport, pin_module_transport};

mod decode;
mod local_nodes;
mod module_reports;
mod network;
mod social;
mod state;
mod storage;
mod wallet;

#[derive(Clone, Copy)]
pub(crate) struct RuntimeMethodEntry {
    name: &'static str,
    handler: RuntimeMethodHandler,
}

impl RuntimeMethodEntry {
    pub(super) const fn sync(name: &'static str, handler: fn(Value) -> Result<Value>) -> Self {
        Self {
            name,
            handler: RuntimeMethodHandler::Sync(handler),
        }
    }

    pub(super) const fn with_runtime(
        name: &'static str,
        handler: fn(&Runtime, Value) -> Result<Value>,
    ) -> Self {
        Self {
            name,
            handler: RuntimeMethodHandler::WithRuntime(handler),
        }
    }

    pub(super) const fn no_args(name: &'static str, handler: fn() -> Result<Value>) -> Self {
        Self {
            name,
            handler: RuntimeMethodHandler::NoArgs(handler),
        }
    }

    pub(super) const fn with_module_transport(
        name: &'static str,
        handler: fn(&Runtime, Value, SharedModuleTransport) -> Result<Value>,
    ) -> Self {
        Self {
            name,
            handler: RuntimeMethodHandler::WithModuleTransport(handler),
        }
    }

    #[cfg(test)]
    pub(crate) fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn execute(
        &self,
        runtime: &Runtime,
        args: Value,
        module_transport: SharedModuleTransport,
    ) -> Result<Value> {
        match self.handler {
            RuntimeMethodHandler::Sync(handler) => handler(args),
            RuntimeMethodHandler::WithRuntime(handler) => handler(runtime, args),
            RuntimeMethodHandler::NoArgs(handler) => handler(),
            RuntimeMethodHandler::WithModuleTransport(handler) => {
                handler(runtime, args, pin_module_transport(module_transport)?)
            }
        }
    }

    pub(crate) const fn allows_host_synchronous_call(&self) -> bool {
        matches!(
            self.handler,
            RuntimeMethodHandler::Sync(_) | RuntimeMethodHandler::NoArgs(_)
        )
    }
}

impl fmt::Debug for RuntimeMethodEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeMethodEntry")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RuntimeMethodEntry {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for RuntimeMethodEntry {}

#[derive(Clone, Copy)]
enum RuntimeMethodHandler {
    Sync(fn(Value) -> Result<Value>),
    WithRuntime(fn(&Runtime, Value) -> Result<Value>),
    NoArgs(fn() -> Result<Value>),
    WithModuleTransport(fn(&Runtime, Value, SharedModuleTransport) -> Result<Value>),
}

const RUNTIME_METHOD_CATALOGS: &[&[RuntimeMethodEntry]] = &[
    decode::METHOD_CATALOG,
    network::METHOD_CATALOG,
    wallet::METHOD_CATALOG,
    local_nodes::METHOD_CATALOG,
    state::METHOD_CATALOG,
    module_reports::METHOD_CATALOG,
    storage::METHOD_CATALOG,
    social::METHOD_CATALOG,
];

pub(crate) fn runtime_method_entries() -> impl Iterator<Item = &'static RuntimeMethodEntry> {
    RUNTIME_METHOD_CATALOGS
        .iter()
        .flat_map(|catalog| catalog.iter())
}

#[cfg(test)]
pub(crate) fn runtime_method_names() -> impl Iterator<Item = &'static str> {
    runtime_method_entries().map(|entry| entry.name)
}

pub(crate) fn lookup(method: &str) -> Option<&'static RuntimeMethodEntry> {
    runtime_method_entries().find(|entry| entry.name == method)
}

pub(crate) fn handle(
    runtime: &Runtime,
    entry: &RuntimeMethodEntry,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    entry.execute(runtime, args, module_transport)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn runtime_catalog_names_are_unique() {
        let mut names = HashSet::new();
        for entry in runtime_method_entries() {
            assert!(
                names.insert(entry.name()),
                "duplicate runtime method `{}`",
                entry.name()
            );
        }
    }

    #[test]
    fn runtime_catalog_lookup_round_trips_entries() {
        for entry in runtime_method_entries() {
            assert_eq!(lookup(entry.name()), Some(entry));
        }
    }

    #[test]
    fn remote_settings_backup_upload_is_owned_by_runtime_operations() {
        assert!(lookup("storageBackupSettings").is_none());
        assert!(lookup("storageUploadBackupCatalogEntry").is_none());
        assert!(
            crate::inspector::commands::operations::operation_bridge_command(
                "storageUploadBackupCatalogEntry"
            )
            .is_some()
        );
    }

    #[test]
    fn remote_settings_backup_download_is_owned_by_runtime_operations() {
        assert!(lookup("storageRestoreSettings").is_none());
        assert!(lookup("storageDownloadBackupCatalogEntry").is_none());
        assert!(
            crate::inspector::commands::operations::operation_bridge_command(
                "storageDownloadBackupCatalogEntry"
            )
            .is_some()
        );
        assert!(
            crate::inspector::commands::operations::operation_bridge_command(
                "storageRestoreSettings"
            )
            .is_some()
        );
    }

    #[test]
    fn storage_payload_upload_is_owned_by_runtime_operations() {
        assert!(lookup("storageUploadPayload").is_none());
        assert!(
            crate::inspector::commands::operations::operation_bridge_command(
                "storageUploadPayload"
            )
            .is_some()
        );
    }

    #[test]
    fn runtime_catalog_defines_host_synchronous_execution_policy() {
        assert!(
            lookup("sourcePolicy").is_some_and(RuntimeMethodEntry::allows_host_synchronous_call)
        );
        assert!(
            lookup("decodeAccount").is_some_and(RuntimeMethodEntry::allows_host_synchronous_call)
        );
        assert!(
            lookup("loadIdlState").is_some_and(RuntimeMethodEntry::allows_host_synchronous_call)
        );
        assert!(lookup("rawRpc").is_some_and(|entry| !entry.allows_host_synchronous_call()));
        assert!(lookup("modules").is_some_and(|entry| !entry.allows_host_synchronous_call()));
        assert!(lookup("storageExists").is_some_and(|entry| !entry.allows_host_synchronous_call()));
    }
}
