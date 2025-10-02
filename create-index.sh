#!/bin/bash

# Create Elasticsearch index for NFT tokens with optimized mapping
# Usage: ./create-index.sh [elasticsearch_url] [index_name]

ELASTICSEARCH_URL=${1:-"http://localhost:9300"}
INDEX_NAME=${2:-"nft_tokens"}

echo "Creating index '$INDEX_NAME' at $ELASTICSEARCH_URL..."

# Delete index if it exists
curl -X DELETE "$ELASTICSEARCH_URL/$INDEX_NAME" 2>/dev/null

# Create index with mapping
curl -X PUT "$ELASTICSEARCH_URL/$INDEX_NAME" \
  -H 'Content-Type: application/json' \
  -d @elasticsearch-mapping.json

if [ $? -eq 0 ]; then
    echo "✅ Index '$INDEX_NAME' created successfully!"
    echo ""
    echo "Verify with:"
    echo "curl -X GET \"$ELASTICSEARCH_URL/$INDEX_NAME/_mapping?pretty\""
else
    echo "❌ Failed to create index"
    exit 1
fi
