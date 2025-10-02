# Handling Mixed Data Types in NFT Attributes

## The Problem

In the same collection, you might have:
```json
// Token #1
{ "level": 5 }  // number

// Token #2
{ "level": "10" }  // numeric string

// Token #3
{ "level": "max" }  // non-numeric string

// Token #4
{ "level": "5-10" }  // range string
```

## Solutions Comparison

### Option 1: Keep Current Flattened Field (Simplest) ⭐ RECOMMENDED FOR NOW

**Current mapping:**
```json
"attributes": {
  "type": "flattened",
  "depth_limit": 20
}
```

**Pros:**
- ✅ Handles ANY value type automatically
- ✅ No indexing failures
- ✅ Works with inconsistent data
- ✅ No data loss

**Cons:**
- ❌ Everything is string (even numbers)
- ❌ Range queries need both bounds
- ❌ String comparison: "10" < "9" (lexicographic)

**Best for:**
- Collections with unpredictable/inconsistent metadata
- When you need maximum flexibility
- MVP/prototype stage

---

### Option 2: Dual Indexing (Best Performance + Flexibility) ⭐ RECOMMENDED FOR PRODUCTION

**Strategy:** Index BOTH as string AND as number (when possible)

**Mapping:**
```json
{
  "mappings": {
    "properties": {
      "token_address": { "type": "keyword" },
      "token_id": { "type": "keyword" },
      "owner": { "type": "keyword" },
      
      // Dual indexing for level
      "level_numeric": { 
        "type": "integer",
        "ignore_malformed": true  // Skip non-numeric values
      },
      "level_string": { 
        "type": "keyword"  // Keep original as-is
      },
      
      // Same for other numeric-ish attributes
      "tier_numeric": { 
        "type": "integer",
        "ignore_malformed": true
      },
      "tier_string": { 
        "type": "keyword"
      },
      
      // Pure categorical attributes
      "rarity": { "type": "keyword" },
      "nft_type": { "type": "keyword" },
      
      // Keep full attributes for display
      "attributes": {
        "type": "flattened",
        "depth_limit": 20
      }
    }
  }
}
```

**Query for numeric filtering:**
```bash
curl -X POST "http://localhost:9300/INDEX/_search" -H 'Content-Type: application/json' -d'
{
  "query": {
    "bool": {
      "should": [
        {
          "range": {
            "level_numeric": { "gte": 5 }  // Matches numeric levels >= 5
          }
        },
        {
          "term": {
            "level_string": "max"  // Also include special values
          }
        }
      ],
      "minimum_should_match": 1
    }
  }
}
'
```

**Implementation in Rust:**

```rust
#[derive(Debug, Serialize)]
pub struct ElasticsearchDocument {
    pub token_address: Option<String>,
    pub token_id: Option<String>,
    pub owner: Option<String>,
    
    // Dual indexing
    pub level_numeric: Option<i32>,
    pub level_string: Option<String>,
    pub tier_numeric: Option<i32>,
    pub tier_string: Option<String>,
    
    // Pure categorical
    pub rarity: Option<String>,
    pub nft_type: Option<String>,
    
    // Keep full attributes
    pub attributes: Option<Map<String, Value>>,
    
    // ... rest
}

fn extract_level(attributes: &Option<Map<String, Value>>) -> (Option<i32>, Option<String>) {
    let level_value = attributes.as_ref()
        .and_then(|a| a.get("level"));
    
    match level_value {
        Some(Value::Number(n)) => {
            // It's already a number
            let numeric = n.as_i64().map(|v| v as i32);
            let string = Some(n.to_string());
            (numeric, string)
        }
        Some(Value::String(s)) => {
            // Try to parse as number
            let numeric = s.parse::<i32>().ok();
            let string = Some(s.clone());
            (numeric, string)
        }
        Some(Value::Array(arr)) if !arr.is_empty() => {
            // Handle array like ["5"]
            extract_level_from_value(&arr[0])
        }
        _ => (None, None)
    }
}

fn extract_level_from_value(value: &Value) -> (Option<i32>, Option<String>) {
    match value {
        Value::Number(n) => {
            let numeric = n.as_i64().map(|v| v as i32);
            let string = Some(n.to_string());
            (numeric, string)
        }
        Value::String(s) => {
            let numeric = s.parse::<i32>().ok();
            let string = Some(s.clone());
            (numeric, string)
        }
        _ => (None, None)
    }
}

impl From<CsvRecord> for ElasticsearchDocument {
    fn from(record: CsvRecord) -> Self {
        let attributes = parse_attributes(&record.attributes);
        
        // Extract with dual indexing
        let (level_numeric, level_string) = extract_level(&attributes);
        let (tier_numeric, tier_string) = extract_tier(&attributes);
        
        let rarity = attributes.as_ref()
            .and_then(|a| a.get("rarity"))
            .and_then(|v| extract_string_value(v));
            
        let nft_type = attributes.as_ref()
            .and_then(|a| a.get("type"))
            .and_then(|v| extract_string_value(v));
        
        Self {
            token_address: parse_optional_string(&record.token_address),
            token_id: parse_optional_string(&record.token_id),
            owner: parse_optional_string(&record.owner),
            level_numeric,
            level_string,
            tier_numeric,
            tier_string,
            rarity,
            nft_type,
            attributes,
            // ... rest
        }
    }
}
```

