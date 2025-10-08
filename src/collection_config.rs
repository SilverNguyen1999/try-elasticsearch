use serde_json::{json, Map, Value};

/// Configuration for a specific NFT collection
#[derive(Debug, Clone)]
pub struct CollectionConfig {
    pub address: String,
    pub name: String,
    pub extracted_fields: Vec<ExtractedField>,
}

/// Field to extract from properties for fast queries
#[derive(Debug, Clone)]
pub struct ExtractedField {
    pub name: String,           // Field name in ES document
    pub field_type: FieldType,  // Type for ES mapping
    pub source_key: String,     // Key in raw_metadata.properties
}

#[derive(Debug, Clone)]
pub enum FieldType {
    Integer,
    Keyword,
    Text,
}

/// Get collection-specific configuration
/// Returns None for unknown collections (will use generic mapping)
pub fn get_collection_config(address: &str) -> Option<CollectionConfig> {
    let address_lower = address.to_lowercase();
    
    match address_lower.as_str() {
        // Wildforest Units Collection
        "0xa038c593115f6fcd673f6833e15462b475994879" => Some(CollectionConfig {
            address: address.to_string(),
            name: "Wildforest Units".to_string(),
            extracted_fields: vec![
                ExtractedField {
                    name: "tier".to_string(),
                    field_type: FieldType::Integer,
                    source_key: "tier".to_string(),
                },
                ExtractedField {
                    name: "level".to_string(),
                    field_type: FieldType::Integer,
                    source_key: "level".to_string(),
                },
                ExtractedField {
                    name: "rarity".to_string(),
                    field_type: FieldType::Keyword,
                    source_key: "rarity".to_string(),
                },
                ExtractedField {
                    name: "nft_type".to_string(),
                    field_type: FieldType::Keyword,
                    source_key: "type".to_string(),
                },
            ],
        }),
        
        // Example: Axie Infinity Collection
        "0x32950db2a7164ae833121501c797d79e7b79d74c" => Some(CollectionConfig {
            address: address.to_string(),
            name: "Axie".to_string(),
            extracted_fields: vec![
                ExtractedField {
                    name: "class".to_string(),
                    field_type: FieldType::Keyword,
                    source_key: "class".to_string(),
                },
                ExtractedField {
                    name: "body_part".to_string(),
                    field_type: FieldType::Keyword,
                    source_key: "body".to_string(),
                },
                ExtractedField {
                    name: "breed_count".to_string(),
                    field_type: FieldType::Integer,
                    source_key: "breedCount".to_string(),
                },
            ],
        }),
        
        // Example: Land Collection
        "0x8c666c2fab1a27c49a01d608e23daa99dfa2b489" => Some(CollectionConfig {
            address: address.to_string(),
            name: "Land".to_string(),
            extracted_fields: vec![
                ExtractedField {
                    name: "land_type".to_string(),
                    field_type: FieldType::Keyword,
                    source_key: "land_type".to_string(),
                },
                ExtractedField {
                    name: "x_coordinate".to_string(),
                    field_type: FieldType::Integer,
                    source_key: "col".to_string(),
                },
                ExtractedField {
                    name: "y_coordinate".to_string(),
                    field_type: FieldType::Integer,
                    source_key: "row".to_string(),
                },
            ],
        }),
        
        // Unknown collection - will use generic mapping
        _ => None,
    }
}

/// Generate Elasticsearch mapping for a collection
pub fn generate_collection_mapping(config: Option<&CollectionConfig>) -> Value {
    let mut mapping = base_mapping();
    
    // Add collection-specific extracted fields if config exists
    if let Some(cfg) = config {
        let properties = mapping["mappings"]["properties"]
            .as_object_mut()
            .expect("properties should be an object");
        
        for field in &cfg.extracted_fields {
            let field_mapping = field_type_to_mapping(&field.field_type);
            properties.insert(field.name.clone(), field_mapping);
        }
    }
    
    mapping
}

