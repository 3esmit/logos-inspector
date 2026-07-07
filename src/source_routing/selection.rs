use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use super::{
    CoreEndpointMode, CoreSourceMode, DEFAULT_DELIVERY_REST_ENDPOINT,
    DEFAULT_STORAGE_REST_ENDPOINT, SourceFamily, effective_source_mode, module,
};

pub(crate) struct Args {
    values: Vec<Value>,
}

pub(crate) struct SourceEndpoint<'a> {
    pub(crate) mode: CoreEndpointMode,
    pub(crate) endpoint: &'a str,
    pub(crate) next_index: usize,
    pub(crate) module: &'static str,
}

pub(crate) struct AccountSources<'a> {
    pub(crate) execution_mode: CoreEndpointMode,
    pub(crate) sequencer_endpoint: &'a str,
    pub(crate) indexer_mode: CoreEndpointMode,
    pub(crate) indexer_endpoint: &'a str,
    pub(crate) account: &'a str,
    pub(crate) next_index: usize,
}

pub(crate) struct RestSource<'a> {
    pub(crate) endpoint: &'a str,
    pub(crate) next_index: usize,
}

pub(crate) struct DeliveryStoreQuery<'a> {
    pub(crate) peer_addr: Option<&'a str>,
    pub(crate) content_topics: Option<&'a str>,
    pub(crate) pubsub_topic: Option<&'a str>,
    pub(crate) cursor: Option<&'a str>,
    pub(crate) page_size: u64,
    pub(crate) ascending: bool,
    pub(crate) include_data: bool,
}

impl Args {
    pub(crate) fn new(value: Value) -> Result<Self> {
        let values = value
            .as_array()
            .context("bridge args must be a JSON array")?
            .clone();
        Ok(Self { values })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Value> {
        self.values.iter()
    }

    pub(crate) fn value(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    pub(crate) fn string(&self, index: usize, label: &str) -> Result<&str> {
        self.value(index)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .with_context(|| format!("{label} is required"))
    }

    pub(crate) fn optional_string(&self, index: usize) -> Option<&str> {
        self.value(index)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub(crate) fn optional_bool(&self, index: usize) -> bool {
        match self.value(index) {
            Some(Value::Bool(value)) => *value,
            Some(Value::String(value)) => matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            _ => false,
        }
    }

    pub(crate) fn u64(&self, index: usize, label: &str) -> Result<u64> {
        let value = self
            .value(index)
            .with_context(|| format!("{label} is required"))?;
        if let Some(value) = value.as_u64() {
            return Ok(value);
        }
        self.string(index, label)?
            .parse::<u64>()
            .with_context(|| format!("invalid {label}"))
    }

    pub(crate) fn json_or_empty_array(&self, index: usize) -> Result<Value> {
        let Some(value) = self.value(index) else {
            return Ok(Value::Array(vec![]));
        };
        match value {
            Value::String(raw) if raw.trim().is_empty() => Ok(Value::Array(vec![])),
            Value::String(raw) => {
                serde_json::from_str(raw).context("failed to parse JSON argument")
            }
            value => Ok(value.clone()),
        }
    }

    pub(crate) fn source_endpoint(&self, index: usize, label: &str) -> Result<SourceEndpoint<'_>> {
        let first = self.string(index, label)?;
        if let Some(mode) = CoreSourceMode::from_token(first) {
            return Ok(SourceEndpoint {
                mode: mode.effective(),
                endpoint: self.string(index + 1, label)?,
                next_index: index + 2,
                module: source_module_for_label(label),
            });
        }
        Ok(SourceEndpoint {
            mode: CoreEndpointMode::Rpc,
            endpoint: first,
            next_index: index + 1,
            module: source_module_for_label(label),
        })
    }

    pub(crate) fn account_sources(&self) -> Result<AccountSources<'_>> {
        let first = self.string(0, "sequencer endpoint")?;
        if let Some(execution_mode) = CoreSourceMode::from_token(first) {
            let indexer_mode = CoreSourceMode::from_token(self.string(2, "indexer source mode")?)
                .context("indexer source mode must be `rpc` or `module`")?;
            return Ok(AccountSources {
                execution_mode: execution_mode.effective(),
                sequencer_endpoint: self.string(1, "sequencer endpoint")?,
                indexer_mode: indexer_mode.effective(),
                indexer_endpoint: self.string(3, "indexer endpoint")?,
                account: self.string(4, "account id")?,
                next_index: 5,
            });
        }
        Ok(AccountSources {
            execution_mode: CoreEndpointMode::Rpc,
            sequencer_endpoint: first,
            indexer_mode: CoreEndpointMode::Rpc,
            indexer_endpoint: self.string(1, "indexer endpoint")?,
            account: self.string(2, "account id")?,
            next_index: 3,
        })
    }
}

pub(crate) fn storage_rest_source(args: &Args) -> Result<RestSource<'_>> {
    rest_source(
        args,
        DEFAULT_STORAGE_REST_ENDPOINT,
        "storage",
        "Storage REST data actions",
    )
}

pub(crate) fn delivery_rest_source(args: &Args) -> Result<RestSource<'_>> {
    rest_source(
        args,
        DEFAULT_DELIVERY_REST_ENDPOINT,
        "delivery",
        "Delivery REST message actions",
    )
}

pub(crate) fn require_mutating_diagnostics(args: &Args, index: usize, label: &str) -> Result<()> {
    if args.optional_bool(index) {
        return Ok(());
    }
    bail!("{label} requires mutating diagnostics to be enabled")
}

fn source_module_for_label(label: &str) -> &'static str {
    if label.contains("indexer") {
        module::INDEXER_MODULE
    } else if label.contains("sequencer") {
        module::LEZ_CORE_MODULE
    } else {
        module::BLOCKCHAIN_MODULE
    }
}

fn rest_source<'a>(
    args: &'a Args,
    default_endpoint: &'static str,
    source_name: &str,
    action_name: &str,
) -> Result<RestSource<'a>> {
    let mode = args.optional_string(0).unwrap_or("rest");
    let normalized = match source_name {
        "storage" => effective_source_mode(SourceFamily::Storage, mode),
        "delivery" => effective_source_mode(SourceFamily::Delivery, mode),
        _ => "unsupported",
    };
    match normalized {
        "rest" => Ok(RestSource {
            endpoint: args.optional_string(1).unwrap_or(default_endpoint),
            next_index: 2,
        }),
        "module" => {
            bail!("{action_name} require {source_name} REST source, not module")
        }
        "metrics" => bail!("{action_name} require {source_name} REST source, not metrics"),
        _ => bail!("{source_name} source mode `{mode}` is not supported"),
    }
}
