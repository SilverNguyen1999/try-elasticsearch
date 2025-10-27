# NFT Marketplace Elasticsearch Strategy

**Date:** October 15, 2025
**Updated:** October 25, 2025
**Goal:** Design Elasticsearch solution for NFT trait filtering with double-write architecture and migration strategy

---

## Architecture Overview

### Double-Write Pattern
- **New writes** go to both PostgreSQL and Elasticsearch simultaneously
- **Existing data** migrated via separate migration job
- **Benefits:**
  - Gradual rollout without downtime
  - Can validate ES queries against PostgreSQL
  - Easy rollback if needed
  - Supports both ERC721 and ERC1155

### Migration Strategy
- **Separate migration job** reads from PostgreSQL, writes to Elasticsearch
- **Supports both ERC721 and ERC1155** token standards
- **Checkpoint-based resumable migration** for large datasets
- **Batch processing** for performance
- **Can be run multiple times** (idempotent - overwrites existing documents)

---

## Implementation Components

### 1. Double-Write Service (In Main Application)
**Location:** `mavis-marketplace-services` (or similar)

**Responsibility:**
- When creating/updating NFT listings, write to both:
  - PostgreSQL (existing)
  - Elasticsearch (new)
- Handle failures gracefully:
  - If ES write fails, log error but don't block PostgreSQL write
  - Implement retry logic with exponential backoff
  - Alert on persistent ES write failures

**Code Pattern:**
```rust
// Pseudo-code
async fn create_listing(nft: NFT) -> Result<()> {
    // Write to PostgreSQL
    db.insert_listing(&nft).await?;

    // Write to Elasticsearch (non-blocking)
    match es.index_document(&nft).await {
        Ok(_) => {},
        Err(e) => {
            error!("Failed to index in ES: {}", e);
            // Log for later retry, don't fail the request
        }
    }

    Ok(())
}
```

### 2. Migration Job (This Repository)
**Location:** `migrate-sample-erc721-data` (current)

**Responsibility:**
- Read existing NFT data from PostgreSQL
- Transform to Elasticsearch document format
- Bulk index into Elasticsearch
- Support resumable migration with checkpoints
- Support both ERC721 and ERC1155

**Note:** Implementation structure to be determined

### 3. Query Service (In Main Application)
**Location:** `mavis-marketplace-services` (or similar)

**Responsibility:**
- Provide query interface that uses Elasticsearch
- Fall back to PostgreSQL if ES unavailable
- Support trait filtering with range queries
- Support full-text search on names

**Query Pattern:**
```rust
// Pseudo-code
async fn search_nfts(filters: SearchFilters) -> Result<Vec<NFT>> {
    match es.search(&filters).await {
        Ok(results) => Ok(results),
        Err(e) => {
            warn!("ES search failed, falling back to PostgreSQL: {}", e);
            db.search_nfts(&filters).await
        }
    }
}
```

---

## Why Elasticsearch?

### Critical Requirement: Fast Range Queries

**Primary Use Case:** Users filter NFTs by numeric trait ranges
```
Examples:
- "Show me all NFTs with level >= 5"
- "Find NFTs with tier between 3 and 7"
- "Filter by power > 100 AND defense < 50"
```

**PostgreSQL Problem:**
```sql
-- Current approach: JSONB with GIN index
SELECT * FROM erc721 
WHERE attributes @> '{"rarity": ["Rare"]}'
  AND (attributes->'level'->>0)::int >= 5  -- Slow! Must cast, can't use index efficiently
```
- Performance: 800-2000ms
- Scaling: Gets worse with collection size
- Can't efficiently use indexes on computed expressions

**Elasticsearch Solution:**
```json
{
  "range": {
    "properties.level": {"gte": 5}  // Fast! Native numeric comparison with index
  }
}
```
- Performance: 10-50ms (20-40x faster)
- Scaling: Consistent performance
- Native numeric indexing

### Why Type Matters for Range Queries

**CRITICAL:** Range queries only work correctly on numeric types!

**Correct (Numeric Type):**
```
Field type: long
Values: 1, 5, 10, 20, 100
Query: level >= 5
Result: 5, 10, 20, 100 ✅
```

**Wrong (String Type):**
```
Field type: keyword
Values: "1", "5", "10", "20", "100"
Query: level >= "5"
Result: "5" < "100" (lexicographic!) ❌
```

**This is why "first type wins" strategy is critical!**

---

## Complete Data Model: Not Just Metadata!

### Critical Understanding: ES Must Store Order Data for Sorting

**The Problem:** Current GraphQL queries (`erc721_tokens`) need to:
1. Filter by NFT attributes/traits (JSONB with GIN index) ← Slow
2. Sort by order fields (price, started_at, etc.) ← Requires denormalized data
3. Filter by order status, owner, auction type ← Requires joins or denormalization

**Current PostgreSQL Approach:**
- **Denormalized design:** Order data stored directly in `erc721` table rows
- **Why:** ERC721 tokens are unique (1 token = 1 owner), so denormalizing order data into the NFT row enables sorting without joins
- **Trade-off:** Must update erc721 row when orders created/matched/cancelled

**Elasticsearch Must Match This Design:**
- Store order data in each NFT document
- Update ES when order events occur
- Enable sorting on price, started_at, ron_price, etc.

---

## ERC721 vs ERC1155 Differences

### ERC721 (Non-Fungible Tokens) - PRIMARY FOCUS

**Characteristics:**
- **One token per token_id** - globally unique
- **One owner per token** - cannot be shared
- **Quantity always 1** - non-fungible
- **Example:** Axie Infinity Units, Lands, Art NFTs

**PostgreSQL Schema (from tracker_database_migration.sql):**
```sql
CREATE TABLE erc721 (
    -- Identity
    token_address varchar NOT NULL,
    token_id numeric(78) NOT NULL,
    
    -- Ownership
    owner varchar NOT NULL,
    is_shown bool DEFAULT true NULL,
    ownership_block_number int8 DEFAULT 0 NULL,
    ownership_log_index int4 DEFAULT 0 NULL,
    received_timestamp int8 NULL,
    
    -- Order data (denormalized for sorting!)
    order_id int8 NULL,
    maker varchar NULL,
    matcher varchar NULL,
    kind int8 NULL,
    base_price numeric(78) NULL,
    ended_price numeric(78) NULL,
    price numeric(78) NULL,
    ron_price numeric(78) NULL,
    started_at int8 NULL,
    ended_at int8 NULL,
    expired_at int8 NULL,
    payment_token varchar NULL,
    order_status varchar NULL,
    state varchar NULL,
    
    -- Metadata
    name varchar NULL,
    attributes jsonb NULL,           -- Processed (string arrays)
    raw_metadata jsonb NULL,         -- Original (proper types)
    image varchar NULL,
    cdn_image varchar NULL,
    video varchar NULL,
    animation_url varchar NULL,
    description varchar NULL,
    metadata_last_updated int8 NULL,
    cached_image_path varchar NULL,
    
    PRIMARY KEY (token_address, token_id)
) PARTITION BY LIST (token_address);
```

**Elasticsearch Document Structure (MUST match Postgres):**
```json
{
  // Identity
  "token_address": "0xa038...",
  "token_id": "409192",
  
  // Ownership (updated on Transfer)
  "owner": "0x123...",
  "is_shown": true,
  "ownership_block_number": 12345678,
  "ownership_log_index": 42,
  "received_timestamp": 1698765432,
  
  // Order data (updated on OrderCreated/Matched/Cancelled)
  "order_id": 98765,
  "maker": "0xabc...",
  "matcher": null,
  "kind": 1,                        // OrderType enum
  "base_price": 1000000000000000000,
  "ended_price": null,
  "price": 1500000000000000000,     // Last sale price
  "ron_price": 100.5,               // Price in RON for sorting
  "started_at": 1698765000,
  "ended_at": null,
  "expired_at": 1698865000,
  "payment_token": "0xc99a...",
  "order_status": "Open",           // "Open", "Matched", "Cancelled", etc.
  "state": "active",
  
  // Metadata
  "name": "Archer #409192",
  "properties": {
    "tier": 0,
    "level": 1,
    "rarity": "common"
  },
  "image": "https://...",
  "cdn_image": "https://...",
  "video": null,
  "animation_url": null,
  "description": "...",
  "metadata_last_updated": 1698700000,
  "raw_metadata": { /* full original */ }
}
```