/// Base mapping that all collections share
fn base_mapping() -> Value {
    json!({
        "settings": {
            "number_of_shards": 1,
            "number_of_replicas": 1,
            "refresh_interval": "5s",
            "analysis": {
                "normalizer": {
                    "lowercase_normalizer": {
                        "type": "custom",
                        "filter": ["lowercase"]
                    }
                },
                "analyzer": {
                    "nft_name_analyzer": {
                        "tokenizer": "standard",
                        "filter": ["lowercase", "asciifolding"]
                    }
                }
            }
        },
        "mappings": {
            "dynamic": false,
            "properties": {
                // Universal infrastructure fields
                "token_address": {"type": "keyword"},
                "token_id": {"type": "keyword"},
                "owner": {"type": "keyword"},
                
                // Universal marketplace fields
                "price": {"type": "double"},
                "ron_price": {"type": "double"},
                "base_price": {"type": "double"},
                "ended_price": {"type": "double"},
                "order_status": {"type": "keyword"},
                "state": {"type": "keyword"},
                "kind": {"type": "long"},
                "maker": {"type": "keyword"},
                "matcher": {"type": "keyword"},
                "payment_token": {"type": "keyword"},
                "order_id": {"type": "long"},
                
                // Timestamps
                "started_at": {"type": "long"},
                "expired_at": {"type": "long"},
                "ended_at": {"type": "long"},
                "metadata_last_updated": {"type": "long"},
                
                // NFT metadata
                "name": {
                    "type": "text",
                    "analyzer": "nft_name_analyzer",
                    "fields": {
                        "keyword": {"type": "keyword"}
                    }
                },
                
                // Collection-specific fields will be added here dynamically
                
                // Flexible fields (same for all collections)
                "properties": {
                    "type": "object",
                    "dynamic": true
                },
                
                "raw_metadata": {
                    "type": "object",
                    "enabled": false
                },
                
                // Media
                "image": {"type": "keyword", "index": false},
                "cdn_image": {"type": "keyword", "index": false},
                "video": {"type": "keyword", "index": false},
                "animation_url": {"type": "keyword", "index": false},
                "description": {"type": "text", "index": false},
                
                // Other
                "is_shown": {"type": "boolean"},
                "ownership_block_number": {"type": "long"},
                "ownership_log_index": {"type": "integer"}
            }
        }
    })
}

/// Convert FieldType to Elasticsearch mapping
fn field_type_to_mapping(field_type: &FieldType) -> Value {
    match field_type {
        FieldType::Integer => json!({"type": "integer"}),
        FieldType::Keyword => json!({
            "type": "keyword",
            "normalizer": "lowercase_normalizer"
        }),
        FieldType::Text => json!({
            "type": "text",
            "analyzer": "nft_name_analyzer"
        }),
    }
}

/// Extract typed value from JSON based on field type
pub fn extract_typed_value(value: &Value, field_type: &FieldType) -> Option<Value> {
    match field_type {
        FieldType::Integer => {
            // Try as number first
            if let Some(n) = value.as_i64() {
                return Some(json!(n));
            }
            // Try parsing string
            if let Some(s) = value.as_str() {
                if let Ok(n) = s.parse::<i64>() {
                    return Some(json!(n));
                }
            }
            None
        }
        FieldType::Keyword => {
            // Normalize to lowercase
            if let Some(s) = value.as_str() {
                Some(json!(s.to_lowercase()))
            } else if let Some(n) = value.as_i64() {
                Some(json!(n.to_string().to_lowercase()))
            } else {
                None
            }
        }
        FieldType::Text => {
            // Keep as-is
            if let Some(s) = value.as_str() {
                Some(json!(s))
            } else if let Some(n) = value.as_i64() {
                Some(json!(n.to_string()))
            } else {
                None
            }
        }
    }
}

/// Extract collection-specific fields from properties
pub fn extract_collection_fields(
    properties: &Map<String, Value>,
    config: &CollectionConfig,
) -> Map<String, Value> {
    let mut extracted = Map::new();
    
    for field in &config.extracted_fields {
        if let Some(value) = properties.get(&field.source_key) {
            if let Some(typed_value) = extract_typed_value(value, &field.field_type) {
                extracted.insert(field.name.clone(), typed_value);
            }
        }
    }
    
    extracted
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_wildforest_config() {
        let config = get_collection_config("0xa038c593115f6fcd673f6833e15462b475994879");
        assert!(config.is_some());
        
        let config = config.unwrap();
        assert_eq!(config.name, "Wildforest Units");
        assert_eq!(config.extracted_fields.len(), 4);
    }

    #[test]
    fn test_get_unknown_collection() {
        let config = get_collection_config("0xunknown");
        assert!(config.is_none());
    }

    #[test]
    fn test_extract_integer_field() {
        let value = json!(5);
        let result = extract_typed_value(&value, &FieldType::Integer);
        assert_eq!(result, Some(json!(5)));
        
        let value = json!("10");
        let result = extract_typed_value(&value, &FieldType::Integer);
        assert_eq!(result, Some(json!(10)));
    }

    #[test]
    fn test_extract_keyword_field() {
        let value = json!("Common");
        let result = extract_typed_value(&value, &FieldType::Keyword);
        assert_eq!(result, Some(json!("common")));
    }

    #[test]
    fn test_generate_mapping_with_config() {
        let config = get_collection_config("0xa038c593115f6fcd673f6833e15462b475994879").unwrap();
        let mapping = generate_collection_mapping(Some(&config));
        
        let properties = &mapping["mappings"]["properties"];
        assert!(properties["tier"].is_object());
        assert!(properties["level"].is_object());
        assert!(properties["rarity"].is_object());
        assert!(properties["nft_type"].is_object());
    }

    #[test]
    fn test_generate_mapping_without_config() {
        let mapping = generate_collection_mapping(None);
        
        let properties = &mapping["mappings"]["properties"];
        // Should have base fields
        assert!(properties["token_address"].is_object());
        assert!(properties["owner"].is_object());
        // Should NOT have collection-specific fields
        assert!(properties["tier"].is_null());
    }
}

