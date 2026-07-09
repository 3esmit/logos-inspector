use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value, json};

pub(crate) fn block_list_report(
    sequencer_blocks: &Value,
    indexer_blocks: &Value,
    limit: usize,
) -> Value {
    let sequencer = block_array(sequencer_blocks);
    let indexer = block_array(indexer_blocks);
    let rows = merge_rows(&sequencer, &indexer, limit);

    json!({
        "report_kind": "lez.block_list",
        "schema_version": 1,
        "rows": rows,
        "raw": {
            "sequencer": sequencer,
            "indexer": indexer,
        },
        "provenance": ["typed_report", "raw_provider_payload"],
    })
}

fn merge_rows(sequencer_blocks: &[Value], indexer_blocks: &[Value], limit: usize) -> Vec<Value> {
    let indexed_rows = sorted_rows(
        indexer_blocks
            .iter()
            .map(|block| block_row(block, "indexer"))
            .collect(),
    );
    let indexed_by_id = indexed_rows
        .iter()
        .filter_map(|row| {
            let id = block_id(row);
            (id > 0).then(|| (id, row.clone()))
        })
        .collect::<HashMap<_, _>>();

    let sequencer_rows = sorted_rows(
        sequencer_blocks
            .iter()
            .map(|block| block_row(block, "sequencer"))
            .collect(),
    );

    let mut rows = Vec::with_capacity(sequencer_rows.len() + indexed_rows.len());
    let mut seen = HashSet::new();
    for sequencer_row in sequencer_rows {
        let id = block_id(&sequencer_row);
        let row = indexed_by_id.get(&id).unwrap_or(&sequencer_row);
        append_block(&mut rows, &mut seen, row.clone());
    }

    for indexer_row in indexed_rows {
        append_block(&mut rows, &mut seen, indexer_row);
    }

    let mut rows = sorted_rows(rows);
    let effective_limit = if limit == 0 {
        rows.len().max(1)
    } else {
        limit.max(1)
    };
    rows.truncate(effective_limit);
    rows
}

fn block_array(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}

fn block_row(block: &Value, source: &str) -> Value {
    let mut row = block.as_object().cloned().unwrap_or_else(Map::new);
    row.insert(
        "report_kind".to_owned(),
        Value::String("lez.block_list.row".to_owned()),
    );
    row.insert("block_id".to_owned(), json!(block_id(block)));
    row.insert("header_hash".to_owned(), json!(header_hash(block)));
    row.insert("parent_hash".to_owned(), json!(parent_hash(block)));
    row.insert("timestamp".to_owned(), json!(timestamp(block)));
    row.insert("tx_count".to_owned(), json!(tx_count(block)));
    row.insert("source".to_owned(), Value::String(source.to_owned()));
    row.insert("raw".to_owned(), block.clone());
    row.insert(
        "provenance".to_owned(),
        json!(["typed_report", source.to_owned()]),
    );
    Value::Object(row)
}

fn append_block(rows: &mut Vec<Value>, seen: &mut HashSet<String>, block: Value) {
    let id = block_id(&block);
    let hash = header_hash(&block);
    let key = if id > 0 {
        format!("id:{id}")
    } else if !hash.is_empty() {
        format!("hash:{hash}")
    } else {
        String::new()
    };
    if !key.is_empty() && !seen.insert(key) {
        return;
    }
    rows.push(block);
}

fn sorted_rows(mut rows: Vec<Value>) -> Vec<Value> {
    rows.sort_by_key(|right| std::cmp::Reverse(block_id(right)));
    rows
}

fn block_id(block: &Value) -> u64 {
    value_u64_any(block, &["block_id", "blockId", "id", "slot", "height"])
        .or_else(|| {
            block.get("header").and_then(|header| {
                value_u64_any(header, &["block_id", "blockId", "id", "slot", "height"])
            })
        })
        .unwrap_or(0)
}

fn header_hash(block: &Value) -> String {
    value_string_any(
        block,
        &["header_hash", "headerHash", "hash", "header_id", "headerId"],
    )
    .or_else(|| {
        block.get("header").and_then(|header| {
            value_string_any(
                header,
                &["header_hash", "headerHash", "hash", "header_id", "headerId"],
            )
        })
    })
    .unwrap_or_default()
}

