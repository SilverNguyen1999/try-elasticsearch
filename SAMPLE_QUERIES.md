# Elasticsearch Sample Queries for Performance Testing

This document contains sample Elasticsearch queries to test performance compared to PostgreSQL queries.

## Setup

Export your Elasticsearch URL and index name:
```bash
export ES_URL="http://localhost:9200"
export INDEX_NAME="erc721_tokens"
```

---

## 1. Get All Documents (Simple Scan)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 100,
  "query": {
    "match_all": {}
  }
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens LIMIT 100 OFFSET 0;
```

---

## 2. Query by Token ID (Exact Match)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "query": {
    "term": {
      "token_id": "123456"
    }
  }
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens WHERE token_id = '123456';
```

---

## 3. Query by Owner Address

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 100,
  "query": {
    "term": {
      "owner": "0x1234567890abcdef1234567890abcdef12345678"
    }
  }
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens 
WHERE owner = '0x1234567890abcdef1234567890abcdef12345678' 
LIMIT 100;
```

---

## 4. Query by Price Range (with Sorting)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 100,
  "query": {
    "range": {
      "price": {
        "gte": 100,
        "lte": 1000
      }
    }
  },
  "sort": [
    { "price": { "order": "asc" } }
  ]
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens 
WHERE price BETWEEN 100 AND 1000 
ORDER BY price ASC 
LIMIT 100;
```

---

## 5. Active Orders with Multiple Filters

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 100,
  "query": {
    "bool": {
      "must": [
        { "term": { "order_status": "active" } },
        { "term": { "is_shown": true } },
        { "range": { "price": { "lte": 5000 } } }
      ]
    }
  },
  "sort": [
    { "price": { "order": "asc" } },
    { "started_at": { "order": "desc" } }
  ]
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens 
WHERE order_status = 'active' 
  AND is_shown = true 
  AND price <= 5000 
ORDER BY price ASC, started_at DESC 
LIMIT 100;
```

---

## 6. Full-Text Search by Name (Fuzzy Match)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 50,
  "query": {
    "match": {
      "name": {
        "query": "dragon",
        "fuzziness": "AUTO"
      }
    }
  }
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens 
WHERE name ILIKE '%dragon%' 
LIMIT 50;
-- Or with pg_trgm extension for better fuzzy matching:
SELECT * FROM erc721_tokens 
WHERE name % 'dragon' 
ORDER BY similarity(name, 'dragon') DESC 
LIMIT 50;
```

---

## 7. Query by Attributes (Flattened Field)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 100,
  "query": {
    "term": {
      "attributes.tier": "1"
    }
  }
}
'
```

**PostgreSQL Equivalent:**
```sql
-- If attributes is JSONB:
SELECT * FROM erc721_tokens 
WHERE attributes->>'tier' = '1' 
LIMIT 100;

-- Or with GIN index:
SELECT * FROM erc721_tokens 
WHERE attributes @> '{"tier": "1"}' 
LIMIT 100;
```

---

## 8. Complex Bool Query with Multiple Conditions

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 100,
  "query": {
    "bool": {
      "must": [
        { "term": { "token_address": "0xabcdef1234567890abcdef1234567890abcdef12" } },
        { "term": { "order_status": "active" } }
      ],
      "filter": [
        { "range": { "price": { "gte": 100, "lte": 10000 } } },
        { "range": { "expired_at": { "gte": 1696291200 } } }
      ],
      "should": [
        { "term": { "payment_token": "0xc99a6a985ed2cac1ef41640596c5a5f9f4e19ef5" } }
      ],
      "minimum_should_match": 0
    }
  },
  "sort": [
    { "price": { "order": "asc" } }
  ]
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens 
WHERE token_address = '0xabcdef1234567890abcdef1234567890abcdef12' 
  AND order_status = 'active' 
  AND price BETWEEN 100 AND 10000 
  AND expired_at >= 1696291200 
ORDER BY price ASC 
LIMIT 100;
```

---

## 9. Aggregation - Price Statistics

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "size": 0,
  "aggs": {
    "price_stats": {
      "stats": {
        "field": "price"
      }
    },
    "ron_price_stats": {
      "stats": {
        "field": "ron_price"
      }
    },
    "avg_price_by_status": {
      "terms": {
        "field": "order_status",
        "size": 10
      },
      "aggs": {
        "avg_price": {
          "avg": {
            "field": "price"
          }
        }
      }
    }
  }
}
'
```

**PostgreSQL Equivalent:**
```sql
-- Basic stats:
SELECT 
  COUNT(*) as count,
  MIN(price) as min_price,
  MAX(price) as max_price,
  AVG(price) as avg_price,
  SUM(price) as sum_price
