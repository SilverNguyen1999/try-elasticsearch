# Elasticsearch Index Optimization Analysis

## Current Mapping Analysis

### ✅ Fields That ARE Indexed (Good Performance)

These fields use `keyword` type and are properly indexed:
- `token_address` - keyword ✅
- `token_id` - keyword ✅
- `owner` - keyword ✅
- `order_status` - keyword ✅
- `payment_token` - keyword ✅
- `maker` - keyword ✅
- `state` - keyword ✅

### ⚠️ Attributes Field - Performance Concerns

**Current:**
```json
"attributes": {
  "type": "flattened",
  "depth_limit": 20
}
```

**Issues:**
1. ✅ **IS indexed** - flattened fields create keyword indexes
2. ❌ **All values treated as strings** - even numbers like "level"
3. ❌ **Range queries require both bounds** (less flexible)
4. ❌ **Less efficient than native types**
5. ❌ **Limited aggregation capabilities**
6. ❌ **No proper numeric sorting**

### ❌ Fields That Are NOT Indexed

These won't benefit from indexes:
- `image` - `"index": false`
- `cdn_image` - `"index": false`
- `video` - `"index": false`
- `animation_url` - `"index": false`
- `description` - `"index": false`
- `raw_metadata` - `"enabled": false`

---

## Performance Test Results

Run these to see current performance:

### 1. Check if queries use index:
```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "profile": true,
  "size": 10,
  "query": {
    "bool": {
      "must": [
        { "term": { "attributes.rarity": "common" } },
        { "term": { "attributes.type": "archer" } }
      ]
    }
  }
}
'
```

Look for `"build_scorer"` section - it should show index usage.

### 2. Compare term query vs range query on flattened:
```bash
# Term query (fast, uses index)
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "profile": true,
  "query": {
    "term": { "attributes.rarity": "common" }
  }
}
'

# Range query on flattened (slower)
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "profile": true,
  "query": {
    "range": {
      "attributes.level": { "gte": "2", "lte": "999" }
    }
  }
}
'
```

---

## Recommended Optimizations

### Option 1: Extract Common Attributes to Top-Level Fields (BEST for Performance)

**Why:** Most queries filter on `rarity`, `type`, `level` - extract these!

**New mapping:**
```json
{
  "mappings": {
    "properties": {
      "token_address": { "type": "keyword" },
      "token_id": { "type": "keyword" },
      "owner": { "type": "keyword" },
      "price": { "type": "double" },
      "ron_price": { "type": "double" },
      "order_status": { "type": "keyword" },
      
      // Extract frequently filtered attributes
      "rarity": { "type": "keyword" },
      "nft_type": { "type": "keyword" },
      "level": { "type": "integer" },
      "tier": { "type": "integer" },
      
      // Keep full attributes for display/other filters
      "attributes": {
        "type": "flattened",
        "depth_limit": 20
      },
      
      "name": {
        "type": "text",
        "analyzer": "nft_name_analyzer",
        "fields": {
          "keyword": { "type": "keyword" }
        }
      },
      
      // ... rest of fields
    }
  }
}
```

**Query becomes:**
```bash
curl -X POST "http://localhost:9300/INDEX/_search" -H 'Content-Type: application/json' -d'
{
  "query": {
    "bool": {
      "must": [
        { "term": { "rarity": "common" } },
        { "term": { "nft_type": "archer" } },
        { "range": { "level": { "gte": 2 } } }  // No upper bound needed!
      ]
    }
  },
  "sort": [{ "price": "asc" }]
}
'
```

**Benefits:**
- ✅ Proper numeric range queries (no upper bound needed)
- ✅ Fast term queries on extracted fields
- ✅ Better aggregations
- ✅ Proper sorting on numeric fields
- ✅ Keep full attributes for flexibility

---

### Option 2: Use Nested Objects with Typed Fields

**For dynamic attributes with known types:**

