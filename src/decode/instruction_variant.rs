use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use super::idl_type::idl_type_label;

pub(crate) struct InstructionVariantMap<'a> {
    instructions: &'a [Value],
    variant_indices: Vec<u32>,
}

impl<'a> InstructionVariantMap<'a> {
    pub(crate) fn from_idl(idl: &'a Value) -> Result<Self> {
        let instructions = idl
            .get("instructions")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .context("IDL has no instructions array")?;
        let external_type = external_instruction_type(idl)?;
        let variant_indices = if let Some(external_type) = external_type {
            external_variant_indices(instructions, &external_type)?
        } else {
            positional_variant_indices(instructions)?
        };
        Ok(Self {
            instructions,
            variant_indices,
        })
    }

    pub(crate) fn instructions(&self) -> &'a [Value] {
        self.instructions
    }

    pub(crate) fn variant_index(&self, row_index: usize) -> Result<u32> {
        self.variant_indices
            .get(row_index)
            .copied()
            .with_context(|| format!("IDL instruction row {row_index} not found"))
    }

    pub(crate) fn instruction_for_variant(&self, variant_index: u32) -> Result<&'a Value> {
        let row_index = self
            .variant_indices
            .iter()
            .position(|candidate| *candidate == variant_index)
            .with_context(|| format!("IDL instruction variant {variant_index} not found"))?;
        self.instructions
            .get(row_index)
            .with_context(|| format!("IDL instruction row {row_index} not found"))
    }
}

fn external_instruction_type(idl: &Value) -> Result<Option<String>> {
    let Some(instruction_type) = idl.get("instruction_type") else {
        return Ok(None);
    };
    if instruction_type.is_null() {
        return Ok(None);
    }
    if instruction_type
        .as_str()
        .is_some_and(|value| value.trim().is_empty())
    {
        bail!("IDL instruction_type must not be blank");
    }
    Ok(Some(idl_type_label(instruction_type)))
}

fn external_variant_indices(instructions: &[Value], external_type: &str) -> Result<Vec<u32>> {
    let count = u32::try_from(instructions.len()).context("IDL has too many instructions")?;
    let mut owners = BTreeMap::<u32, String>::new();
    let mut indices = Vec::with_capacity(instructions.len());
    for (row_index, instruction) in instructions.iter().enumerate() {
        let name = instruction_name(instruction, row_index);
        let variant_index = declared_variant_index(instruction, &name)?.with_context(|| {
            format!(
                "IDL uses external instruction_type `{external_type}`; instruction `{name}` must declare a u32 variant_index"
            )
        })?;
        if variant_index >= count {
            let upper = count.saturating_sub(1);
            bail!(
                "instruction `{name}` variant_index {variant_index} is outside the required 0..{upper} permutation for external instruction_type `{external_type}`"
            );
        }
        if let Some(previous) = owners.insert(variant_index, name.clone()) {
            bail!(
                "instructions `{previous}` and `{name}` both declare variant_index {variant_index} for external instruction_type `{external_type}`"
            );
        }
        indices.push(variant_index);
    }
    Ok(indices)
}

fn positional_variant_indices(instructions: &[Value]) -> Result<Vec<u32>> {
    instructions
        .iter()
        .enumerate()
        .map(|(row_index, instruction)| {
            let expected = u32::try_from(row_index).context("IDL has too many instructions")?;
            let name = instruction_name(instruction, row_index);
            if let Some(declared) = declared_variant_index(instruction, &name)?
                && declared != expected
            {
                bail!(
                    "instruction `{name}` declares variant_index {declared}, but positional IDL order requires {expected}"
                );
            }
            Ok(expected)
        })
        .collect()
}

fn declared_variant_index(instruction: &Value, name: &str) -> Result<Option<u32>> {
    let Some(value) = instruction.get("variant_index") else {
        return Ok(None);
    };
    let index = value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .with_context(|| format!("instruction `{name}` variant_index must be a u32 integer"))?;
    Ok(Some(index))
}

fn instruction_name(instruction: &Value, row_index: usize) -> String {
    instruction
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("row_{row_index}"))
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, ensure};
    use serde_json::json;

    use super::*;

    #[test]
    fn external_instruction_type_requires_complete_unique_permutation() -> Result<()> {
        let missing = json!({
            "instruction_type": "external::Instruction",
            "instructions": [
                {"name": "first", "variant_index": 0},
                {"name": "second"}
            ]
        });
        let error = InstructionVariantMap::from_idl(&missing)
            .err()
            .map(|error| format!("{error:#}"))
            .unwrap_or_default();
        ensure!(error.contains("instruction `second` must declare a u32 variant_index"));

        let duplicate = json!({
            "instruction_type": "external::Instruction",
            "instructions": [
                {"name": "first", "variant_index": 0},
                {"name": "second", "variant_index": 0}
            ]
        });
        let error = InstructionVariantMap::from_idl(&duplicate)
            .err()
            .map(|error| format!("{error:#}"))
            .unwrap_or_default();
        ensure!(error.contains("both declare variant_index 0"));

        let outside = json!({
            "instruction_type": "external::Instruction",
            "instructions": [
                {"name": "first", "variant_index": 0},
                {"name": "second", "variant_index": 2}
            ]
        });
        let error = InstructionVariantMap::from_idl(&outside)
            .err()
            .map(|error| format!("{error:#}"))
            .unwrap_or_default();
        ensure!(error.contains("outside the required 0..1 permutation"));
        Ok(())
    }

    #[test]
    fn variant_index_must_be_u32_and_positional_metadata_cannot_reorder() -> Result<()> {
        for invalid in [
            json!(null),
            json!("1"),
            json!(-1),
            json!(1.5),
            json!(u64::MAX),
        ] {
            let idl = json!({
                "instruction_type": "external::Instruction",
                "instructions": [{"name": "first", "variant_index": invalid}]
            });
            let error = InstructionVariantMap::from_idl(&idl)
                .err()
                .map(|error| format!("{error:#}"))
                .unwrap_or_default();
            ensure!(error.contains("variant_index must be a u32 integer"));
        }

        let positional = json!({
            "instructions": [
                {"name": "first"},
                {"name": "second", "variant_index": 0}
            ]
        });
        let error = InstructionVariantMap::from_idl(&positional)
            .err()
            .map(|error| format!("{error:#}"))
            .unwrap_or_default();
        ensure!(error.contains("positional IDL order requires 1"));
        Ok(())
    }

    #[test]
    fn external_variant_map_resolves_rows_independently_of_array_order() -> Result<()> {
        let idl = json!({
            "instruction_type": "external::Instruction",
            "instructions": [
                {"name": "third", "variant_index": 2},
                {"name": "first", "variant_index": 0},
                {"name": "second", "variant_index": 1}
            ]
        });
        let variants = InstructionVariantMap::from_idl(&idl)?;
        ensure!(variants.variant_index(0)? == 2);
        ensure!(
            variants
                .instruction_for_variant(1)?
                .get("name")
                .and_then(Value::as_str)
                == Some("second")
        );
        Ok(())
    }
}
