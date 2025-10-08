use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use crate::collection_config::{CollectionConfig, extract_collection_fields, extract_typed_value};

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

/// Raw metadata structure as received from the indexer service
#[derive(Debug, Deserialize)]
pub struct RawMetadata {
    pub name: Option<String>,
    pub image: Option<String>,
    pub video: Option<String>,
    pub attributes: Option<Value>,
    pub properties: Option<Map<String, Value>>,
    pub description: Option<String>,
    pub external_url: Option<String>,
    pub animation_url: Option<String>,
}

/// Flexible Elasticsearch document that works with ANY collection
/// Uses serde_json::Value for dynamic fields
#[derive(Debug, Serialize)]
pub struct FlexibleElasticsearchDocument {
    // Universal infrastructure fields
    pub token_address: Option<String>,
    pub token_id: Option<String>,
    pub owner: Option<String>,
    
    // Universal marketplace fields
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
    pub ron_price: Option<f64>,
    pub started_at: Option<i64>,
    pub state: Option<String>,
    pub order_status: Option<String>,
    
    // NFT metadata
    pub name: Option<String>,
    pub image: Option<String>,
    pub video: Option<String>,
    pub cdn_image: Option<String>,
    pub animation_url: Option<String>,
    pub description: Option<String>,
    pub metadata_last_updated: Option<i64>,
    
    // Flexible fields (different per collection)
    pub properties: Option<Map<String, Value>>,
    pub raw_metadata: Option<Value>,
    
    // Other
    pub is_shown: Option<bool>,
    pub ownership_block_number: Option<i64>,
    pub ownership_log_index: Option<i32>,
    
