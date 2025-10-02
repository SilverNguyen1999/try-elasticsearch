# Elasticsearch Setup Guide

## Step 1: Check if ES is Running

```bash
curl -X GET "http://localhost:9300/_cluster/health?pretty"
```

## Step 2: Check Existing Indices

```bash
curl -X GET "http://localhost:9300/_cat/indices?v"
```

## Step 3: Create the Index with Mapping

Use the mapping from `elasticsearch-mapping.json`:

```bash
curl -X PUT "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879?pretty" \
  -H 'Content-Type: application/json' \
  -d @elasticsearch-mapping.json
```

Or manually:

```bash
curl -X PUT "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879" \
  -H 'Content-Type: application/json' \
  -d '{
  "settings": {
    "number_of_shards": 3,
    "number_of_replicas": 1,
    "refresh_interval": "5s",
    "analysis": {
      "analyzer": {
        "nft_name_analyzer": {
          "tokenizer": "standard",
          "filter": ["lowercase", "asciifolding"]
        }
      }
    }
  },
  "mappings": {
    "dynamic": "strict",
    "properties": {
      "token_address": { "type": "keyword" },
      "token_id": { "type": "keyword" },
      "owner": { "type": "keyword" },
      "base_price": { "type": "double" },
      "ended_at": { "type": "long" },
      "ended_price": { "type": "double" },
      "expired_at": { "type": "long" },
      "kind": { "type": "long" },
      "maker": { "type": "keyword" },
      "matcher": { "type": "keyword" },
      "order_id": { "type": "long" },
      "payment_token": { "type": "keyword" },
      "price": { "type": "double" },
      "ron_price": { "type": "double" },
      "started_at": { "type": "long" },
      "state": { "type": "keyword" },
      "order_status": { "type": "keyword" },
      "name": {
        "type": "text",
        "analyzer": "nft_name_analyzer",
        "fields": {
          "keyword": { "type": "keyword" }
        }
      },
      "attributes": {
        "type": "flattened",
        "depth_limit": 20
      },
      "image": { "type": "keyword", "index": false },
      "cdn_image": { "type": "keyword", "index": false },
      "video": { "type": "keyword", "index": false },
      "animation_url": { "type": "keyword", "index": false },
      "description": { "type": "text", "index": false },
      "metadata_last_updated": { "type": "long" },
      "is_shown": { "type": "boolean" },
      "ownership_block_number": { "type": "long" },
      "ownership_log_index": { "type": "integer" },
      "raw_metadata": { "type": "object", "enabled": false }
    }
  }
}'
```

## Step 4: Verify Index Created

```bash
curl -X GET "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879?pretty"
```

## Step 5: Load Data from CSV

Set up your `.env` file:

```bash
cp env.example .env
```

Edit `.env`:
```
CSV_FILE=sample.csv
ELASTICSEARCH_URL=http://localhost:9300
ELASTICSEARCH_INDEX=0xa038c593115f6fcd673f6833e15462b475994879
BATCH_SIZE=100
WORKERS=4
TIMEOUT_SECS=30
```

Run the migrator:

```bash
cargo run --release
```

## Step 6: Verify Data Loaded

Check document count:
```bash
curl -X GET "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_count?pretty"
```

Get a sample document:
```bash
curl -X GET "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879/_doc/409192?pretty"
```

## Troubleshooting

### If index exists and you want to recreate it:

```bash
# Delete the index
curl -X DELETE "http://localhost:9300/0xa038c593115f6fcd673f6833e15462b475994879"

# Then create it again (Step 3)
```

### If ES is not responding:

Check if ES is running on the correct port (9300 for transport, 9200 for HTTP):

```bash
# Try HTTP port instead
curl -X GET "http://localhost:9200/_cluster/health?pretty"
```

Most likely you want to use port **9200** (HTTP) not 9300 (transport):

```bash
export ES_URL="http://localhost:9200"
export INDEX_NAME="0xa038c593115f6fcd673f6833e15462b475994879"
```

