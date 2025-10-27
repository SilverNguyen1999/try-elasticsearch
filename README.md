# ERC721 Elasticsearch Integration

## 🎯 Project Goal

**Migrate NFT marketplace from PostgreSQL to Elasticsearch for 20-40x faster query performance** on complex filters and range queries across multi-collection NFT data.

## 📚 Documentation

**→ See [PROJECT_NOTES.md](PROJECT_NOTES.md) for complete architecture and implementation strategy**

This document covers:
- ✅ Why Elasticsearch (performance problem analysis)
- ✅ ES document structure and field mapping
- ✅ Type handling strategy (first type wins)
- ✅ Edge cases and solutions (60+ scenarios)
- ✅ Data synchronization strategy (order data, ownership, metadata)
- ✅ Event handlers that need ES updates
- ✅ GraphQL query translation
- ✅ Complete code file paths for implementation
- ✅ Rollout strategy (double-write → migration → query switch)

## 🚀 Quick Start

This repository contains the **migration job** to move existing PostgreSQL data to Elasticsearch.

**Main application:** `mavis-marketplace-services` (event handlers + GraphQL queries)

## 🏗️ Architecture Overview

### Three Main Components

**1. Double-Write (Event Handlers)**
- Location: `mavis-marketplace-services/indexer/indexer/`
- Updates both PostgreSQL AND Elasticsearch on every event
- Events: ERC721 Transfer, Order Created/Matched/Cancelled, Metadata Updated

**2. Migration Job (This Repository)**
- Reads existing data from PostgreSQL `erc721` table
- Transforms and bulk indexes to Elasticsearch
- Supports checkpoint/resume for large datasets

**3. Query Service (GraphQL)**
- Location: `mavis-marketplace-services/graphql/mavis-graphql-token/`
- Translates GraphQL filters to Elasticsearch queries
- 20-40x performance improvement over PostgreSQL

See [PROJECT_NOTES.md](PROJECT_NOTES.md) for detailed architecture, data flow, and implementation guide.

---

## 🔑 Key Design Decisions

### One Index Per Collection
Each NFT collection gets its own ES index (e.g., `erc721_0xa038...`).
- **Why:** Different collections have completely different traits
- **Benefit:** No field limit conflicts, optimized per collection

### Dynamic Properties with Type Detection
```json
{
  "properties": {
    "level": 5,           // double (first NFT had number)
    "rarity": "common",   // keyword (first NFT had string)
    "tribe": "Bageni"     // keyword
  }
}
```
- **Strategy:** First type wins, `ignore_malformed: true` for mismatches
- **Limit:** 60 traits per NFT, 100 total fields per index

### Full Data Synchronization
Not just metadata! ES documents include:
- **Ownership:** owner, is_shown, ownership_block_number
- **Order Data:** price, order_status, maker, expired_at, etc.
- **Metadata:** name, image, properties (dynamic traits), raw_metadata

**Why:** Support sorting and filtering on orders/ownership (current PostgreSQL requirement)

---

## 📂 Repository Structure

```
migrate-sample-erc721-data/
├── src/
│   ├── main.rs              # (To be implemented)
│   ├── config.rs            # (Existing config)
│   ├── elasticsearch.rs     # (To be updated for ES client)
│   └── ...
├── PROJECT_NOTES.md         # 📚 Complete implementation guide
├── README.md                # This file
└── Cargo.toml               # Dependencies
```

---

## 🚦 Implementation Status

**Documentation:** ✅ Complete (see PROJECT_NOTES.md)

**Code Implementation:** ⏳ Pending
- [ ] ES client module
- [ ] ES mapping template
- [ ] Migration job (read from Postgres → write to ES)
- [ ] Event handlers (double-write to ES)
- [ ] GraphQL query translator (ES queries)

---

## 📖 Further Reading

- **[PROJECT_NOTES.md](PROJECT_NOTES.md)** - Complete architecture, edge cases, implementation guide
- **Elasticsearch Docs:** [Dynamic Mapping](https://www.elastic.co/guide/en/elasticsearch/reference/current/dynamic-mapping.html)
- **Performance Analysis:** See "GraphQL Query Performance Problem" section in PROJECT_NOTES.md
