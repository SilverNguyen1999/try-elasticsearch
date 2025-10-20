# NFT Marketplace Elasticsearch Strategy

**Date:** October 15, 2025  
**Goal:** Design Elasticsearch solution for NFT trait filtering that handles all edge cases

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

## Type Handling: First Type Wins

### How Elasticsearch Dynamic Mapping Works

**Key Concept:** When a field appears for the first time, ES detects its JSON type and creates the mapping accordingly.

**Process:**
1. **First NFT indexed with trait "level": 1**
   - ES sees: JSON number
   - Creates: `properties.level` field with type `long`
   
2. **Second NFT with "level": 5**
   - Type matches → indexed successfully
   
3. **Third NFT with "level": "max"**
   - Type doesn't match (string vs long)
   - With `ignore_malformed: true` → field value skipped, document still indexed
   - Without it → entire document rejected (bad!)

### Dynamic Templates Configuration

**Purpose:** Tell ES how to map new fields when they appear

**Rules needed:**
- If field under `properties.*` is detected as string → map as `keyword`
- If field under `properties.*` is detected as long → map as `long` with `ignore_malformed: true`
- If field under `properties.*` is detected as double → map as `double` with `ignore_malformed: true`
- If field under `properties.*` is detected as boolean → map as `boolean` with `ignore_malformed: true`

**What `ignore_malformed` does:**
- Allows document to be indexed even if field value doesn't match expected type
- Silently skips the problematic field value
- Other fields in document work normally
- Prevents indexing failures

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
     }
   }
   ```

**Extraction Strategy:**
- **Priority 1:** Use `raw_metadata.properties` if available (has correct types already)
- **Priority 2:** Use `attributes` as fallback (need to convert string arrays to values)

**Conversion Logic for Attributes:**
- Extract first element from array: `["1"]` → `"1"`
- Attempt type detection:
  - If string contains only digits → parse as integer
  - If string contains decimal → parse as float
  - If parse fails → keep as string
- Preserve whatever type we get

---

## Edge Cases & Solutions

### Case 1: First NFT Has Wrong Type (CRITICAL for Range Queries!)

**Problem:**
```
NFT #1: {level: "1"}     → ES creates field as keyword (string)
NFT #2: {level: 5}       → Coerced to string "5"
NFT #3: {level: 10}      → Coerced to string "10"

Query: level >= 5
Result: WRONG! String comparison: "5" > "10" → Returns NFT #1, #2, #3
Correct result should be: NFT #2, #3 only

This BREAKS range queries completely! ❌
```

**Why This Is Critical:**
- Range queries are the primary reason for using Elasticsearch
- If numeric traits indexed as strings → **range queries unusable**
- Must prevent this at all costs

**Real-World Impact:**
```
User searches: "Show me level 5+ NFTs"
- If correct type: Gets 1000 NFTs (level 5-100)
- If wrong type: Gets 500 NFTs (missing level 6-9, 60-69, etc.)
- User sees incomplete results, loses trust
```

**Solutions:**

**Option A: Pre-validation (REQUIRED for range-queryable traits)**
- Before creating index, sample first 100-1000 NFTs
- Identify numeric traits (level, tier, power, stats, etc.)
- Analyze type distribution:
  - If 95%+ are numbers → This trait should be numeric
  - If mixed → Determine which type is correct by convention
- Sort NFTs to index numeric-first for numeric traits
- Ensures correct type wins

**Option B: Reindex on discovery**
- When problem discovered → delete index
- Recreate with correct type
- Re-index all NFTs
- Downtime required (~5-30 minutes)

**Option C: Accept and document**
- ❌ NOT acceptable for traits that need range queries
- Only acceptable for traits that are truly string-based

**Recommendation:** 
- MUST use Option A (pre-validation) for traits that need range queries
- Identify common numeric traits: level, tier, rank, power, attack, defense, hp, etc.
- These MUST be indexed as numbers

### Case 2: Type Conflict Within Collection

**Problem:**
```
NFT #100: {level: 1}      → Number (indexed)
NFT #500: {level: "max"}  → String (skipped)