FROM erc721_tokens;

-- Grouped by status:
SELECT 
  order_status,
  COUNT(*) as count,
  AVG(price) as avg_price
FROM erc721_tokens
GROUP BY order_status;
```

---

## 10. Count Documents Matching Criteria

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_count?pretty" -H 'Content-Type: application/json' -d'
{
  "query": {
    "bool": {
      "must": [
        { "term": { "order_status": "active" } },
        { "range": { "price": { "lte": 1000 } } }
      ]
    }
  }
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT COUNT(*) FROM erc721_tokens 
WHERE order_status = 'active' 
  AND price <= 1000;
```

---

## 11. Get Document by ID (Direct Lookup)

```bash
curl -X GET "${ES_URL}/${INDEX_NAME}/_doc/123456?pretty"
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens WHERE token_id = '123456';
-- Or if you have a primary key:
SELECT * FROM erc721_tokens WHERE id = 123456;
```

---

## 12. Paginated Results with Sorting

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 1000,
  "size": 100,
  "query": {
    "term": { "order_status": "active" }
  },
  "sort": [
    { "started_at": { "order": "desc" } }
  ]
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens 
WHERE order_status = 'active' 
ORDER BY started_at DESC 
LIMIT 100 OFFSET 1000;
```

---

## 13. Multi-Field Search (Search Across Multiple Fields)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 50,
  "query": {
    "multi_match": {
      "query": "rare dragon",
      "fields": ["name", "description"],
      "type": "best_fields",
      "fuzziness": "AUTO"
    }
  }
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens 
WHERE name ILIKE '%rare%' 
   OR name ILIKE '%dragon%'
   OR description ILIKE '%rare%'
   OR description ILIKE '%dragon%'
LIMIT 50;
```

---

## 14. Filter by Multiple Owners (Terms Query)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 100,
  "query": {
    "terms": {
      "owner": [
        "0x1234567890abcdef1234567890abcdef12345678",
        "0xabcdef1234567890abcdef1234567890abcdef12",
        "0x9876543210fedcba9876543210fedcba98765432"
      ]
    }
  }
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens 
WHERE owner IN (
  '0x1234567890abcdef1234567890abcdef12345678',
  '0xabcdef1234567890abcdef1234567890abcdef12',
  '0x9876543210fedcba9876543210fedcba98765432'
) 
LIMIT 100;
```

---

## 15. Range Query on Timestamps

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 100,
  "query": {
    "bool": {
      "must": [
        { "range": { "started_at": { "gte": 1696291200 } } },
        { "range": { "expired_at": { "lte": 1698969600 } } }
      ]
    }
  },
  "sort": [
    { "started_at": { "order": "desc" } }
  ]
}
'
```

**PostgreSQL Equivalent:**
```sql
SELECT * FROM erc721_tokens 
WHERE started_at >= 1696291200 
  AND expired_at <= 1698969600 
ORDER BY started_at DESC 
LIMIT 100;
```

---

## Performance Testing Tips

### 1. Measure Query Time with curl
```bash
time curl -X POST "${ES_URL}/${INDEX_NAME}/_search" -H 'Content-Type: application/json' -d'
{
  "query": { "match_all": {} },
  "size": 100
}
'
```

### 2. Use Elasticsearch Profile API for Detailed Breakdown
```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "profile": true,
  "query": {
    "bool": {
      "must": [
        { "term": { "order_status": "active" } },
        { "range": { "price": { "lte": 1000 } } }
      ]
    }
  }
}
'
```

### 3. Check Index Statistics
```bash
curl -X GET "${ES_URL}/${INDEX_NAME}/_stats?pretty"
```

### 4. Force Cache Clear Before Testing
```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_cache/clear?pretty"
```

### 5. Warm Up Queries (Run Once Before Timing)
Run each query 2-3 times before measuring to ensure fair comparison with warm caches.

---

## Comparison Metrics to Track

When comparing ES vs PostgreSQL, track these metrics:

1. **Query Execution Time** - Time from request to response
2. **Throughput** - Queries per second under load
3. **Latency Percentiles** - p50, p95, p99 response times
4. **Memory Usage** - During query execution
5. **CPU Usage** - During query execution
6. **Index Size** - Storage requirements
7. **Write Performance** - Indexing speed (already tested in your migrator)
8. **Concurrent Query Performance** - Response time with multiple simultaneous queries

### Example Load Test Script