**Key Design Decision:** Order data denormalized into NFT document for efficient sorting
- Sorting by `price`, `ron_price`, `started_at` without joins
- Filtering by `order_status`, `maker` without joins
- Single document update when order changes

### ERC1155 (Semi-Fungible Tokens) - SECONDARY

**Characteristics:**
- **Multiple tokens per token_id** - quantity-based
- **Multiple owners** - same token_id owned by different addresses
- **Variable quantity** - fungible within token_id
- **Example:** Items, Potions, Consumables, Resources

**PostgreSQL Schema:**
```sql
-- Metadata table (one row per token_id)
CREATE TABLE erc1155_data (
    token_address varchar NOT NULL,
    token_id numeric NOT NULL,
    
    -- Metadata (shared across all owners)
    name varchar NULL,
    attributes jsonb NULL,
    raw_metadata jsonb NULL,
    image varchar NULL,
    cdn_image varchar NULL,
    video varchar NULL,
    animation_url varchar NULL,
    description varchar NULL,
    metadata_last_updated int8 NULL,
    
    -- Aggregated stats
    total_owners int8 DEFAULT 0 NULL,
    total_items numeric DEFAULT 0 NULL,
    total_listing int8 DEFAULT 0 NULL,
    total_items_listing numeric DEFAULT 0 NULL,
    min_price numeric(78) NULL,
    
    PRIMARY KEY (token_address, token_id)
) PARTITION BY LIST (token_address);

-- Ownership tracked separately (not shown, likely in balances table)
```

**Elasticsearch Document Structure (ERC1155):**
```json
{
  "token_address": "0x...",
  "token_id": "456",
  
  // Metadata (shared)
  "name": "Magic Potion",
  "properties": {
    "rarity": "rare",
    "effect": "healing"
  },
  
  // Aggregated data
  "total_owners": 150,
  "total_items": 5000,
  "total_listing": 20,
  "total_items_listing": 500,
  "min_price": 50000000000000000,
  
  // Metadata
  "image": "https://...",
  "raw_metadata": { /* full original */ }
}
```

**Key Differences:**
- ERC1155: Metadata stored once per token_id, ownership tracked separately
- ERC721: All data (metadata + ownership + order) in one document
- ERC1155: Sorting by min_price (across all listings)
- ERC721: Sorting by individual NFT's price

**For This Project: Focus on ERC721**
- More complex (denormalized order data)
- Higher query volume
- More performance-critical
- ERC1155 can be added later (simpler, just metadata)

---

## Events That Trigger ES Updates

### 1. ERC721 Transfer (Owner Changed)

**Event Source:** `indexer/src/event/erc721_transfer.rs`

**What Happens:**
```rust
// Updates in erc721 table:
- owner = new_owner
- is_shown = !EXCLUDED_ADDRESSES.contains(new_owner)  // false if burnt!
- ownership_block_number = event.block_number
- ownership_log_index = event.log_index
- received_timestamp = block_timestamp

// EXCLUDED_ADDRESSES = [ZERO_ADDRESS, DEAD_ADDRESS, ...]
// If NFT transferred to excluded address → it's burnt → is_shown = false
```

**ES Update Required:**
```json
{
  "doc": {
    "owner": "0xnew_owner...",
    "is_shown": true,  // false if burnt (owner in EXCLUDED_ADDRESSES)
    "ownership_block_number": 12345678,
    "ownership_log_index": 42,
    "received_timestamp": 1698765432
  }
}
```

**Important: Burnt NFTs**
- If `new_owner` is in `EXCLUDED_ADDRESSES` (0x0, 0xdead, etc.) → NFT is burnt
- `is_shown = false` → NFT won't appear in marketplace listings
- Still indexed in ES but filtered out by `is_shown` query
- Common in games: burn items, burn land, etc.

**Trigger:** Every NFT transfer (mint, sale, transfer, burn)
**Frequency:** High (thousands per day)
**Critical:** Yes (owner and is_shown must be up-to-date)

### 2. Order Created (Listing/Offer Made)

**Event Source:** `indexer/src/event/order_exchange.rs` → `OrderCreated`

**What Happens:**
```rust
// Order table INSERT
// erc721 table UPDATE (for Sell orders):
- order_id = new_order.id
- maker = new_order.maker
- kind = new_order.kind
- base_price = new_order.base_price
- ended_price = new_order.ended_price
- started_at = new_order.started_at
- ended_at = new_order.ended_at
- expired_at = new_order.expired_at
- payment_token = new_order.payment_token
- order_status = "Open"
- ron_price = converted_price
```

**ES Update Required:**
```json
{
  "doc": {
    "order_id": 98765,
    "maker": "0xmaker...",
    "kind": 1,
    "base_price": 1000000000000000000,
    "ended_price": null,
    "started_at": 1698765000,
    "ended_at": null,
    "expired_at": 1698865000,
    "payment_token": "0xc99a...",
    "order_status": "Open",
    "ron_price": 100.5
  }
}
```

**Trigger:** User creates listing or offer
**Frequency:** Medium (hundreds per day)
**Critical:** Yes (must be searchable/sortable immediately)

### 3. Order Matched (Sale Completed)

**Event Source:** `indexer/src/event/order_exchange.rs` → `OrderMatched`

**What Happens:**
```rust
// erc721 table UPDATE:
- maker = NULL
- order_id = NULL
- kind = NULL
- base_price = NULL
- ended_price = NULL
- started_at = NULL
- ended_at = NULL
- expired_at = NULL
- payment_token = NULL
- order_status = NULL
- ron_price = NULL
- matcher = matcher_address
- price = final_sale_price
```

**ES Update Required:**
```json
{
  "doc": {
    "order_id": null,
    "maker": null,
    "kind": null,
    "base_price": null,
    "ended_price": null,
    "started_at": null,
    "ended_at": null,
    "expired_at": null,
    "payment_token": null,
    "order_status": null,
    "ron_price": null,
    "matcher": "0xmatcher...",
    "price": 1500000000000000000
  }
}
```

**Trigger:** Order is filled (sale completed)
**Frequency:** Medium (hundreds per day)
**Critical:** Yes (must remove from active listings immediately)

### 4. Order Cancelled

**Event Source:** `indexer/src/event/order_exchange.rs` → `OrderCancelledVec`

**What Happens:**
```rust
// erc721 table UPDATE (for Sell orders):
- maker = NULL
- order_id = NULL
- kind = NULL
- base_price = NULL
- ended_price = NULL
- started_at = NULL
- ended_at = NULL
- expired_at = NULL
- payment_token = NULL
- order_status = NULL
- ron_price = NULL
```

**ES Update Required:**
```json
{
  "doc": {
    "order_id": null,
    "maker": null,
    "kind": null,
    "base_price": null,
    "ended_price": null,
    "started_at": null,
    "ended_at": null,
    "expired_at": null,
    "payment_token": null,
    "order_status": null,
    "ron_price": null
  }
}
```

**Trigger:** User cancels listing or offer
**Frequency:** Medium (hundreds per day)
**Critical:** Yes (must remove from active listings immediately)

### 5. Metadata Updated

**Event Source:** `indexer-metadata` service (separate from indexer)

**What Happens:**
```rust
// erc721 table UPDATE:
- name = new_metadata.name
- attributes = processed_attributes
- raw_metadata = original_metadata
- image = new_metadata.image
- cdn_image = cached_image
- metadata_last_updated = timestamp
```

