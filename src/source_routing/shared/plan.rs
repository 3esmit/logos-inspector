use super::super::SourceProbeKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleProbeStep<'a> {
    pub(crate) method: &'static str,
    pub(crate) args: Vec<&'a str>,
    pub(crate) key: Option<SourceProbeKey>,
}

impl<'a> ModuleProbeStep<'a> {
    pub(crate) fn keyed(method: &'static str, key: SourceProbeKey) -> Self {
        Self {
            method,
            args: Vec::new(),
            key: Some(key),
        }
    }

    pub(crate) fn keyed_with_args(
        method: &'static str,
        args: Vec<&'a str>,
        key: SourceProbeKey,
    ) -> Self {
        Self {
            method,
            args,
            key: Some(key),
        }
    }

    pub(crate) fn unkeyed(method: &'static str, args: Vec<&'a str>) -> Self {
        Self {
            method,
            args,
            key: None,
        }
    }
}