```bash
#!/bin/bash
# Run 100 concurrent queries and measure average time

for i in {1..100}; do
  (curl -s -w "%{time_total}\n" -o /dev/null \
    -X POST "${ES_URL}/${INDEX_NAME}/_search" \
    -H 'Content-Type: application/json' \
    -d '{"query":{"term":{"order_status":"active"}},"size":100}') &
done | awk '{sum+=$1; count++} END {print "Average:", sum/count, "seconds"}'
```

---

## 16. Real Marketplace Query - Complex Multi-Filter Search

This query mimics the actual GraphQL query used in the Ronin Marketplace:

**GraphQL Query Parameters:**
- `tokenAddress`: "0xa038c593115f6fcd673f6833e15462b475994879"
- `auctionType`: "All"
- `criteria`: [{"name":"rarity","values":["Common"]}]
- `rangeCriteria`: [{"name":"level","range":{"from":5,"to":∞}}]
- `sort`: "PriceAsc"
- `from`: 0, `size`: 50

**Elasticsearch Equivalent:**

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 50,
  "query": {
    "bool": {
      "must": [
        {
          "term": {
            "token_address": "0xa038c593115f6fcd673f6833e15462b475994879"
          }
        },
        {
          "term": {
            "attributes.rarity": "Common"
          }
        },
        {
          "range": {
            "attributes.level": {
              "gte": 5
            }
          }
        }
      ],
      "filter": [
        {
          "term": {
            "order_status": "active"
          }
        },
        {
          "term": {
            "is_shown": true
          }
        }
      ]
    }
  },
  "sort": [
    {
      "price": {
        "order": "asc"
      }
    }
  ],
  "_source": {
    "includes": [
      "token_address",
      "token_id",
      "owner",
      "name",
      "image",
      "cdn_image",
      "video",
      "attributes",
      "price",
      "ron_price",
      "order_id",
      "order_status",
      "maker",
      "payment_token",
      "started_at",
      "expired_at",
      "base_price",
      "kind"
    ]
  }
}
'
```

**PostgreSQL Equivalent:**

```sql
SELECT 
  token_address,
  token_id,
  owner,
  name,
  image,
  cdn_image,
  video,
  attributes,
  price,
  ron_price,
  order_id,
  order_status,
  maker,
  payment_token,
  started_at,
  expired_at,
  base_price,
  kind
FROM erc721_tokens 
WHERE token_address = '0xa038c593115f6fcd673f6833e15462b475994879'
  AND order_status = 'active'
  AND is_shown = true
  AND attributes->>'rarity' = 'Common'
  AND CAST(attributes->>'level' AS INTEGER) >= 5
ORDER BY price ASC 
LIMIT 50 OFFSET 0;
```

---

## 17. Marketplace Query Variations

### A. Filter by Multiple Rarity Values (OR condition)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 50,
  "query": {
    "bool": {
      "must": [
        {
          "term": {
            "token_address": "0xa038c593115f6fcd673f6833e15462b475994879"
          }
        },
        {
          "terms": {
            "attributes.rarity": ["Common", "Rare", "Epic"]
          }
        }
      ],
      "filter": [
        {
          "term": {
            "order_status": "active"
          }
        }
      ]
    }
  },
  "sort": [
    {
      "price": {
        "order": "asc"
      }
    }
  ]
}
'
```

**PostgreSQL:**
```sql
SELECT * FROM erc721_tokens 
WHERE token_address = '0xa038c593115f6fcd673f6833e15462b475994879'
  AND order_status = 'active'
  AND attributes->>'rarity' IN ('Common', 'Rare', 'Epic')
ORDER BY price ASC 
LIMIT 50;
```

### B. Multiple Range Criteria (Level + Price)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 50,
  "query": {
    "bool": {
      "must": [
        {
          "term": {
            "token_address": "0xa038c593115f6fcd673f6833e15462b475994879"
          }
        }
      ],
      "filter": [
        {
          "range": {
            "attributes.level": {
              "gte": 5,
              "lte": 30
            }
          }
        },
        {
          "range": {
            "price": {
              "gte": 100,
              "lte": 10000
            }
          }
        },
        {
          "term": {
            "order_status": "active"
          }
        }
      ]
    }
  },
  "sort": [
    {
      "price": {
        "order": "asc"
      }
    }
  ]
}
'
```

**PostgreSQL:**
```sql
SELECT * FROM erc721_tokens 
WHERE token_address = '0xa038c593115f6fcd673f6833e15462b475994879'
  AND order_status = 'active'
  AND CAST(attributes->>'level' AS INTEGER) BETWEEN 5 AND 30
  AND price BETWEEN 100 AND 10000