**ES Update Required:**
```json
{
  "doc": {
    "name": "Updated Name",
    "properties": {
      "tier": 1,
      "level": 5,
      "rarity": "epic"
    },
    "image": "https://...",
    "cdn_image": "https://...",
    "metadata_last_updated": 1698800000,
    "raw_metadata": { /* updated */ }
  }
}
```

**Trigger:** NFT metadata refresh (mint, manual refresh, scheduled refresh)
**Frequency:** Low-Medium (metadata updates are less frequent)
**Critical:** Medium (metadata changes are important but not time-critical)

---

## GraphQL Query Performance Problem

### Current Query: `erc721_tokens` (mavis-graphql-token/src/schema.rs)

**Query Pattern:**
```graphql
query GetERC721TokensList(
  $tokenAddress: String
  $owner: String
  $auctionType: AuctionType
  $criteria: [SearchCriteria!]          # Trait filters: [{name: "rarity", values: ["Rare"]}]
  $rangeCriteria: [RangeSearchCriteria!] # Range filters: [{name: "level", min: 5, max: 100}]
  $priceRange: InputRange                # Price range
  $name: String                          # Name search
  $from: Int
  $size: Int
  $sort: SortBy                          # "PriceAsc", "PriceDesc", "RecentlyListed", etc.
) {
  erc721Tokens(...)
}
```

**Example Real Query:**
```graphql
{
  tokenAddress: "0xa038c593115f6fcd673f6833e15462b475994879"
  auctionType: "Sale"
  criteria: [
    {name: "Accessory", values: ["Snot Bubble", "Omamori"]},
    {name: "Tribe", values: ["Bageni"]}
  ]
  rangeCriteria: [
    {name: "level", min: 5}
  ]
  sort: "PriceAsc"
  from: 0
  size: 50
}
```

**Current PostgreSQL Query (Slow!):**
```sql
SELECT *
FROM erc721
WHERE token_address = '0xa038...'
  AND is_shown = true
  AND order_status = 'Open'
  AND attributes @> '{"Accessory": ["Snot Bubble"]}'  -- JSONB containment (slow!)
  OR attributes @> '{"Accessory": ["Omamori"]}'
  AND attributes @> '{"Tribe": ["Bageni"]}'
  AND (attributes->'level'->>0)::int >= 5             -- Type cast (can't use index!)
ORDER BY ron_price ASC NULLS LAST, token_id ASC       -- Sorting
LIMIT 50 OFFSET 0;
```

**Performance Issues:**
1. ❌ JSONB `@>` operator with GIN index is slow for complex conditions
2. ❌ Type casting `(attributes->'level'->>0)::int` can't use index efficiently
3. ❌ Multiple JSONB checks (one per criteria) compounds the slowness
4. ❌ Sorting requires scanning filtered results
5. ❌ Deep pagination (large OFFSET) is very slow

**Performance:** 800-2000ms for complex queries with multiple filters

**Target ES Query (Fast!):**
```json
{
  "query": {
    "bool": {
      "must": [
        {"term": {"token_address": "0xa038..."}},
        {"term": {"is_shown": true}},
        {"term": {"order_status": "Open"}},
        {"terms": {"properties.Accessory": ["Snot Bubble", "Omamori"]}},
        {"terms": {"properties.Tribe": ["Bageni"]}},
        {"range": {"properties.level": {"gte": 5}}}
      ]
    }
  },
  "sort": [
    {"ron_price": {"order": "asc", "missing": "_last"}},
    {"token_id": {"order": "asc"}}
  ],
  "from": 0,
  "size": 50
}
```

**Expected Performance:** 10-50ms (20-40x faster)

**Why ES is Faster:**
1. ✅ Native support for exact-match filters on keyword fields
2. ✅ Native numeric range queries with proper indexes
3. ✅ All filters evaluated in single pass
4. ✅ Efficient sorting using doc values
5. ✅ Better pagination performance

---

## Core Architecture

### Index Strategy
- **One index per collection** (index name = collection address)
- Fixed fields: Same across all collections (token_id, owner, price, etc.)
- Dynamic fields: Different per collection (properties object with traits)
- No cross-collection search needed

### Data Structure in Elasticsearch
```
{
  // Fixed fields (always present)
  "token_address": "0xa038...",
  "token_id": "409192",
  "owner": "0x...",
  "name": "Archer",
  "price": 1000000,
  "order_status": "active",
  
  // Dynamic trait fields (different per collection)
  "properties": {
    "tier": 0,           // number
    "level": 1,          // number
    "rarity": "common",  // string
    "type": "archer"     // string
  },
  
  // Original data (stored, not indexed)
  "raw_metadata": { /* full original */ }
}
```

---

## Type Handling: First Type Wins, Ignore Mismatches

### Simple Rule: First Type Wins

**Key Concept:** The first value of a trait determines its type. All subsequent values must match that type or they are ignored.

**Process:**
1. **First NFT indexed with trait "level": 1**
   - ES sees: JSON number
   - Creates: `properties.level` field with type `long`
   - **Type is now locked for this field**

2. **Second NFT with "level": 5**
   - Type matches (number) → indexed successfully ✅

3. **Third NFT with "level": "max"**
   - Type doesn't match (string vs long) → **value ignored** ❌
   - Document still indexed (other fields work)
   - This NFT's level value is skipped

**That's it.** No complex logic, no type coercion, no special handling. Just:
- First value sets the type
- Matching values: indexed
- Non-matching values: silently ignored

### Dynamic Templates Configuration

**Purpose:** Tell ES how to map new fields when they appear

**Rules needed:**
- If field under `properties.*` is detected as string → map as `keyword`
- If field under `properties.*` is detected as long → map as `long` with `ignore_malformed: true`
- If field under `properties.*` is detected as double → map as `double` with `ignore_malformed: true`
- If field under `properties.*` is detected as boolean → map as `boolean` with `ignore_malformed: true`

