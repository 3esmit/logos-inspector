use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, Result, bail};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum BackupImportArea {
    Settings,
    Favorites,
    IdlRegistry,
    WalletProfile,
}

impl BackupImportArea {
    pub(crate) const ALL: [Self; 4] = [
        Self::Settings,
        Self::Favorites,
        Self::IdlRegistry,
        Self::WalletProfile,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Favorites => "favorites",
            Self::IdlRegistry => "idl_registry",
            Self::WalletProfile => "wallet_profile",
        }
    }

    const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Settings => &["settings", "app_settings"],
            Self::Favorites => &["favorites"],
            Self::IdlRegistry => &["idl_registry", "idls", "idl"],
            Self::WalletProfile => &["wallet_profile", "wallet"],
        }
    }

    const fn supports_merge(self) -> bool {
        matches!(self, Self::Favorites | Self::IdlRegistry)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackupImportMode {
    Replace,
    Merge,
    Skip,
}

impl BackupImportMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Merge => "merge",
            Self::Skip => "skip",
        }
    }

    pub(crate) const fn is_selected(self) -> bool {
        !matches!(self, Self::Skip)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackupImportSelection {
    settings: BackupImportMode,
    favorites: BackupImportMode,
    idl_registry: BackupImportMode,
    wallet_profile: BackupImportMode,
}

impl Default for BackupImportSelection {
    fn default() -> Self {
        Self {
            settings: BackupImportMode::Skip,
            favorites: BackupImportMode::Skip,
            idl_registry: BackupImportMode::Skip,
            wallet_profile: BackupImportMode::Skip,
        }
    }
}

impl BackupImportSelection {
    pub(crate) const fn mode(&self, area: BackupImportArea) -> BackupImportMode {
        match area {
            BackupImportArea::Settings => self.settings,
            BackupImportArea::Favorites => self.favorites,
            BackupImportArea::IdlRegistry => self.idl_registry,
            BackupImportArea::WalletProfile => self.wallet_profile,
        }
    }

    pub(crate) fn selected_areas(&self) -> Vec<BackupImportArea> {
        BackupImportArea::ALL
            .into_iter()
            .filter(|area| self.mode(*area).is_selected())
            .collect()
    }

    pub(crate) fn affected_areas(&self) -> Vec<BackupImportArea> {
        self.selected_areas()
    }

    pub(crate) fn applied_areas(
        &self,
        settings_applied: bool,
        favorites_applied: bool,
        idl_applied: bool,
        wallet_applied: bool,
    ) -> Vec<BackupImportArea> {
        [
            (BackupImportArea::Settings, settings_applied),
            (BackupImportArea::Favorites, favorites_applied),
            (BackupImportArea::IdlRegistry, idl_applied),
            (BackupImportArea::WalletProfile, wallet_applied),
        ]
        .into_iter()
        .filter_map(|(area, applied)| (applied && self.mode(area).is_selected()).then_some(area))
        .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct BackupImportOptions {
    selection: BackupImportSelection,
    selected_items: BTreeMap<BackupImportArea, BTreeSet<String>>,
    conflict_decisions: BTreeMap<BackupImportArea, BTreeMap<String, ConflictDecision>>,
}

impl BackupImportOptions {
    pub(crate) fn parse(value: Option<&Value>) -> Result<Self> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let object = value
            .as_object()
            .context("backup import options must be a JSON object")?;
        validate_top_level_keys(object)?;
        let selection = BackupImportSelection {
            settings: mode_setting(object, BackupImportArea::Settings)?,
            favorites: mode_setting(object, BackupImportArea::Favorites)?,
            idl_registry: mode_setting(object, BackupImportArea::IdlRegistry)?,
            wallet_profile: mode_setting(object, BackupImportArea::WalletProfile)?,
        };
        let selected_items = aliased_option_map(
            object,
            &["items", "selected_items"],
            "item selection",
            parse_selected_items,
        )?
        .unwrap_or_default();
        let conflict_decisions = aliased_option_map(
            object,
            &["conflicts", "conflict_decisions", "conflictDecisions"],
            "conflict decisions",
            parse_conflict_decisions,
        )?
        .unwrap_or_default();
        validate_detail_modes(&selection, selected_items.keys(), "item selection")?;
        validate_detail_modes(&selection, conflict_decisions.keys(), "conflict decisions")?;
        Ok(Self {
            selection,
            selected_items,
            conflict_decisions,
        })
    }

    pub(crate) const fn selection(&self) -> &BackupImportSelection {
        &self.selection
    }

    pub(crate) const fn mode(&self, area: BackupImportArea) -> BackupImportMode {
        self.selection.mode(area)
    }

    pub(crate) fn selected_areas(&self) -> Vec<BackupImportArea> {
        self.selection.selected_areas()
    }

    pub(crate) fn affected_areas(&self) -> Vec<BackupImportArea> {
        self.selection.affected_areas()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.selected_areas().is_empty()
    }

    pub(super) fn item_is_selected(&self, area: BackupImportArea, key: &str) -> bool {
        self.selected_items
            .get(&area)
            .is_none_or(|keys| keys.contains(key))
    }

    pub(super) fn conflict_decision(
        &self,
        area: BackupImportArea,
        key: &str,
    ) -> Option<ConflictDecision> {
        self.conflict_decisions
            .get(&area)
            .and_then(|decisions| decisions.get(key))
            .copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConflictDecision {
    ReplaceExisting,
    SkipBackupItem,
}

#[derive(Default)]
pub(super) struct ItemAreaReport {
    pub(super) items: Vec<Value>,
    pub(super) conflicts: Vec<Value>,
}

fn validate_top_level_keys(object: &Map<String, Value>) -> Result<()> {
    for key in object.keys() {
        let is_area = BackupImportArea::ALL
            .into_iter()
            .any(|area| area.aliases().contains(&key.as_str()));
        let is_detail = matches!(
            key.as_str(),
            "items" | "selected_items" | "conflicts" | "conflict_decisions" | "conflictDecisions"
        );
        if !is_area && !is_detail {
            bail!("unsupported backup import option `{key}`");
        }
    }
    Ok(())
}

fn mode_setting(object: &Map<String, Value>, area: BackupImportArea) -> Result<BackupImportMode> {
    let mut parsed = None;
    for key in area.aliases() {
        let Some(value) = object.get(*key) else {
            continue;
        };
        let mode = parse_import_mode(value, key)?;
        if parsed.is_some_and(|accepted| accepted != mode) {
            bail!("conflicting backup import aliases for `{}`", area.as_str());
        }
        parsed = Some(mode);
    }
    let mode = parsed.unwrap_or(BackupImportMode::Skip);
    if mode == BackupImportMode::Merge && !area.supports_merge() {
        bail!(
            "backup import mode `merge` is not supported for `{}`",
            area.as_str()
        );
    }
    Ok(mode)
}

fn parse_import_mode(value: &Value, key: &str) -> Result<BackupImportMode> {
    let text = value
        .as_str()
        .with_context(|| format!("backup import mode for `{key}` must be a string"))?;
    match text.trim().to_lowercase().as_str() {
        "" | "replace" => Ok(BackupImportMode::Replace),
        "merge" => Ok(BackupImportMode::Merge),
        "skip" | "none" | "not_import" | "not import" => Ok(BackupImportMode::Skip),
        other => bail!("unsupported backup import mode `{other}` for `{key}`"),
    }
}

fn aliased_option_map<T: PartialEq>(
    object: &Map<String, Value>,
    aliases: &[&str],
    label: &str,
    parse: impl Fn(&Value) -> Result<T>,
) -> Result<Option<T>> {
    let mut accepted = None;
    for alias in aliases {
        let Some(value) = object.get(*alias) else {
            continue;
        };
        let parsed = parse(value)?;
        if accepted.as_ref().is_some_and(|current| current != &parsed) {
            bail!("conflicting backup import aliases for `{label}`");
        }
        accepted = Some(parsed);
    }
    Ok(accepted)
}

fn parse_selected_items(value: &Value) -> Result<BTreeMap<BackupImportArea, BTreeSet<String>>> {
    let object = value
        .as_object()
        .context("backup import item selection must be an object")?;
    parse_area_map(object, "item selection", parse_selected_item_area)
}

fn parse_selected_item_area(value: &Value, area: BackupImportArea) -> Result<BTreeSet<String>> {
    match value {
        Value::Array(values) => {
            let mut keys = BTreeSet::new();
            for value in values {
                let key = item_option_key(value)?;
                if !key.is_empty() {
                    keys.insert(key);
                }
            }
            Ok(keys)
        }
        Value::Object(object) => {
            let mut keys = BTreeSet::new();
            for (key, value) in object {
                if option_enabled(value, key)? {
                    keys.insert(key.to_owned());
                }
            }
            Ok(keys)
        }
        _ => bail!(
            "backup import item selection for `{}` must be an array or object",
            area.as_str()
        ),
    }
}

fn item_option_key(value: &Value) -> Result<String> {
    if let Some(key) = value.as_str() {
        return Ok(key.trim().to_owned());
    }
    if let Some(object) = value.as_object() {
        return Ok(object
            .get("key")
            .and_then(Value::as_str)
            .map(str::trim)
            .context("backup import selected item object requires a string `key`")?
            .to_owned());
    }
    bail!("backup import selected item must be a key string or object")
}

fn option_enabled(value: &Value, key: &str) -> Result<bool> {
    match value {
        Value::Bool(enabled) => Ok(*enabled),
        Value::String(text) => match text.trim().to_lowercase().as_str() {
            "true" | "include" | "selected" | "yes" | "on" => Ok(true),
            "false" | "skip" | "none" | "no" | "off" | "" => Ok(false),
            other => bail!("unsupported backup content value `{other}` for `{key}`"),
        },
        _ => bail!("backup content value for `{key}` must be boolean or string"),
    }
}

fn parse_conflict_decisions(
    value: &Value,
) -> Result<BTreeMap<BackupImportArea, BTreeMap<String, ConflictDecision>>> {
    let object = value
        .as_object()
        .context("backup import conflict decisions must be an object")?;
    parse_area_map(object, "conflict decisions", parse_conflict_area)
}

fn parse_conflict_area(
    value: &Value,
    area: BackupImportArea,
) -> Result<BTreeMap<String, ConflictDecision>> {
    let object = value.as_object().with_context(|| {
        format!(
            "backup import conflict decisions for `{}` must be an object",
            area.as_str()
        )
    })?;
    object
        .iter()
        .map(|(key, value)| Ok((key.clone(), parse_conflict_decision(value)?)))
        .collect()
}

fn parse_conflict_decision(value: &Value) -> Result<ConflictDecision> {
    let text = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("backup import conflict decision must be a string")?;
    match text.to_lowercase().replace('-', "_").as_str() {
        "replace" | "replace_existing" | "use_backup" => Ok(ConflictDecision::ReplaceExisting),
        "skip" | "skip_backup_item" | "keep_existing" => Ok(ConflictDecision::SkipBackupItem),
        other => bail!("unsupported backup import conflict decision `{other}`"),
    }
}

fn parse_area_map<T: PartialEq>(
    object: &Map<String, Value>,
    label: &str,
    parse: impl Fn(&Value, BackupImportArea) -> Result<T>,
) -> Result<BTreeMap<BackupImportArea, T>> {
    for key in object.keys() {
        if !BackupImportArea::ALL
            .into_iter()
            .any(|area| area.aliases().contains(&key.as_str()))
        {
            bail!("unsupported backup import {label} area `{key}`");
        }
    }
    let mut result = BTreeMap::new();
    for area in BackupImportArea::ALL {
        let mut accepted = None;
        for alias in area.aliases() {
            let Some(value) = object.get(*alias) else {
                continue;
            };
            let parsed = parse(value, area)?;
            if accepted.as_ref().is_some_and(|current| current != &parsed) {
                bail!(
                    "conflicting backup import {label} aliases for `{}`",
                    area.as_str()
                );
            }
            accepted = Some(parsed);
        }
        if let Some(value) = accepted {
            result.insert(area, value);
        }
    }
    Ok(result)
}

fn validate_detail_modes<'a>(
    selection: &BackupImportSelection,
    areas: impl Iterator<Item = &'a BackupImportArea>,
    label: &str,
) -> Result<()> {
    for area in areas {
        if !area.supports_merge() || selection.mode(*area) != BackupImportMode::Merge {
            bail!(
                "backup import {label} for `{}` requires merge mode",
                area.as_str()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::json;

    use super::*;

    #[test]
    fn canonical_and_legacy_aliases_normalize_to_one_selection() -> Result<()> {
        let canonical = BackupImportOptions::parse(Some(&json!({
            "settings": "replace",
            "favorites": "merge",
            "idl_registry": "merge",
            "wallet_profile": "replace",
            "items": { "idl_registry": ["idl-a"] },
            "conflicts": { "idl_registry": { "idl-a": "replace_existing" } }
        })))?;
        let legacy = BackupImportOptions::parse(Some(&json!({
            "app_settings": "replace",
            "favorites": "merge",
            "idls": "merge",
            "wallet": "replace",
            "selected_items": { "idl": ["idl-a"] },
            "conflictDecisions": { "idls": { "idl-a": "use_backup" } }
        })))?;

        if canonical != legacy
            || canonical.selected_areas() != legacy.affected_areas()
            || !canonical.item_is_selected(BackupImportArea::IdlRegistry, "idl-a")
            || canonical.conflict_decision(BackupImportArea::IdlRegistry, "idl-a")
                != Some(ConflictDecision::ReplaceExisting)
        {
            bail!("legacy import options did not normalize to canonical options");
        }
        Ok(())
    }

    #[test]
    fn matching_aliases_are_accepted_but_conflicting_aliases_are_rejected() -> Result<()> {
        BackupImportOptions::parse(Some(&json!({
            "idl_registry": "merge",
            "idls": "merge"
        })))?;
        if BackupImportOptions::parse(Some(&json!({
            "idl_registry": "merge",
            "idls": "replace"
        })))
        .is_ok()
        {
            bail!("conflicting area aliases should be rejected");
        }
        if BackupImportOptions::parse(Some(&json!({
            "items": { "idl_registry": ["idl-a"] },
            "selected_items": { "idls": ["idl-b"] }
        })))
        .is_ok()
        {
            bail!("conflicting nested aliases should be rejected");
        }
        Ok(())
    }

    #[test]
    fn import_modes_require_strings_and_replace_only_areas_reject_merge() -> Result<()> {
        if BackupImportOptions::parse(Some(&json!({ "settings": true }))).is_ok() {
            bail!("non-string import mode should be rejected");
        }
        if BackupImportOptions::parse(Some(&json!({ "settings": "merge" }))).is_ok()
            || BackupImportOptions::parse(Some(&json!({ "wallet": "merge" }))).is_ok()
        {
            bail!("replace-only import area accepted merge");
        }
        Ok(())
    }

    #[test]
    fn top_level_option_map_rejects_unknown_keys() -> Result<()> {
        for options in [
            json!({ "idl_regsitry": "merge" }),
            json!({ "favorites": "merge", "favorties": "skip" }),
        ] {
            if BackupImportOptions::parse(Some(&options)).is_ok() {
                bail!("unknown top-level backup import option was accepted: {options}");
            }
        }
        Ok(())
    }

    #[test]
    fn nested_option_maps_reject_unknown_areas() -> Result<()> {
        for options in [
            json!({
                "idl_registry": "merge",
                "items": { "idl_regsitry": ["idl-a"] }
            }),
            json!({
                "idl_registry": "merge",
                "conflicts": { "unknown": { "idl-a": "replace" } }
            }),
        ] {
            if BackupImportOptions::parse(Some(&options)).is_ok() {
                bail!("unknown nested backup import area was accepted: {options}");
            }
        }
        Ok(())
    }

    #[test]
    fn item_and_conflict_details_require_itemized_merge_areas() -> Result<()> {
        for options in [
            json!({
                "settings": "replace",
                "items": { "settings": [] }
            }),
            json!({
                "idl_registry": "replace",
                "items": { "idl_registry": ["idl-a"] }
            }),
            json!({
                "favorites": "skip",
                "conflicts": { "favorites": { "favorite-a": "replace" } }
            }),
            json!({
                "wallet_profile": "replace",
                "conflicts": { "wallet": { "profile": "replace" } }
            }),
        ] {
            if BackupImportOptions::parse(Some(&options)).is_ok() {
                bail!("non-merge backup import details were accepted: {options}");
            }
        }

        BackupImportOptions::parse(Some(&json!({
            "favorites": "merge",
            "items": { "favorites": ["favorite-a"] },
            "conflicts": { "favorites": { "favorite-a": "replace" } }
        })))?;
        BackupImportOptions::parse(Some(&json!({
            "idl_registry": "merge",
            "items": { "idls": ["idl-a"] },
            "conflicts": { "idl": { "idl-a": "keep_existing" } }
        })))?;
        Ok(())
    }
}
