use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct CsvRecord {
    pub token_address: Option<String>,
    pub token_id: Option<String>,
    pub owner: Option<String>,
    pub base_price: Option<String>,
    pub ended_at: Option<String>,
    pub ended_price: Option<String>,
    pub expired_at: Option<String>,
    pub kind: Option<String>,
    pub maker: Option<String>,
    pub matcher: Option<String>,
    pub order_id: Option<String>,
    pub payment_token: Option<String>,
    pub price: Option<String>,
    pub started_at: Option<String>,
    pub state: Option<String>,
    pub name: Option<String>,
    pub attributes: Option<String>,
    pub image: Option<String>,
    pub video: Option<String>,
    pub metadata_last_updated: Option<String>,
    pub cdn_image: Option<String>,
    pub animation_url: Option<String>,
    pub description: Option<String>,
    pub is_shown: Option<String>,
    pub ownership_block_number: Option<String>,
    pub ownership_log_index: Option<String>,
    pub raw_metadata: Option<String>,
    pub order_status: Option<String>,
    pub ron_price: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ElasticsearchDocument {
    pub token_address: Option<String>,
    pub token_id: Option<String>,
    pub owner: Option<String>,
    pub base_price: Option<f64>,
    pub ended_at: Option<i64>,
    pub ended_price: Option<f64>,
    pub expired_at: Option<i64>,
    pub kind: Option<i64>,
    pub maker: Option<String>,
    pub matcher: Option<String>,
    pub order_id: Option<i64>,
    pub payment_token: Option<String>,
    pub price: Option<f64>,
    pub started_at: Option<i64>,
    pub state: Option<String>,
    pub name: Option<String>,
    pub attributes: Option<Map<String, Value>>,
    pub image: Option<String>,
    pub video: Option<String>,
    pub metadata_last_updated: Option<i64>,
    pub cdn_image: Option<String>,
    pub animation_url: Option<String>,
    pub description: Option<String>,
    pub is_shown: Option<bool>,
    pub ownership_block_number: Option<i64>,
    pub ownership_log_index: Option<i32>,
    pub raw_metadata: Option<Value>,
    pub order_status: Option<String>,
    pub ron_price: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct BulkIndexAction {
    pub index: BulkIndexMetadata,
}

#[derive(Debug, Serialize)]
pub struct BulkIndexMetadata {
    #[serde(rename = "_id")]
    pub id: String,
}

fn parse_optional_string(s: &Option<String>) -> Option<String> {
    s.as_ref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
}

fn parse_optional_f64(s: &Option<String>) -> Option<f64> {
    s.as_ref()
        .and_then(|s| s.trim().parse().ok())
}

fn parse_optional_i64(s: &Option<String>) -> Option<i64> {
    s.as_ref()
        .and_then(|s| s.trim().parse().ok())
}

fn parse_optional_i32(s: &Option<String>) -> Option<i32> {
    s.as_ref()
        .and_then(|s| s.trim().parse().ok())
}

fn parse_optional_bool(s: &Option<String>) -> Option<bool> {
    s.as_ref().and_then(|s| match s.trim().to_lowercase().as_str() {
        "t" | "true" => Some(true),
        "f" | "false" => Some(false),
        _ => None,
    })
}

fn parse_attributes(attributes_str: &Option<String>) -> Option<Map<String, Value>> {
    let attr_str = attributes_str.as_ref()?.trim();
    if attr_str.is_empty() {
        return None;
    }

    match serde_json::from_str::<HashMap<String, Value>>(attr_str) {
        Ok(attrs) => {
            let mut flattened = Map::new();
            for (key, value) in attrs {
                // Convert array values to single values for easier querying
                // e.g., {"tier": ["1"]} -> {"tier": "1"}
                let flattened_value = match value {
                    Value::Array(arr) if !arr.is_empty() => arr[0].clone(),
                    other => other,
                };
                flattened.insert(key, flattened_value);
            }
            Some(flattened)
        }
        Err(_) => None,
    }
}

fn parse_raw_metadata(raw_metadata_str: &Option<String>) -> Option<Value> {
    let metadata_str = raw_metadata_str.as_ref()?.trim();
    if metadata_str.is_empty() {
        return None;
    }

    match serde_json::from_str(metadata_str) {
        Ok(metadata) => Some(metadata),
        Err(_) => None,
    }
}

impl From<CsvRecord> for ElasticsearchDocument {
    fn from(record: CsvRecord) -> Self {
        Self {
            token_address: parse_optional_string(&record.token_address),
            token_id: parse_optional_string(&record.token_id),
            owner: parse_optional_string(&record.owner),
            base_price: parse_optional_f64(&record.base_price),
            ended_at: parse_optional_i64(&record.ended_at),
            ended_price: parse_optional_f64(&record.ended_price),
            expired_at: parse_optional_i64(&record.expired_at),
            kind: parse_optional_i64(&record.kind),
            maker: parse_optional_string(&record.maker),
            matcher: parse_optional_string(&record.matcher),
            order_id: parse_optional_i64(&record.order_id),
            payment_token: parse_optional_string(&record.payment_token),
            price: parse_optional_f64(&record.price),
            started_at: parse_optional_i64(&record.started_at),
            state: parse_optional_string(&record.state),
            name: parse_optional_string(&record.name),
            attributes: parse_attributes(&record.attributes),
            image: parse_optional_string(&record.image),
            video: parse_optional_string(&record.video),
            metadata_last_updated: parse_optional_i64(&record.metadata_last_updated),
            cdn_image: parse_optional_string(&record.cdn_image),
            animation_url: parse_optional_string(&record.animation_url),
            description: parse_optional_string(&record.description),
            is_shown: parse_optional_bool(&record.is_shown),
            ownership_block_number: parse_optional_i64(&record.ownership_block_number),
            ownership_log_index: parse_optional_i32(&record.ownership_log_index),
            raw_metadata: parse_raw_metadata(&record.raw_metadata),
            order_status: parse_optional_string(&record.order_status),
            ron_price: parse_optional_f64(&record.ron_price),
        }
    }
}