Query: level >= 5
Result: NFT #500 not returned (its level value was ignored)
```

**Is this acceptable?**
- If <1% of NFTs affected → Yes, acceptable
- If >10% affected → Collection has data quality issue, should fix at source

**Handling:**
- Log when conflicts occur (which collection, which trait, which NFT)
- Monitor conflict rate per collection
- Alert collection owner if >10% conflict rate
- Provide data quality report

**What about the skipped NFTs?**
- They're still indexed (other traits work)
- Still searchable by owner, token_id, other traits
- Just missing the conflicting trait in search results
- Full data still in `raw_metadata` for API response

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

## Pre-validation Strategy

### Before Creating Index

**Step 1: Sample Collection**
- Fetch first 1000 NFTs from PostgreSQL
- Extract all `raw_metadata.properties` or `attributes`

**Step 2: Identify Range-Queryable Traits**

**Critical Step:** Determine which traits users will filter by range

**Common patterns for numeric traits:**
- Names containing: level, tier, rank, power, stat, attack, defense, hp, mp, speed
- Names containing: score, rating, point, value, amount, count, quantity
- Names containing numbers: gen1, wave2, phase3

**Heuristic:**
- If trait name suggests progression/hierarchy → likely numeric
- If trait appears with consistently numeric values → must be numeric

**Step 3: Analyze Traits**
- Identify all unique trait names
- For each trait, analyze types:
  - Count: How many times it appears
  - Types: Distribution of JSON types (number, string, boolean)
  - Range-queryable: Is this trait likely filtered by range?
  - Example: "level": 950 numbers, 50 strings

**Step 4: Generate Report**
```
Collection: 0xa038...
Total NFTs sampled: 1000
Total unique traits: 8

Trait: level ⚠️ RANGE-QUERYABLE
  - Appears: 1000 times (100%)
  - Types: 95% number, 5% string
  - Recommendation: MUST index as long (required for range queries)
  - Action: Sort NFTs to index numbers first
  - Priority: CRITICAL

Trait: tier ⚠️ RANGE-QUERYABLE
  - Appears: 1000 times (100%)
  - Types: 100% number
  - Recommendation: Index as long
  - Action: None needed (consistent type)
  - Priority: CRITICAL

Trait: rarity
  - Appears: 1000 times (100%)
  - Types: 100% string
  - Recommendation: Index as keyword
  - Action: None needed
  - Priority: Normal

Trait: type
  - Appears: 1000 times (100%)
  - Types: 100% string
  - Recommendation: Index as keyword
  - Action: None needed
  - Priority: Normal

Trait: perk1
  - Appears: 450 times (45%)
  - Types: 100% string
  - Recommendation: Index as keyword
  - Action: None needed (optional trait)
  - Priority: Low
```

**Step 5: Sort Indexing Order (CRITICAL for Range-Queryable Traits)**
- **For range-queryable traits with mixed types:**
  - MUST sort NFTs to index numeric values first
  - Example: For "level" (95% number), index NFTs with numeric level first
  - This ensures "first type wins" gets numeric type
  
- **For non-range-queryable traits:**
  - Sort by majority type (optional, nice to have)
  - Less critical since string filtering works either way

**Step 6: Alert if Critical Issues Found**
```
❌ BLOCKER: Trait "level" has mixed types (50% number, 50% string)
   - Action required: Contact collection owner
   - Must fix data at source before indexing
   - Range queries will not work correctly otherwise

⚠️  WARNING: Trait "power" exists but 20% are non-numeric
   - Can proceed with sorting strategy
   - 20% of NFTs will not appear in range queries
   - Consider notifying collection owner

✅ OK: All critical traits have consistent types
   - Safe to proceed with indexing