ORDER BY price ASC 
LIMIT 50;
```

### C. Filter by Owner (Exclude Specific Address)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 50,
  "query": {
    "bool": {
      "must": [
        {
          "term": {
            "token_address": "0xa038c593115f6fcd673f6833e15462b475994879"
          }
        },
        {
          "term": {
            "order_status": "active"
          }
        }
      ],
      "must_not": [
        {
          "term": {
            "owner": "0x1234567890abcdef1234567890abcdef12345678"
          }
        }
      ]
    }
  },
  "sort": [
    {
      "price": {
        "order": "asc"
      }
    }
  ]
}
'
```

**PostgreSQL:**
```sql
SELECT * FROM erc721_tokens 
WHERE token_address = '0xa038c593115f6fcd673f6833e15462b475994879'
  AND order_status = 'active'
  AND owner != '0x1234567890abcdef1234567890abcdef12345678'
ORDER BY price ASC 
LIMIT 50;
```

### D. Search by Name + Filters (Text Search)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 50,
  "query": {
    "bool": {
      "must": [
        {
          "term": {
            "token_address": "0xa038c593115f6fcd673f6833e15462b475994879"
          }
        },
        {
          "match": {
            "name": {
              "query": "dragon",
              "fuzziness": "AUTO"
            }
          }
        }
      ],
      "filter": [
        {
          "term": {
            "order_status": "active"
          }
        },
        {
          "range": {
            "price": {
              "lte": 5000
            }
          }
        }
      ]
    }
  },
  "sort": [
    {
      "_score": {
        "order": "desc"
      }
    },
    {
      "price": {
        "order": "asc"
      }
    }
  ]
}
'
```

**PostgreSQL:**
```sql
SELECT * FROM erc721_tokens 
WHERE token_address = '0xa038c593115f6fcd673f6833e15462b475994879'
  AND order_status = 'active'
  AND name ILIKE '%dragon%'
  AND price <= 5000
ORDER BY price ASC 
LIMIT 50;
```

### E. Multiple Attributes Filter (AND conditions)

```bash
curl -X POST "${ES_URL}/${INDEX_NAME}/_search?pretty" -H 'Content-Type: application/json' -d'
{
  "from": 0,
  "size": 50,
  "query": {
    "bool": {
      "must": [
        {
          "term": {
            "token_address": "0xa038c593115f6fcd673f6833e15462b475994879"
          }
        },
        {
          "term": {
            "attributes.rarity": "Common"
          }
        },
        {
          "term": {
            "attributes.class": "Beast"
          }
        },
        {
          "term": {
            "attributes.part_type": "Horn"
          }
        }
      ],
      "filter": [
        {
          "term": {
            "order_status": "active"
          }
        },
        {
          "range": {
            "attributes.level": {
              "gte": 5
            }
          }
        }
      ]
    }
  },
  "sort": [
    {
      "price": {
        "order": "asc"
      }
    }
  ]
}
'
```

**PostgreSQL:**
```sql
SELECT * FROM erc721_tokens 
WHERE token_address = '0xa038c593115f6fcd673f6833e15462b475994879'
  AND order_status = 'active'
  AND attributes->>'rarity' = 'Common'
  AND attributes->>'class' = 'Beast'
  AND attributes->>'part_type' = 'Horn'
  AND CAST(attributes->>'level' AS INTEGER) >= 5
ORDER BY price ASC 
LIMIT 50;
```

---

## Performance Comparison Notes

### Key Differences Between ES and PostgreSQL for This Use Case:

1. **Flattened Attributes**
   - **ES**: `attributes.rarity` - direct dot notation, indexed efficiently
   - **PG**: `attributes->>'rarity'` - JSONB extraction, requires GIN index

2. **Multiple Filters**
   - **ES**: Bool query with must/filter/should - highly optimized
   - **PG**: Multiple AND/OR conditions - depends on index strategy

3. **Full-Text Search**
   - **ES**: Native text analysis with custom analyzers (`nft_name_analyzer`)
   - **PG**: ILIKE or pg_trgm extension needed for fuzzy matching

4. **Sorting on Filtered Results**
   - **ES**: Can use doc values for efficient sorting
   - **PG**: May need composite indexes for optimal performance

### Recommended Test Scenarios:

1. **Simple filter** (token_address + order_status)
2. **Single attribute filter** (+ rarity)
3. **Multiple attribute filters** (+ rarity + level range)
4. **Price range + attributes** (most complex)
5. **Text search + filters** (full-text capability)
6. **Aggregations on filtered results** (stats by rarity)

### Test with Different Data Volumes:

```bash
# Test with 1K records
# Test with 10K records
# Test with 100K records
# Test with 1M+ records (real marketplace scale)
```

For accurate comparison, run each query 10 times and calculate:
- Average response time
- p50, p95, p99 latency
- Memory usage
- CPU usage during query