    // Collection-specific extracted fields (dynamic)
    #[serde(flatten)]
    pub extracted_fields: Map<String, Value>,
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

// Helper functions
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

/// Parse raw_metadata JSON string into RawMetadata struct
fn parse_raw_metadata_struct(raw_metadata_str: &Option<String>) -> Option<RawMetadata> {
    let metadata_str = raw_metadata_str.as_ref()?.trim();
    if metadata_str.is_empty() {
        return None;
    }

    serde_json::from_str::<RawMetadata>(metadata_str).ok()
}

/// Parse raw_metadata as generic JSON Value for storage
fn parse_raw_metadata_value(raw_metadata_str: &Option<String>) -> Option<Value> {
    let metadata_str = raw_metadata_str.as_ref()?.trim();
    if metadata_str.is_empty() {
        return None;
    }

    serde_json::from_str(metadata_str).ok()
}

impl FlexibleElasticsearchDocument {
    /// Build document from CSV record with optional collection-specific config
    pub fn from_record(record: CsvRecord, config: Option<&CollectionConfig>) -> Self {
        // Parse raw_metadata to extract structured properties
        let raw_metadata_struct = parse_raw_metadata_struct(&record.raw_metadata);
        
        // Get properties from raw_metadata if available
        let properties = raw_metadata_struct
            .as_ref()
            .and_then(|rm| rm.properties.clone());
        
        // Extract collection-specific fields if config is provided
        let extracted_fields = if let (Some(props), Some(cfg)) = (&properties, config) {
            extract_collection_fields(props, cfg)
        } else {
            Map::new()
        };
        
        // Get metadata from raw_metadata or fall back to CSV
        let name = raw_metadata_struct
            .as_ref()
            .and_then(|rm| rm.name.clone())
            .or_else(|| parse_optional_string(&record.name));
        
        let image = raw_metadata_struct
            .as_ref()
            .and_then(|rm| rm.image.clone())
            .or_else(|| parse_optional_string(&record.image));
        
        let video = raw_metadata_struct
            .as_ref()
            .and_then(|rm| rm.video.clone())
            .or_else(|| parse_optional_string(&record.video));
        
        let animation_url = raw_metadata_struct
            .as_ref()
            .and_then(|rm| rm.animation_url.clone())
            .or_else(|| parse_optional_string(&record.animation_url));
        
        let description = raw_metadata_struct
            .as_ref()
            .and_then(|rm| rm.description.clone())
            .or_else(|| parse_optional_string(&record.description));
        
        Self {
            // Infrastructure
            token_address: parse_optional_string(&record.token_address),
            token_id: parse_optional_string(&record.token_id),
            owner: parse_optional_string(&record.owner),
            
            // Marketplace
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
            ron_price: parse_optional_f64(&record.ron_price),
            started_at: parse_optional_i64(&record.started_at),
            state: parse_optional_string(&record.state),
            order_status: parse_optional_string(&record.order_status),
            
            // Metadata
            name,
            image,
            video,
            cdn_image: parse_optional_string(&record.cdn_image),
            animation_url,
            description,
            metadata_last_updated: parse_optional_i64(&record.metadata_last_updated),
            
            // Flexible fields
            properties,
            raw_metadata: parse_raw_metadata_value(&record.raw_metadata),
            
            // Other
            is_shown: parse_optional_bool(&record.is_shown),
            ownership_block_number: parse_optional_i64(&record.ownership_block_number),
            ownership_log_index: parse_optional_i32(&record.ownership_log_index),
            
            // Collection-specific extracted fields (flattened into document root)
            extracted_fields,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection_config::get_collection_config;

    #[test]
    fn test_build_document_without_config() {
        let record = CsvRecord {
            token_address: Some("0xa038c593115f6fcd673f6833e15462b475994879".to_string()),
            token_id: Some("123".to_string()),
            owner: Some("0x123...".to_string()),
            raw_metadata: Some(r#"{"name":"Test","properties":{"tier":1,"level":5}}"#.to_string()),
            ..Default::default()
        };
        
        let doc = FlexibleElasticsearchDocument::from_record(record, None);
        
        assert_eq!(doc.token_address, Some("0xa038c593115f6fcd673f6833e15462b475994879".to_string()));
        assert_eq!(doc.name, Some("Test".to_string()));
        assert!(doc.properties.is_some());
        // No extracted fields without config
        assert!(doc.extracted_fields.is_empty());
    }

    #[test]
    fn test_build_document_with_config() {
        let config = get_collection_config("0xa038c593115f6fcd673f6833e15462b475994879").unwrap();
        
        let record = CsvRecord {
            token_address: Some("0xa038c593115f6fcd673f6833e15462b475994879".to_string()),
            token_id: Some("123".to_string()),
            owner: Some("0x123...".to_string()),
            raw_metadata: Some(r#"{"name":"Test","properties":{"tier":1,"level":5,"rarity":"Common","type":"Archer"}}"#.to_string()),
            ..Default::default()
        };
        
        let doc = FlexibleElasticsearchDocument::from_record(record, Some(&config));
        
        assert_eq!(doc.token_address, Some("0xa038c593115f6fcd673f6833e15462b475994879".to_string()));
        assert_eq!(doc.name, Some("Test".to_string()));
        
        // Should have extracted fields
        assert!(!doc.extracted_fields.is_empty());
        assert_eq!(doc.extracted_fields.get("tier"), Some(&serde_json::json!(1)));
        assert_eq!(doc.extracted_fields.get("level"), Some(&serde_json::json!(5)));
        assert_eq!(doc.extracted_fields.get("rarity"), Some(&serde_json::json!("common")));
        assert_eq!(doc.extracted_fields.get("nft_type"), Some(&serde_json::json!("archer")));
    }
}

// Default implementation for CsvRecord for tests
impl Default for CsvRecord {
    fn default() -> Self {
        Self {
            token_address: None,
            token_id: None,
            owner: None,
            base_price: None,
            ended_at: None,
            ended_price: None,
            expired_at: None,
            kind: None,
            maker: None,
            matcher: None,
            order_id: None,
            payment_token: None,
            price: None,
            started_at: None,
            state: None,
            name: None,
            attributes: None,
            image: None,
            video: None,
            metadata_last_updated: None,
            cdn_image: None,
            animation_url: None,
            description: None,
            is_shown: None,
            ownership_block_number: None,
            ownership_log_index: None,
            raw_metadata: None,
            order_status: None,
            ron_price: None,
        }
    }
}