**Pros:**
- ✅ Fast numeric range queries
- ✅ Handles non-numeric values gracefully
- ✅ No indexing failures (`ignore_malformed`)
- ✅ Can query both ways
- ✅ Best of both worlds

**Cons:**
- ⚠️ Uses more storage (2x for dual fields)
- ⚠️ Slightly more complex queries
- ⚠️ Need to handle both fields in application

---

### Option 3: Smart Coercion at Query Time

Keep single field but handle in application:

```bash
# Application logic decides which query to use
if is_numeric(user_filter):
    # Use range query with string comparison
    query = {
        "range": {
            "attributes.level": {
                "gte": str(value).zfill(10),  # Pad: "5" -> "0000000005"
                "lte": "9999999999"
            }
        }
    }
else:
    # Use term query for special values
    query = {
        "term": { "attributes.level": "max" }
    }
```

**Pros:**
- ✅ No mapping changes needed
- ✅ Works with current setup

**Cons:**
- ❌ String padding is hacky
- ❌ Doesn't work for all cases
- ❌ Complex application logic

---

### Option 4: Normalize During Ingestion (Most Robust)

**Strategy:** Clean/normalize data BEFORE indexing

```rust
fn normalize_level(value: &Value) -> NormalizedLevel {
    match value {
        Value::Number(n) => NormalizedLevel::Numeric(n.as_i64().unwrap() as i32),
        Value::String(s) => {
            if let Ok(num) = s.parse::<i32>() {
                NormalizedLevel::Numeric(num)
            } else {
                // Map known special values
                match s.to_lowercase().as_str() {
                    "max" | "maximum" => NormalizedLevel::Special("max".to_string()),
                    "min" | "minimum" => NormalizedLevel::Special("min".to_string()),
                    _ => {
                        // Try to extract first number from "5-10"
                        let first_num = s.split(&['-', '/', ' '][..])
                            .next()
                            .and_then(|part| part.parse::<i32>().ok());
                        
                        if let Some(num) = first_num {
                            NormalizedLevel::Numeric(num)
                        } else {
                            NormalizedLevel::Invalid(s.clone())
                        }
                    }
                }
            }
        }
        _ => NormalizedLevel::Missing
    }
}

enum NormalizedLevel {
    Numeric(i32),
    Special(String),
    Invalid(String),
    Missing,
}
```

**Mapping:**
```json
{
  "level": { 
    "type": "integer",
    "ignore_malformed": true
  },
  "level_category": {
    "type": "keyword"  // "max", "min", "invalid", "normal"
  }
}
```

---

## Real-World Example: Your Data

From your CSV:
```csv
"attributes": {"level": ["1"], "rarity": ["common"]}
```

All your levels are arrays of strings! So you need to:

1. Extract from array: `["1"]` → `"1"`
2. Convert to number: `"1"` → `1`
3. Handle edge cases: `["max"]` → special handling

**Current implementation already does #1:**
```rust
// In models.rs line 124
let flattened_value = match value {
    Value::Array(arr) if !arr.is_empty() => arr[0].clone(),
    other => other,
};
```

---

## My Recommendation

### For Your Use Case:

**Phase 1 (Now):** Keep flattened field
- ✅ Works with any data
- ✅ Simple to implement
- ✅ Good for testing/MVP

**Phase 2 (Production):** Dual indexing
- Extract top 3-5 attributes with dual fields
- Better performance for numeric queries
- Handles edge cases gracefully

### Priority Attributes for Dual Indexing:

Based on your GraphQL query:
1. **level** - often numeric range queries
2. **tier** - often numeric
3. **rarity** - categorical (keyword only)
4. **type** - categorical (keyword only)

### Mapping to Use:

```json
{
  "level_numeric": { "type": "integer", "ignore_malformed": true },
  "level_string": { "type": "keyword" },
  "tier_numeric": { "type": "integer", "ignore_malformed": true },
  "tier_string": { "type": "keyword" },
  "rarity": { "type": "keyword" },
  "nft_type": { "type": "keyword" },
  "attributes": { "type": "flattened", "depth_limit": 20 }
}
```

This gives you:
- Fast numeric queries when possible
- Graceful handling of non-numeric values
- Backward compatibility with full attributes
- No indexing failures

---

## Migration Strategy

1. **Analyze your data first:**
```bash
# Check what level values exist
curl -X POST "http://localhost:9300/INDEX/_search" -H 'Content-Type: application/json' -d'
{
  "size": 0,
  "aggs": {
    "level_values": {
      "terms": {
        "field": "attributes.level",
        "size": 100
      }
    }
  }
}
'
```

2. **If 95%+ are numeric:** Dual indexing is worth it
3. **If very mixed:** Keep flattened field
4. **Test both approaches** with your real data volume

Would you like me to create the optimized mapping with dual indexing for you?