fn parent_hash(block: &Value) -> String {
    value_string_any(
        block,
        &[
            "parent_hash",
            "parentHash",
            "prev_block_hash",
            "prevBlockHash",
            "parent_id",
            "parentId",
        ],
    )
    .or_else(|| {
        block.get("header").and_then(|header| {
            value_string_any(
                header,
                &[
                    "parent_hash",
                    "parentHash",
                    "prev_block_hash",
                    "prevBlockHash",
                    "parent_id",
                    "parentId",
                ],
            )
        })
    })
    .unwrap_or_default()
}

fn timestamp(block: &Value) -> u64 {
    value_u64_any(block, &["timestamp", "time"])
        .or_else(|| {
            block
                .get("header")
                .and_then(|header| value_u64_any(header, &["timestamp", "time"]))
        })
        .unwrap_or(0)
}

fn tx_count(block: &Value) -> usize {
    value_u64_any(block, &["tx_count", "txCount"])
        .and_then(|value| usize::try_from(value).ok())
        .or_else(|| {
            block
                .get("transactions")
                .and_then(Value::as_array)
                .map(Vec::len)
        })
        .or_else(|| {
            block
                .get("body")
                .and_then(|body| body.get("transactions"))
                .and_then(Value::as_array)
                .map(Vec::len)
        })
        .unwrap_or(0)
}

fn value_u64_any(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| value_u64(value.get(*key)?))
}

fn value_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.trim().parse::<u64>().ok())
}

fn value_string_any(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_prefers_indexer_rows_for_matching_block_ids() {
        let sequencer = json!([
            { "block_id": 102, "header_hash": "seq-102", "parent_hash": "p", "timestamp": 10, "transactions": [] }
        ]);
        let indexer = json!([
            { "block_id": 102, "header_hash": "idx-102", "parent_hash": "p", "timestamp": 10, "transactions": [{ "hash": "tx" }] },
            { "block_id": 101, "header_hash": "idx-101", "parent_hash": "p", "timestamp": 9, "tx_count": 0 }
        ]);

        let report = block_list_report(&sequencer, &indexer, 10);
        let rows = report
            .get("rows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        assert_eq!(
            report.get("report_kind").and_then(Value::as_str),
            Some("lez.block_list")
        );
        assert_eq!(rows.len(), 2);
        let Some(first) = rows.first() else {
            return;
        };
        assert_eq!(
            first.get("report_kind").and_then(Value::as_str),
            Some("lez.block_list.row")
        );
        assert_eq!(first.get("block_id").and_then(Value::as_u64), Some(102));
        assert_eq!(
            first.get("header_hash").and_then(Value::as_str),
            Some("idx-102")
        );
        assert_eq!(first.get("source").and_then(Value::as_str), Some("indexer"));
        assert_eq!(first.get("tx_count").and_then(Value::as_u64), Some(1));
        assert_eq!(
            report
                .pointer("/raw/sequencer")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            report
                .pointer("/raw/indexer")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn report_maps_nested_provider_shapes_to_typed_fields() {
        let sequencer = json!([]);
        let indexer = json!([{
            "header": {
                "block_id": "45",
                "hash": "idx-45",
                "prev_block_hash": "idx-44",
                "timestamp": "1001"
            },
            "body": {
                "transactions": [{ "hash": "tx" }]
            }
        }]);

        let report = block_list_report(&sequencer, &indexer, 1);
        let row = report
            .pointer("/rows/0")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        assert_eq!(row.get("block_id").and_then(Value::as_u64), Some(45));
        assert_eq!(
            row.get("header_hash").and_then(Value::as_str),
            Some("idx-45")
        );
        assert_eq!(
            row.get("parent_hash").and_then(Value::as_str),
            Some("idx-44")
        );
        assert_eq!(row.get("timestamp").and_then(Value::as_u64), Some(1001));
        assert_eq!(row.get("tx_count").and_then(Value::as_u64), Some(1));
    }
}