**What `ignore_malformed: true` does:**
- Allows document to be indexed even if field value doesn't match expected type
- Silently skips the problematic field value (doesn't index it)
- Other fields in document work normally
- Prevents indexing failures
- **This is the key setting that makes "first type wins" work**

---

## Data Source Strategy

### From PostgreSQL to Elasticsearch

**PostgreSQL has two fields:**
1. **`attributes`** (jsonb): Processed data, string arrays
   ```json
   {"level": ["1"], "rarity": ["common"]}
   ```

2. **`raw_metadata`** (jsonb): Original from indexer, proper types
   ```json
   {
     "properties": {
       "level": 1,
       "rarity": "common"
     },
     "attributes": [
       {"trait_type": "level", "value": 1, "display_type": "number"},
       {"trait_type": "rarity", "value": "common", "display_type": "string"}
     ]
   }
   ```

**Extraction Strategy:**

**Important:** Each NFT uses **either** `properties` **or** `attributes`, not both. Different NFT collections use different metadata formats.

- **If `raw_metadata.attributes` exists:** Use it (array format with explicit display_type)
- **If `raw_metadata.properties` exists:** Use it (object format with inferred types)
- **If neither in raw_metadata:** Fall back to PostgreSQL `attributes` field (string arrays)

**How Indexer-Metadata Processes Raw Metadata:**

The indexer-metadata service (`flatten_raw_attrs` function) handles both formats to support all NFT collections:

1. **Processing `attributes` array (if present):**
   - Parses each attribute object with `trait_type`, `value`, `display_type`, `min_value`, `max_value`
   - Type detection based on JSON value type:
     - `Number` → `TraitValueType::Number`, display_type from attribute or defaults to "number"
     - `String` → `TraitValueType::String`, display_type from attribute or defaults to "string"
     - `Boolean` → `TraitValueType::Boolean`, display_type = "bool"
   - Preserves `min_value` and `max_value` for numeric traits
   - Only includes attributes with valid `VALID_DISPLAY_TYPE`: ["date", "string", "number", "number_ranking", "bool"]
   - Groups duplicate `trait_type` values into arrays (e.g., multiple "Accessory" values)
   - Stores trait value types in `trait_value_type` table

**IMPORTANT: Display Type → ES Type Mapping:**
From `metadata_count.rs`, these `display_type` values are treated as **numeric** in the system:
- `"date"` → ES type: **`double`** (numeric timestamp or date value)
- `"number"` → ES type: **`double`** (numeric value)
- `"number_ranking"` → ES type: **`double`** (numeric ranking/score)
- `"string"` → ES type: `keyword` (text value)
- `"bool"` → ES type: `boolean` (true/false)

**Why `double` for all numeric types:**
- **Massive range:** ±1.7 × 10^308 (more than sufficient for any NFT trait)
- Handles both integers (5) and decimals (12.5) without data loss
- Simpler than choosing between `long` and `double`
- ES stores efficiently (no significant overhead for whole numbers)
- Range queries work identically: `level >= 5` works for both int and float
- Values exceeding `double` range → Ignored (acceptable edge case, extremely unlikely)

**Code reference (lines 26-44 in metadata_count.rs):**
```rust
if attribute.1.display_type.eq("number_ranking")
    || attribute.1.display_type.eq("number")
    || attribute.1.display_type.eq("date")
{
    // These are all treated as numbers - parse as f64
    let value = &attribute.1.value[0].parse::<f64>().unwrap_or_default();
    // Also track min_value and max_value
}
```

**This means for Elasticsearch indexing:**
- When `display_type` is "date", "number", or "number_ranking" → Index as numeric (long/double)
- This enables range queries: `level >= 5`, `timestamp > 1234567890`
- Values stored as strings in PostgreSQL are parsed to numbers during ES indexing

2. **Processing `properties` object (if present):**
   - Parses each key-value pair
   - Type detection based on JSON value type:
     - `Number` → `TraitValueType::Number`, display_type = "number"
     - `String` → `TraitValueType::String`, display_type = "string"
     - `Boolean` → `TraitValueType::Boolean`, display_type = "string" (note: stored as string)
   - No `min_value`/`max_value` support (set to None)
   - Stores trait value types in `trait_value_type` table

**Note:** If both exist in raw_metadata (rare), properties are processed after attributes. However, in practice, each NFT uses one format or the other.

**Conversion Logic for PostgreSQL `attributes` field (fallback):**
- Extract first element from array: `["1"]` → `"1"`
- Attempt type detection:
  - If string contains only digits → parse as integer
  - If string contains decimal → parse as float
  - If parse fails → keep as string
- Preserve whatever type we get

**Key Differences Between Attributes and Properties:**

These are **alternative metadata formats** - each NFT uses one or the other:

| Aspect | `raw_metadata.attributes` | `raw_metadata.properties` |
|--------|---------------------------|---------------------------|
| **Format** | Array of objects | Flat object |
| **Structure** | `[{"trait_type": "X", "value": Y, "display_type": "Z"}]` | `{"X": Y}` |
| **Type Info** | Explicit `display_type` field | Inferred from JSON value type |
| **Min/Max Values** | Supported (`min_value`, `max_value`) | Not supported |
| **Display Types** | Can be: "number", "number_ranking", "date", "string", "bool" | Always: "number", "string", or "string" (for bool) |
| **Duplicate Keys** | Supported (multiple values for same `trait_type`) | Not supported (last value wins) |
| **Validation** | Only valid `display_type` values included | All values included |
| **Use Case** | Standard ERC721 metadata format | Alternative format used by some collections |

**Example 1: NFT with `attributes` format:**
```json
{
  "name": "Warrior #123",
  "attributes": [
    {"trait_type": "level", "value": 5, "display_type": "number", "min_value": 1, "max_value": 100},
    {"trait_type": "rarity", "value": "Rare", "display_type": "string"},
    {"trait_type": "accessory", "value": "Sword", "display_type": "string"},
    {"trait_type": "accessory", "value": "Shield", "display_type": "string"}
  ]
}
```

**Example 2: NFT with `properties` format:**
```json
{
  "name": "Warrior #456",
  "properties": {
    "level": 5,
    "rarity": "Rare",
    "tier": 3,
    "generation": "Gen1",
    "is_legendary": false
  }
}
```

**After Processing by Indexer-Metadata (from attributes format):**
```json
{
  "level": {
    "display_type": "number",
    "value": ["5"],
    "min_value": 1,
    "max_value": 100
  },
  "rarity": {
    "display_type": "string",
    "value": ["Rare"],
    "min_value": null,
    "max_value": null
  },
  "accessory": {
    "display_type": "string",
    "value": ["Sword", "Shield"],  // Multiple values grouped from duplicate trait_type
    "min_value": null,
    "max_value": null
  }
}
```

**After Processing by Indexer-Metadata (from properties format):**
```json
{
  "level": {
    "display_type": "number",
    "value": ["5"],
    "min_value": null,
    "max_value": null
  },
  "rarity": {
    "display_type": "string",
    "value": ["Rare"],
    "min_value": null,
    "max_value": null
  },
  "tier": {
    "display_type": "number",
    "value": ["3"],
    "min_value": null,
    "max_value": null
  },
  "generation": {
    "display_type": "string",
    "value": ["Gen1"],
    "min_value": null,
    "max_value": null
  },
  "is_legendary": {
    "display_type": "string",  // Note: Boolean stored as string
    "value": ["false"],
    "min_value": null,
    "max_value": null
  }
}
```

**Implications for Elasticsearch Indexing:**

1. **Use `raw_metadata.attributes` when available** - Better type information with `display_type`
2. **Use `raw_metadata.properties` as supplement** - Adds additional traits not in attributes
3. **Preserve `min_value` and `max_value`** - Important for numeric range validation
4. **Handle duplicate trait_type** - Create ES arrays for multiple values
5. **Respect display_type** - Use it to determine ES field type mapping
6. **Properties override attributes** - If same key exists in both, properties wins

---

## Edge Cases & Solutions

### Case 1: First NFT Has Wrong Type (CRITICAL for Range Queries!)

**Problem:**
```
NFT #1: {level: "1"}     → ES creates field as keyword (string)
NFT #2: {level: 5}       → Type mismatch, value ignored
NFT #3: {level: 10}      → Type mismatch, value ignored

Query: level >= 5
Result: WRONG! Only NFT #1 has level indexed (as string "1")
Correct result should be: NFT #2, #3 (and any others with numeric level)

This BREAKS range queries completely! ❌
```

**Why This Is Critical:**
- Range queries are the primary reason for using Elasticsearch
- If numeric traits indexed as strings → **range queries don't work**
- Must prevent this at all costs

**Real-World Impact:**
```
User searches: "Show me level 5+ NFTs"
- If correct type: Gets 1000 NFTs (level 5-100)
- If wrong type: Gets 0 NFTs (all numeric values ignored)
- User sees no results, feature appears broken
```

**Solution: Accept First Type, Ignore Mismatches**

This is expected behavior with mixed-type data:
1. Index the data as-is (no pre-processing needed)
2. First NFT's type wins (in this case, string)
3. Range queries won't work for this trait (expected behavior)

**Why this happens:**
- Collection has inconsistent data (some numeric, some string)
- First NFT indexed happens to have string value
- ES locks in string type
- All numeric values are ignored (type mismatch)

**What happens:**
- Mismatched types are silently ignored (handled by `ignore_malformed: true`)
- Document still indexed (other fields work)
- That trait's value just isn't searchable for this NFT
- No errors, no alerts, no special handling needed

### Case 2: Type Conflict Within Collection

**Problem:**
```
NFT #100: {level: 1}      → Number (indexed) ✅
NFT #500: {level: "max"}  → String (type mismatch, ignored) ❌

Query: level >= 5
Result: NFT #500 not returned (its level value was ignored)
```

**What happens:**
- Type mismatch is silently ignored (handled by `ignore_malformed: true`)
- NFT #500 is still indexed (other traits work)
- Still searchable by owner, token_id, other traits
- Just missing the conflicting trait in search results
- Full data still in `raw_metadata` for API response
- User can still find the NFT, just not via that trait's range query

**This is expected behavior** - no errors, no alerts, no special handling needed

### Case 3: NFT with >60 Traits

**Problem:**
- Some NFTs might have 100+ traits
- ES has field limit per index

**Solution:**
- Set `index.mapping.total_fields.limit` to 100 (~30 fixed + 60 traits + buffer)
- When NFT has >60 traits:
  - Index first 60 traits (by order they appear or by priority)
  - Remaining traits not indexed
  - Log warning with collection address and token_id
  - Full data still in `raw_metadata`

**Indexing priority:**
1. Range-queryable traits first (level, tier, power, etc.)
2. Common filter traits (rarity, type, etc.)
3. Remaining traits in order

**Query impact:**
- First 60 traits: Searchable ✅
- Traits 61+: Not searchable ❌
- API still returns full data from `raw_metadata`

**Is 60 trait limit enough?**
- 95%+ of NFTs have <20 traits
- 99%+ have <60 traits
- 60 should be sufficient for most collections
- If specific collection needs more, can increase limit per index

### Case 4: New Traits Appear Over Time

**Problem:**
```
Month 1: {level, rarity, type}
Month 6: {level, rarity, type, generation}  ← New trait!
```

**Solution:**
- Dynamic mapping handles this automatically
- When first NFT with "generation" trait indexed:
  - ES creates new field `properties.generation`
  - Type detected from first occurrence
  - No reindex needed ✅

**Edge case within this:**
- What if NFT #1000 is first with new trait "generation": 1 (number)
- Later NFT #5000 has "generation": "alpha" (string)
- Same "first type wins" logic applies
- "alpha" value would be skipped

### Case 5: Empty or Null Values

**Problem:**
```
NFT #1: {level: 1}
NFT #2: {level: null}
NFT #3: {level: ""}
NFT #4: {} // no level field
```

**Handling:**
- `null` → Not indexed (field doesn't exist for this doc)
- `""` (empty string) → Indexed as empty string
- Missing field → Not indexed (field doesn't exist)

**Query behavior:**
```
Query: level >= 5
- NFT #1: Matched if level >= 5
- NFT #2, #3, #4: Not matched (field missing/empty)

Query: exists(level)
- NFT #1: Matched
- NFT #2: Not matched (null)
- NFT #3: Matched (empty string exists)
- NFT #4: Not matched (field missing)
```

### Case 6: Same Trait Name, Different Meaning

**Problem:**
- Collection A: "level" = combat level (1-100, number)
- Collection B: "level" = building level (Floor 1, Floor 2, string)

**Solution:**
- Not a problem! Each collection has its own index
- Collection A index: `properties.level` is `long`
- Collection B index: `properties.level` is `keyword`
- No conflict because they're separate indexes

### Case 7: Multiple Values for Same Trait

**Problem:**
Raw metadata has array of attribute objects with duplicate `trait_type`:
```json
{
  "attributes": [
    {"trait_type": "Tribe", "value": "Humba"},
    {"trait_type": "Accessory", "value": "Ikari Maaku"},
    {"trait_type": "Accessory", "value": "Snot Bubble"}  // Duplicate!
  ]
}
```

**Solution: Store as ES Array (Preserves All Values)**

**Conversion Logic:**
1. Group by `trait_type`
2. If single value → store as primitive
3. If multiple values → store as array

```json
{
  "properties": {
    "Tribe": "Humba",                               // Single value
    "Accessory": ["Ikari Maaku", "Snot Bubble"]    // Multiple values as array
  }
}
```

**How ES Handles Arrays:**
- Field type determined by element type (all elements must be same type)
- ES internally indexes each value separately
- All values are searchable
- Queries match if ANY value matches

**Query Behavior:**
```json
// Find NFTs with "Snot Bubble" accessory
{"term": {"properties.Accessory": "Snot Bubble"}}
→ Matches! ✅ (found in array)

// Find NFTs with "Ikari Maaku" accessory
{"term": {"properties.Accessory": "Ikari Maaku"}}
→ Matches! ✅ (found in array)

// Find NFTs with ANY of these accessories (OR logic)
{"terms": {"properties.Accessory": ["Snot Bubble", "Omamori"]}}
→ Matches! ✅ (Snot Bubble found)

// Combined with other filters (AND logic)
{
  "bool": {
    "must": [
      {"terms": {"properties.Accessory": ["Snot Bubble", "Omamori"]}},
      {"terms": {"properties.Tribe": ["Bageni"]}}
    ]
  }
}
→ Matches if Accessory contains any of the values AND Tribe matches
```

**GraphQL to ES Translation:**
```
GraphQL criteria:
[
  {"name": "Accessory", "values": ["Snot Bubble", "Omamori"]},  // OR
  {"name": "Tribe", "values": ["Bageni"]}                        // AND
]

ES query:
{
  "bool": {
    "must": [
      {"terms": {"properties.Accessory": ["Snot Bubble", "Omamori"]}},
      {"terms": {"properties.Tribe": ["Bageni"]}}
    ]
  }
}
```

**Implementation Notes:**
- During conversion from `attributes` array, check for duplicate `trait_type`
- Collect all values for same `trait_type`
- Create ES array if >1 value found
- This is the ONLY way arrays are created (from duplicate trait_type)

**Important: Arrays in Raw Metadata Are NOT Supported**
- If `raw_metadata.properties` already has an array value → Skip that trait
- We only create arrays by grouping duplicate `trait_type` from attributes array
- Pre-existing arrays in source data are ignored

```json
// ❌ Skip if already array in raw_metadata
{
  "properties": {
    "perks": ["Fire", "Ice"]  // Skip this, not indexed
  }
}

// ✅ Convert from attributes array
{
  "attributes": [
    {"trait_type": "Accessory", "value": "A"},
    {"trait_type": "Accessory", "value": "B"}
  ]
}
→ Creates: {"properties": {"Accessory": ["A", "B"]}}
```

**Type Consistency:**
- All values for same `trait_type` must be same type
- Example: All strings `["Fire", "Ice"]` → keyword type
- Example: All numbers `[10, 20]` → long type
- Mixed types: `[10, "max"]` → Skip this trait entirely

### Case 8: Nested Objects in Traits (NOT SUPPORTED)

**Problem:**
```
{
  "stats": {
    "attack": 50,
    "defense": 30
  }
}
```

**Decision: Skip nested traits entirely**
- Only accept flat trait structure (one level)
- If trait value is an object → Skip it, don't index
- Only index primitive values: number, string, boolean, array of primitives

**Behavior:**
```
{
    "properties": {
    "level": 5,           // ✅ Indexed (number)
    "rarity": "Rare",     // ✅ Indexed (string)
    "stats": {            // ❌ Skipped (nested object)
      "attack": 50
    },
    "perks": ["Fire"]     // ✅ Indexed (array of strings)
  }
}

Result in ES:
{
  "properties": {
    "level": 5,
    "rarity": "Rare",
    "perks": ["Fire"]
    // stats not indexed
  }
}
```

### Case 9: Attributes vs Properties Conflict (RARE EDGE CASE)

**Problem:**
In rare cases, an NFT might have both `attributes` and `properties` with conflicting data:
```json
{
  "attributes": [
    {"trait_type": "level", "value": 5, "display_type": "number"}
  ],
  "properties": {
    "level": "max"  // Different type!
  }
}
```

**Solution: Properties Override Attributes**
- Indexer-metadata processes attributes first, then properties
- If same key exists in both, properties value replaces attributes value
- This allows properties to override/correct attribute data

**Processing Order:**
1. Parse all attributes → `level: 5 (number)`
2. Parse all properties → `level: "max" (string)` ← Overwrites!
3. Final result: `level: "max" (string)`

**Implications for ES:**
- If attributes had numeric level first, ES field type is `long`
- Properties sends string "max" → Fails type check
- With `ignore_malformed: true` → Value skipped, document still indexed
- Result: NFT indexed but level value missing

**Note:** This is a rare edge case. In practice, each NFT uses either attributes OR properties, not both. This handling is defensive programming to catch all possible metadata formats.

### Case 10: Display Type Validation in Attributes

**Problem:**
```json
{
  "attributes": [
    {"trait_type": "level", "value": 5, "display_type": "custom_type"}
  ]
}
```

**Solution: Only Valid Display Types Indexed**
- Valid types: `["date", "string", "number", "number_ranking", "bool"]`
- Invalid types: Attribute skipped entirely
- Trait value type still recorded in `trait_value_type` table (for analytics)
- But attribute not included in parsed results

**Example:**
```json
{
  "attributes": [
    {"trait_type": "level", "value": 5, "display_type": "number"},        // ✅ Indexed
    {"trait_type": "rarity", "value": "Rare", "display_type": "custom"},  // ❌ Skipped
    {"trait_type": "power", "value": 100, "display_type": "number_ranking"} // ✅ Indexed
  ]
}
```

**Result:**
- `level` and `power` indexed
- `rarity` skipped (invalid display_type)
- All three recorded in trait_value_type table
```

**Rationale:**
- Simplifies implementation
- No complex flattening logic needed
- Predictable field structure
- Nested traits are rare anyway

**Implementation:**
- During indexing, check each trait value type
- If value is object (not array) → skip it
- Log skipped traits for monitoring
- Full original data still in `raw_metadata`

**Query impact:**
- Nested traits not searchable
- If collection needs to search nested data → creator must flatten at source

### Case 9: Boolean as String

**Problem:**
```
NFT #1: {is_special: true}        → Boolean
NFT #2: {is_special: "true"}      → String
NFT #3: {is_special: 1}           → Number
```

**Handling:**
- First NFT wins
- If NFT #1 indexed first → field is `boolean`
- NFT #2: "true" string coerced to boolean (ES smart about this)
- NFT #3: 1 → might fail conversion (depends on ES version)

**Best practice:**
- Be consistent with boolean representation
- Prefer actual JSON boolean over strings

---

## Elasticsearch Mapping Structure

### Settings
```
{
  "index.mapping.total_fields.limit": 100,  // ~30 fixed fields + 60 trait fields + buffer
  "number_of_shards": 3,
  "number_of_replicas": 1,
  "refresh_interval": "5s"
}
```

### Mappings
```
{
  "dynamic": true,  // Allow new fields
  
  "dynamic_templates": [
    // Rule for string traits
    {
      "properties_strings": {
        "path_match": "properties.*",
        "match_mapping_type": "string",
        "mapping": {
          "type": "keyword"
        }
      }
    },
    
    // Rule for integer traits
    {
      "properties_longs": {
        "path_match": "properties.*",
        "match_mapping_type": "long",
        "mapping": {
          "type": "long",
          "ignore_malformed": true  // KEY SETTING
        }
      }
    },
    
    // Rule for decimal traits
    {
      "properties_doubles": {
        "path_match": "properties.*",
        "match_mapping_type": "double",
        "mapping": {
          "type": "double",
          "ignore_malformed": true
        }
      }
    },
    
    // Rule for boolean traits
    {
      "properties_booleans": {
        "path_match": "properties.*",
        "match_mapping_type": "boolean",
        "mapping": {
          "type": "boolean",
          "ignore_malformed": true
        }
      }
    }
  ],
  
    "properties": {
    // Fixed fields (explicit mapping)
      "token_address": {"type": "keyword"},
      "token_id": {"type": "keyword"},
      "owner": {"type": "keyword"},
      "name": {
        "type": "text",
      "fields": {"keyword": {"type": "keyword"}}
    },
    "price": {"type": "double"},
    "order_status": {"type": "keyword"},
    // ... all other fixed fields
    
    // Dynamic properties
    "properties": {
      "type": "object",
      "dynamic": true  // Allow new trait fields
    },
    
    // Archival
    "raw_metadata": {
      "type": "object",
      "enabled": false  // Not indexed
    }
  }
}
```

---

## Query Behavior & Edge Cases

### Query Type: Exact Match (Term)
```
Query: rarity = "Rare"
ES: {"term": {"properties.rarity": "Rare"}}
```
**Works for:** String traits (keyword type)
**Edge cases:**
- Case sensitive by default
- If creator has "Rare" and "rare" → different values
- Solution: Normalize to lowercase during indexing

### Query Type: Range (Numeric) - PRIMARY USE CASE

**This is why we need Elasticsearch!**

**Single-sided ranges:**
```
Query: level >= 5
ES: {"range": {"properties.level": {"gte": 5}}}

Query: tier < 10
ES: {"range": {"properties.level": {"lt": 10}}}
```

**Bounded ranges:**
```
Query: level >= 5 AND level <= 10
ES: {"range": {"properties.level": {"gte": 5, "lte": 10}}}
```

**Multiple range filters:**
```
Query: level >= 5 AND tier >= 3 AND power > 100
ES: {
  "bool": {
    "must": [
      {"range": {"properties.level": {"gte": 5}}},
      {"range": {"properties.tier": {"gte": 3}}},
      {"range": {"properties.power": {"gt": 100}}}
    ]
  }
}
```

**Range + Exact filters combined:**
```
Query: rarity = "Rare" AND level >= 5
ES: {
  "bool": {
    "must": [
      {"term": {"properties.rarity": "Rare"}},
      {"range": {"properties.level": {"gte": 5}}}
    ]
    }
}
```

**Works for:** Numeric traits (long/double type)

**Requirements:**
- ✅ Field MUST be indexed as numeric type (long or double)
- ✅ Values MUST be actual JSON numbers, not strings

**Edge cases:**
- ❌ If field is keyword → string comparison (WRONG results!)
  ```
  String comparison: "5" > "40" → true (lexicographic)
  Numeric comparison: 5 > 40 → false (correct)
  ```
- ⚠️ NFTs with non-numeric values excluded from results (acceptable)
- ⚠️ NFTs where field doesn't exist excluded from results (expected)

**Performance:**
- PostgreSQL: 800-2000ms (with JSONB cast and extraction)
- Elasticsearch: 10-50ms (native numeric index)
- **Improvement: 20-40x faster**

### Query Type: Multiple Values (Terms)
```
Query: rarity IN ["Rare", "Epic", "Legendary"]
ES: {"terms": {"properties.rarity": ["Rare", "Epic", "Legendary"]}}
```
**Works for:** String traits
**Edge cases:**
- Case sensitive
- Must match exactly

### Query Type: Exists
```
Query: Must have "level" trait
ES: {"exists": {"field": "properties.level"}}
```
**Matches:** Any NFT where field exists (even if empty string)
**Doesn't match:** NFTs where field is null or missing

### Query Type: Combined Filters
```
Query: rarity = "Rare" AND level >= 5 AND type = "Warrior"
ES: {
  "bool": {
    "must": [
      {"term": {"properties.rarity": "Rare"}},
      {"range": {"properties.level": {"gte": 5}}},
      {"term": {"properties.type": "Warrior"}}
    ]
  }
}
```
**Edge cases:**
- If any field doesn't exist on NFT → NFT excluded
- If any field has wrong type → might be excluded

---

## Logging & Monitoring for Edge Cases

### What to Log During Indexing

**Type Conflicts:**
```
WARN: Type conflict in collection 0xa038...
  - Field: properties.level
  - Expected: long
  - Got: "max" (string)
  - NFT: token_id=12345
  - Action: Field value skipped, document indexed
```

**Field Limit Reached:**
```
WARN: Field limit approaching in collection 0xa038...
  - Current fields: 92 / 100
  - NFT with excess traits: token_id=67890
  - Traits indexed: 60
  - Traits skipped: 12
```

**Nested Object Skipped:**
```
INFO: Nested trait skipped in collection 0xa038...
  - Field: properties.stats
  - Reason: Nested object not supported (only flat traits)
  - NFT: token_id=11111
  - Available in raw_metadata
```

### Monitoring Metrics

**Per Collection:**
- Document count
- Field count (how many traits total)
- Type conflict rate (% of NFTs with conflicts)
- Average traits per NFT
- Max traits in any single NFT

**System-wide:**
- Total collections (indexes)
- Total documents
- Failed indexing attempts
- Query latency by query type

**Alerts:**
- Type conflict rate >10% in any collection
- Field count >90 in any collection (approaching 100 limit)
- Index creation failures
- Query latency >100ms

---

## Migration Job Configuration

### Environment Variables
```bash
# Elasticsearch
ELASTICSEARCH_URL=http://localhost:9200
ELASTICSEARCH_INDEX=nft-listings

# PostgreSQL (for migration job)
DATABASE_URL=postgresql://user:password@localhost/marketplace_db

# Migration settings
BATCH_SIZE=1000              # Documents per batch
WORKERS=4                    # Parallel workers
TIMEOUT_SECS=30              # Request timeout
TOKEN_TYPE=erc721            # erc721 or erc1155
COLLECTION_ADDRESS=0x...     # Optional: migrate specific collection only
```

### Database Schema for Migration
The migration job reads from PostgreSQL tables:

**ERC721 Table:**
```sql
CREATE TABLE erc721_tokens (
    id BIGSERIAL PRIMARY KEY,
    token_address VARCHAR(255) NOT NULL,
    token_id VARCHAR(255) NOT NULL,
    owner VARCHAR(255) NOT NULL,
    name VARCHAR(255),
    image VARCHAR(255),
    price NUMERIC,
    order_status VARCHAR(50),
    attributes JSONB,           -- Processed attributes (string arrays)
    raw_metadata JSONB,         -- Original metadata with properties/attributes
    metadata_last_updated BIGINT,
    created_at TIMESTAMP,
    updated_at TIMESTAMP,
    UNIQUE(token_address, token_id)
);

CREATE INDEX idx_erc721_token_address ON erc721_tokens(token_address);
CREATE INDEX idx_erc721_owner ON erc721_tokens(owner);
```

**ERC1155 Table:**
```sql
CREATE TABLE erc1155_tokens (
    id BIGSERIAL PRIMARY KEY,
    token_address VARCHAR(255) NOT NULL,
    token_id VARCHAR(255) NOT NULL,
    owner VARCHAR(255) NOT NULL,
    quantity BIGINT NOT NULL,
    name VARCHAR(255),
    image VARCHAR(255),
    price NUMERIC,
    order_status VARCHAR(50),
    attributes JSONB,           -- Processed attributes (string arrays)
    raw_metadata JSONB,         -- Original metadata with properties/attributes
    metadata_last_updated BIGINT,
    created_at TIMESTAMP,
    updated_at TIMESTAMP,
    UNIQUE(token_address, token_id, owner)
);

CREATE INDEX idx_erc1155_token_address ON erc1155_tokens(token_address);
CREATE INDEX idx_erc1155_owner ON erc1155_tokens(owner);
```

### Checkpoint File Format
Migration progress saved to `.checkpoint.{csv_file_hash}.json`:

```json
{
  "csv_file": "sample.csv",
  "total_records": 10000,
  "processed_records": 5000,
  "completed_batches": [
    {"start_index": 0, "batch_size": 1000},
    {"start_index": 1000, "batch_size": 1000},
    {"start_index": 2000, "batch_size": 1000},
    {"start_index": 3000, "batch_size": 1000},
    {"start_index": 4000, "batch_size": 1000}
  ],
  "failed_batches": 0,
  "last_updated": "2025-10-25T10:30:00Z"
}
```

---

## Final Strategy Summary

### Core Decisions

**1. Data Source Priority**
- **Each NFT uses either `attributes` OR `properties`, not both** (different collection formats)
- **If `raw_metadata.attributes` exists:** Use it (array format with explicit display_type and min/max)
- **Else if `raw_metadata.properties` exists:** Use it (object format with inferred types)
- **Else:** Fall back to PostgreSQL `attributes` field (string arrays, requires type inference)
- **Defensive handling:** If both exist (rare), process attributes first, then properties (properties override)

**2. Type Handling: First Type Wins, Ignore Mismatches**
- First NFT's value determines the field type (locked in)
- Subsequent NFTs: matching type → indexed ✅, non-matching type → ignored ❌
- Example: If "level" first indexed as number, all string "level" values are ignored
- Document still indexed (other fields work), just that field value skipped
- **For attributes:** Use explicit `display_type` to determine ES field type
- **For properties:** Infer type from JSON value type
- **Key:** Use `ignore_malformed: true` in mapping to silently skip mismatches

**3. Multiple Values for Same Trait: ES Arrays**
- Convert from attributes array format: `[{"trait_type": "Accessory", "value": "A"}, {"trait_type": "Accessory", "value": "B"}]`
- Group duplicate trait_type → Create ES array: `{"Accessory": ["A", "B"]}`
- All values preserved and searchable
- Queries use `terms` for OR logic, `bool.must` for AND between different traits
- Pre-existing arrays in raw_metadata → ignored
- Properties don't support duplicate keys (last value wins)

**4. Field Limits**
- Total ES fields: **100 maximum** (`index.mapping.total_fields.limit: 100`)
- Fixed fields: ~30 (token_address, owner, price, order fields, etc.)
- Trait fields: **60 maximum**
- Buffer: ~10 for safety
- NFTs with >60 traits → First 60 indexed (prioritize range-queryable traits)

**5. Dynamic Mapping Configuration**
- `dynamic: true` → Auto-create new trait fields
- Dynamic templates for `properties.*` path:
  - String values → `keyword` type (exact match)
  - Long values → `long` type with `ignore_malformed: true`
  - Double values → `double` type with `ignore_malformed: true`
  - Boolean values → `boolean` type with `ignore_malformed: true`
- Arrays supported (created from duplicate trait_type only)
- Nested objects → skipped entirely

**6. Attributes-Specific Handling**
- Only include attributes with valid `display_type`: ["date", "string", "number", "number_ranking", "bool"]
- Invalid display_type → Attribute skipped (but type recorded in trait_value_type table)
- Preserve `min_value` and `max_value` for numeric traits
- Use explicit display_type instead of inferring from JSON type

**7. Properties-Specific Handling**
- Infer type from JSON value type (no explicit display_type)
- No min/max value support
- Boolean values stored as string display_type
- Override attributes if same key exists

**8. Data Not Supported (Will Be Skipped)**
- ❌ Nested objects in traits
- ❌ Pre-existing arrays in raw_metadata.properties
- ❌ Mixed types in same trait (non-numeric values in numeric fields)
- ❌ Traits beyond 60 limit
- ❌ Attributes with invalid display_type

**9. Type Determination: First NFT Wins**
- Whatever type the first NFT has for a field → that's the field type
- If first NFT has numeric level → field is numeric (range queries work) ✅
- If first NFT has string level → field is string (range queries don't work) ❌
- No pre-validation needed, no sorting required
- Just index the data as-is, first value determines type
- Accept that some collections may have type mismatches (handled by `ignore_malformed`)

---

## Summary of Strategy

### Core Principles
1. **Range queries are the primary goal** (why we need ES)
2. **One index per collection** (isolated schemas)
3. **First type wins** (ES dynamic mapping behavior - no pre-validation needed)
4. **Ignore mismatches** (graceful degradation for type conflicts with `ignore_malformed: true`)

### How It Works
1. **Index data as-is** - no pre-processing or sorting required
2. **First NFT's value type** determines the field type (locked in)
3. **Subsequent NFTs:**
   - Matching type → indexed ✅
   - Non-matching type → ignored ❌ (document still indexed)
4. **Result:** Simple, predictable behavior

### Acceptable Trade-offs
- NFTs with non-numeric values in numeric fields: Values skipped but document indexed
- NFTs with >60 traits: Extra traits not indexed (rare edge case)
- Nested object traits: Skipped entirely, not indexed
- Type conflicts: Silently ignored (handled by `ignore_malformed: true`)
- Empty/null values: Excluded from queries (expected behavior)

---

---

## Rollout Strategy

### Phase 1: Setup & Validation (Week 1)
1. Deploy Elasticsearch cluster
2. Create indexes with proper mappings
3. Run migration job on staging environment
4. Validate data integrity:
   - Compare document counts
   - Spot-check trait values
   - Verify range queries work correctly
5. Performance testing:
   - Benchmark query latency
   - Compare PostgreSQL vs Elasticsearch

### Phase 2: Double-Write Deployment (Week 2)
1. Deploy double-write code to production
2. Monitor ES write failures
3. Verify data consistency between PostgreSQL and ES
4. Keep PostgreSQL as primary source of truth
5. Run migration job on production (off-peak hours)

### Phase 3: Query Migration (Week 3)
1. Deploy query service with ES support
2. Implement fallback to PostgreSQL
3. Monitor query latency and error rates
4. Gradually increase ES query traffic
5. Keep PostgreSQL queries as fallback

### Phase 4: Cleanup (Week 4+)
1. Once ES is stable and queries are fast:
   - Remove PostgreSQL fallback
   - Deprecate PostgreSQL trait queries
   - Keep PostgreSQL for archival/backup
2. Monitor for any issues
3. Document lessons learned

---

## Monitoring & Alerting

### Metrics to Track

**Elasticsearch Health:**
- Cluster health status (green/yellow/red)
- Index size and document count
- Query latency (p50, p95, p99)
- Indexing latency
- Failed bulk requests

**Data Consistency:**
- Document count: PostgreSQL vs Elasticsearch
- Trait value mismatches
- Type conflict rate (% of documents with type mismatches)
- Field limit exceeded rate

**Migration Job:**
- Documents processed per second
- Batch success/failure rate
- Checkpoint save frequency
- Total migration time

### Alerts to Set Up

**Critical:**
- ES cluster health is red
- Bulk indexing failure rate >5%
- Query latency >500ms
- Document count mismatch >1%

**Warning:**
- ES cluster health is yellow
- Bulk indexing failure rate >1%
- Query latency >200ms
- Type conflict rate >5%
- Field limit exceeded in any collection

---

## Troubleshooting Guide

### Issue: Type Conflicts in Elasticsearch
**Symptom:** Range queries return no results for numeric traits

**Cause:** First NFT indexed had string value, field locked as keyword

**Solution:**
1. Check which NFT was indexed first
2. If data is wrong, delete index and re-migrate
3. If data is correct, accept that trait isn't range-queryable for this collection
4. Document in collection config

### Issue: Bulk Indexing Failures
**Symptom:** Migration job reports failed batches

**Cause:**
- ES cluster out of memory
- Malformed documents
- Field limit exceeded

**Solution:**
1. Check ES logs for specific errors
2. Reduce batch size if memory issue
3. Validate document structure
4. Increase field limit if needed

### Issue: Query Latency High
**Symptom:** ES queries slower than PostgreSQL

**Cause:**
- Index not optimized
- Too many shards/replicas
- Queries not using indexes

**Solution:**
1. Check query execution plan
2. Verify indexes are being used
3. Adjust shard/replica count
4. Consider query optimization

---

## Code Files That Need ES Integration

### Indexer Service (Event Handlers)

**1. ERC721 Transfer Handler**
- **Path:** `mavis-marketplace-services/indexer/indexer/src/event/erc721_transfer.rs`
- **Changes:** Add ES update after Postgres update
- **Updates:** owner, is_shown, ownership_block_number, ownership_log_index, received_timestamp

**2. Order Exchange Handlers**
- **Path:** `mavis-marketplace-services/indexer/indexer/src/event/order_exchange.rs`
- **Changes:** Add ES updates for 3 event types:
  - `OrderCreated` → Set order fields
  - `OrderMatched` → Clear order fields, set price/matcher
  - `OrderCancelledVec` → Clear order fields

**3. Metadata Handler**
- **Path:** `mavis-marketplace-services/indexer/indexer-metadata/src/event/` (metadata update handler)
- **Changes:** Add ES update after Postgres update
- **Updates:** name, properties, image, cdn_image, metadata_last_updated, raw_metadata

**4. ERC1155 Transfer Handler (Optional - Lower Priority)**
- **Path:** `mavis-marketplace-services/indexer/indexer/src/event/erc1155_transfer.rs`
- **Changes:** Add ES update for ERC1155 data table

### GraphQL Service (Query Layer)

**5. ERC721 Token Query**
- **Path:** `mavis-marketplace-services/graphql/mavis-graphql-token/src/schema.rs`
- **Function:** `erc721_tokens()` (line ~278)
- **Changes:** 
  - Add ES query builder
  - Translate GraphQL filters to ES query
  - Add fallback to Postgres if ES fails
  - Maintain same response format

**6. Order Creation (Optional - For Validation)**
- **Path:** `mavis-marketplace-services/graphql/mavis-graphql-order/src/schema.rs`
- **Function:** `create_order()` (line ~446)
- **Changes:** Might need ES lookup for validation (optional)

### New Files to Create

**7. ES Client Module**
- **Path:** Create new module (location TBD)
- **Purpose:** 
  - ES connection management
  - Common ES operations (index, update, bulk)
  - Error handling and retries
  - Query builder utilities

**8. ES Mapping Template**
- **Path:** Create JSON file (location TBD)
- **Purpose:** ES index mapping with dynamic templates

**9. Migration Job**
- **Path:** `migrate-sample-erc721-data/src/` (this repository)
- **Purpose:**
  - Read from Postgres erc721 table
  - Transform to ES documents
  - Bulk index to ES
  - Checkpoint/resume support

### Database Schema Reference

**10. PostgreSQL Schema**
- **Path:** `mavis-marketplace-services/database/tracker_database_migration.sql`
- **Lines:** 828-861 (erc721 table), 897-923 (erc1155_data table)
- **Purpose:** Reference for ES document structure

### Configuration Files (To Be Updated)

**11. Environment Variables**
- Add ES_URL, ES_INDEX_PREFIX, ES_BATCH_SIZE, etc.
- Location depends on service configuration structure

**12. Dependency Files**
- **Paths:** 
  - `mavis-marketplace-services/indexer/indexer/Cargo.toml`
  - `mavis-marketplace-services/graphql/mavis-graphql-token/Cargo.toml`
  - `migrate-sample-erc721-data/Cargo.toml`
- **Changes:** Add `elasticsearch` crate dependency

---

## Summary of Changes Needed

### High Priority (Core Functionality)
1. ✅ Create ES mapping template
2. ✅ Create ES client module (shared)
3. ✅ Update `erc721_transfer.rs` (owner changes)
4. ✅ Update `order_exchange.rs` (order lifecycle)
5. ✅ Update metadata handler (metadata updates)
6. ✅ Update `erc721_tokens()` query (ES queries)
7. ✅ Create migration job (historical data)

### Medium Priority (Nice to Have)
8. ⚠️ Add monitoring/metrics
9. ⚠️ Add error handling/retries
10. ⚠️ Add integration tests

### Low Priority (Future)
11. 📋 ERC1155 support
12. 📋 Advanced ES features (aggregations, highlighting)

---

**Status:** Architecture and strategy documented, code paths identified
**Next:** Begin implementation with ES client module and mapping template