```json
{
  "attributes": {
    "type": "object",
    "properties": {
      "rarity": { "type": "keyword" },
      "type": { "type": "keyword" },
      "level": { "type": "integer" },
      "tier": { "type": "integer" },
      "perk1": { "type": "keyword" },
      "perk2": { "type": "keyword" },
      "perk3": { "type": "keyword" }
    }
  }
}
```

**Pros:**
- Better than flattened for known fields
- Proper data types

**Cons:**
- Must know all fields upfront
- `"dynamic": "strict"` will reject unknown attributes

---

### Option 3: Keep Flattened BUT Add Workarounds

If you can't change mapping now:

**For numeric range queries, use script:**
```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search" -H 'Content-Type: application/json' -d'
{
  "query": {
    "bool": {
      "must": [
        { "term": { "attributes.rarity": "common" } },
        {
          "script": {
            "script": {
              "source": "doc['\''attributes.level'\''].value != null && Integer.parseInt(doc['\''attributes.level'\''].value) >= params.min",
              "params": { "min": 2 }
            }
          }
        }
      ]
    }
  }
}
'
```

**Warning:** Script queries are SLOW - not recommended for production!

---

## Migration Path

### Step 1: Test Current Performance
```bash
# Profile your most common query
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "profile": true,
  "query": {
    "bool": {
      "must": [
        { "term": { "attributes.rarity": "common" } },
        { "range": { "attributes.level": { "gte": "2", "lte": "999" } } }
      ]
    }
  }
}
'
```

### Step 2: Create New Index with Optimized Mapping
```bash
curl -X PUT "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879_v2" \
  -H 'Content-Type: application/json' \
  -d @optimized-mapping.json
```

### Step 3: Update Your Rust Code

In `models.rs`, extract common attributes:

```rust
#[derive(Debug, Serialize)]
pub struct ElasticsearchDocument {
    pub token_address: Option<String>,
    pub token_id: Option<String>,
    pub owner: Option<String>,
    
    // Extracted for better indexing
    pub rarity: Option<String>,
    pub nft_type: Option<String>,
    pub level: Option<i32>,
    pub tier: Option<i32>,
    
    // Keep full attributes
    pub attributes: Option<Map<String, Value>>,
    
    // ... rest of fields
}

impl From<CsvRecord> for ElasticsearchDocument {
    fn from(record: CsvRecord) -> Self {
        let attributes = parse_attributes(&record.attributes);
        
        // Extract common fields
        let rarity = attributes.as_ref()
            .and_then(|a| a.get("rarity"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
            
        let nft_type = attributes.as_ref()
            .and_then(|a| a.get("type"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
            
        let level = attributes.as_ref()
            .and_then(|a| a.get("level"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i32>().ok());
            
        Self {
            token_address: parse_optional_string(&record.token_address),
            token_id: parse_optional_string(&record.token_id),
            owner: parse_optional_string(&record.owner),
            rarity,
            nft_type,
            level,
            attributes,
            // ... rest
        }
    }
}
```

### Step 4: Reindex Data
```bash
cargo run --release
```

### Step 5: Compare Performance
Run same queries on both indexes and compare `"took"` time.

---

## Recommendations

### For Small Dataset (< 100K docs):
- Current flattened mapping is **acceptable**
- Term queries on `attributes.*` ARE indexed
- Just use both bounds for range queries

### For Medium Dataset (100K - 1M docs):
- **Extract top 3-5 most queried attributes** (rarity, type, level)
- Keep flattened for less common attributes
- Best balance of flexibility and performance

### For Large Dataset (> 1M docs):
- **Definitely extract common attributes**
- Consider removing flattened entirely if you know all possible attributes
- Use proper data types (integer, keyword, etc.)

---

## Quick Check: Is Your Index Being Used?

Run this and check the `"description"` field in the profile output:

```bash
curl -X POST "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "profile": true,
  "size": 0,
  "query": {
    "term": { "attributes.rarity": "common" }
  }
}
'
```

Look for:
- ✅ `"type": "TermQuery"` - means index is used
- ✅ Low `"build_scorer"` time - efficient
- ❌ High `"next_doc"` time - might need optimization

The `"took"` value (in milliseconds) is your baseline for comparison.
