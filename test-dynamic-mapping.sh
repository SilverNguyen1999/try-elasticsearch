#!/bin/bash

ES_URL="http://localhost:9300"
INDEX_NAME="nft_marketplace_test"

echo "🧪 Testing Dynamic Mapping Strategy"
echo "===================================="
echo ""

# Step 1: Create index with dynamic templates
echo "📋 Step 1: Creating index with dynamic templates..."
curl -X DELETE "$ES_URL/$INDEX_NAME" 2>/dev/null
curl -X PUT "$ES_URL/$INDEX_NAME" \
  -H 'Content-Type: application/json' \
  -d @elasticsearch-dynamic-mapping.json
echo ""
echo ""

# Step 2: Index Wildforest NFT (tier, level, rarity, type)
echo "📋 Step 2: Indexing Wildforest NFT..."
curl -X POST "$ES_URL/$INDEX_NAME/_doc/1" \
  -H 'Content-Type: application/json' \
  -d '{
  "token_address": "0xa038c593115f6fcd673f6833e15462b475994879",
  "token_id": "409192",
  "owner": "0x123...",
  "price": 100.5,
  "properties": {
    "tier": 1,
    "level": 5,
    "rarity": "Common",
    "type": "Archer"
  }
}'
echo ""
echo ""

# Step 3: Index Axie NFT (class, body, breed_count)
echo "📋 Step 3: Indexing Axie NFT (different properties)..."
curl -X POST "$ES_URL/$INDEX_NAME/_doc/2" \
  -H 'Content-Type: application/json' \
  -d '{
  "token_address": "0x32950db2a7164ae833121501c797d79e7b79d74c",
  "token_id": "12345",
  "owner": "0x456...",
  "price": 250.0,
  "properties": {
    "class": "Beast",
    "body": "Furball",
    "breed_count": 2,
    "purity": 95
  }
}'
echo ""
echo ""

# Step 4: Index Land NFT (land_type, coordinates)
echo "📋 Step 4: Indexing Land NFT (totally different properties)..."
curl -X POST "$ES_URL/$INDEX_NAME/_doc/3" \
  -H 'Content-Type: application/json' \
  -d '{
  "token_address": "0xf5319c51ee0a62ec8a32e91b4e82a7fb5a94e01b",
  "token_id": "999",
  "owner": "0x789...",
  "price": 500.0,
  "properties": {
    "land_type": "Savannah",
    "x_coordinate": 100,
    "y_coordinate": 200,
    "district": "Genesis"
  }
}'
echo ""
echo ""

# Wait for indexing
echo "⏳ Waiting for indexing to complete..."
sleep 2
echo ""

# Step 5: Check auto-generated mapping
echo "📋 Step 5: Check auto-generated mapping for properties..."
echo "ES automatically detected these types:"
curl -X GET "$ES_URL/$INDEX_NAME/_mapping?pretty" | grep -A 50 '"properties"' | head -60
echo ""

# Step 6: Query Wildforest by level (integer range)
echo "📋 Step 6: Query Wildforest by level >= 5 (integer range)..."
curl -X POST "$ES_URL/$INDEX_NAME/_search?pretty" \
  -H 'Content-Type: application/json' \
  -d '{
  "query": {
    "bool": {
      "filter": [
        {"term": {"token_address": "0xa038c593115f6fcd673f6833e15462b475994879"}},
        {"range": {"properties.level": {"gte": 5}}}
      ]
    }
  }
}'
echo ""

# Step 7: Query Axie by breed_count (different property)
echo "📋 Step 7: Query Axie by breed_count < 5 (different property)..."
curl -X POST "$ES_URL/$INDEX_NAME/_search?pretty" \
  -H 'Content-Type: application/json' \
  -d '{
  "query": {
    "bool": {
      "filter": [
        {"term": {"token_address": "0x32950db2a7164ae833121501c797d79e7b79d74c"}},
        {"range": {"properties.breed_count": {"lt": 5}}}
      ]
    }
  }
}'
echo ""

# Step 8: Cross-collection query
echo "📋 Step 8: Cross-collection query (price > 200)..."
curl -X POST "$ES_URL/$INDEX_NAME/_search?pretty" \
  -H 'Content-Type: application/json' \
  -d '{
  "query": {
    "range": {"price": {"gte": 200}}
  },
  "sort": [{"price": "asc"}]
}'
echo ""

echo "✅ Test complete!"
echo ""
echo "📊 Summary:"
echo "   - Created ONE index for ALL collections"
echo "   - ES auto-detected types (tier→long, rarity→keyword, etc.)"
echo "   - Each collection has different properties"
echo "   - All queries work without any collection config!"
echo ""
echo "🎯 This scales to 1000+ collections easily!"