```

---

## Final Strategy Summary

### Core Decisions

**1. Type Handling: First Type Wins**
- When a trait appears for the first time → ES detects type and creates field
- Subsequent NFTs must match that type
- If type doesn't match → value skipped (with `ignore_malformed: true`)
- Example: If "level" first indexed as number → all string "level" values ignored

**2. Multiple Values for Same Trait: ES Arrays**
- Convert from attributes array format: `[{"trait_type": "Accessory", "value": "A"}, {"trait_type": "Accessory", "value": "B"}]`
- Group duplicate trait_type → Create ES array: `{"Accessory": ["A", "B"]}`
- All values preserved and searchable
- Queries use `terms` for OR logic, `bool.must` for AND between different traits
- Pre-existing arrays in raw_metadata → ignored

**3. Field Limits**
- Total ES fields: **100 maximum** (`index.mapping.total_fields.limit: 100`)
- Fixed fields: ~30 (token_address, owner, price, order fields, etc.)
- Trait fields: **60 maximum**
- Buffer: ~10 for safety
- NFTs with >60 traits → First 60 indexed (prioritize range-queryable traits)

**4. Dynamic Mapping Configuration**
- `dynamic: true` → Auto-create new trait fields
- Dynamic templates for `properties.*` path:
  - String values → `keyword` type (exact match)
  - Long values → `long` type with `ignore_malformed: true`
  - Double values → `double` type with `ignore_malformed: true`
  - Boolean values → `boolean` type with `ignore_malformed: true`
- Arrays supported (created from duplicate trait_type only)
- Nested objects → skipped entirely

**5. Data Not Supported (Will Be Skipped)**
- ❌ Nested objects in traits
- ❌ Pre-existing arrays in raw_metadata
- ❌ Mixed types in same trait (non-numeric values in numeric fields)
- ❌ Traits beyond 60 limit

**6. Critical for Range Queries**
- Pre-validation required for numeric traits (level, tier, power, etc.)
- Must ensure numeric type wins first
- Sort indexing order if needed
- This is non-negotiable - range queries break if wrong type wins

---

## Summary of Strategy

### Core Principles
1. **Range queries are the primary goal** (why we need ES)
2. **One index per collection** (isolated schemas)
3. **First type wins** (ES dynamic mapping behavior)
4. **Numeric types are critical** (required for range queries)
5. **Pre-validate range-queryable traits** (MUST get type right)
6. **Ignore malformed** (graceful degradation for edge cases)
7. **Log everything** (monitor edge cases)

### Critical Success Factors
1. **Identify range-queryable traits before indexing**
   - Common patterns: level, tier, rank, power, stats
   - MUST be indexed as numeric types
   
2. **Pre-validation is mandatory for numeric traits**
   - Sample collection first
   - Analyze type distribution
   - Sort indexing order if needed
   
3. **Range query correctness > Everything else**
   - If numeric trait indexed as string → Entire ES solution fails
   - Users get wrong/incomplete results
   - Better to block indexing than index with wrong type

### Acceptable Trade-offs
- NFTs with non-numeric values in numeric fields: Values skipped but document indexed (<10% acceptable)
- NFTs with >60 traits: Extra traits not indexed (rare edge case)
- Nested object traits: Skipped entirely, not indexed
- Type conflicts <10%: Acceptable data quality
- Empty/null values: Excluded from queries (expected behavior)

### Unacceptable Scenarios
- ❌ Range-queryable trait indexed as string (breaks primary use case)
- ❌ Wrong type wins first for numeric traits (use pre-validation)
- ❌ Indexing failures (use ignore_malformed to prevent)
- ❌ Type conflict rate >10% (alert collection owner, may need data fix)

### When to Block Indexing
- Range-queryable trait has 50/50 mixed types
- More than 20% type conflicts for critical numeric traits
- Data quality too poor to provide correct results

### When to Alert Collection Owner
- Range-queryable trait has >10% non-numeric values
- Trait count >60 in multiple NFTs (exceeding limit)
- Nested object traits detected (will be skipped)
- Large percentage of null/empty values

---

**Status:** Core strategy designed, range query support prioritized  
**Next:** Implement mapping template and pre-validation tool with range query focus
