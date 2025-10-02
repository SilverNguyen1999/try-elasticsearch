# ERC721 Elasticsearch Migrator

A high-performance Rust application for migrating ERC721 NFT data from CSV files to Elasticsearch.

## Features

- **High Performance**: Built in Rust with async/await and concurrent processing
- **Flexible Attribute Mapping**: Handles dynamic NFT attributes using Elasticsearch's flattened field type
- **Bulk Indexing**: Efficient batch processing with configurable batch sizes
- **Resume/Checkpoint**: Automatic progress saving and resume from interruptions
- **Graceful Shutdown**: Ctrl+C saves progress before exit
- **Error Handling**: Robust error handling with retry capabilities
- **Concurrent Workers**: Configurable number of concurrent workers for optimal throughput

## Prerequisites

- Rust 1.70+ installed
- Elasticsearch running (default: localhost:9300)
- CSV file with ERC721 data

## Installation

```bash
# Navigate to the project directory
cd /path/to/migrate-sample-erc721-data

# Build the project
cargo build --release
```

## Configuration

The application uses environment variables for configuration. Copy `env.example` to `.env` and modify as needed:

```bash
cp env.example .env
```

### Environment Variables

- `ELASTICSEARCH_URL`: Elasticsearch endpoint (default: http://localhost:9300)
- `ELASTICSEARCH_INDEX`: Index name (default: nft_tokens)
- `BATCH_SIZE`: Documents per batch (default: 2000)
- `WORKERS`: Concurrent workers (default: 6)
- `TIMEOUT_SECS`: HTTP timeout (default: 30)

## Usage

### Basic Usage
```bash
./target/release/erc721-elasticsearch-migrator data.csv
```

### With Custom Environment
```bash
# Override specific settings
BATCH_SIZE=5000 WORKERS=8 ./target/release/erc721-elasticsearch-migrator data.csv
```

## Resume/Checkpoint Feature

The migrator automatically saves progress and can resume from interruptions:

### Automatic Checkpointing
- Progress is saved every 10 batches
- Checkpoint file: `<csv_filename>.checkpoint`
- Contains: processed records, batch counts, timestamps

### Resume Migration
```bash
# Simply run the same command - it will automatically resume
./target/release/erc721-elasticsearch-migrator data.csv

# Output will show:
# 📁 Found checkpoint: 45.2% complete (678000/1500000 records)
# 🔄 Resuming from record 678000
```

### Graceful Shutdown
- Press `Ctrl+C` to stop migration safely
- Progress is saved before exit
- Resume later with the same command

### Manual Checkpoint Management
```bash
# View checkpoint status
cat data.csv.checkpoint

# Remove checkpoint to restart from beginning
rm data.csv.checkpoint
```

## CSV Format

The CSV file should contain the following columns:
- `token_address`: Contract address of the NFT
- `token_id`: Unique token ID
- `owner`: Current owner address
- `name`: NFT name
- `attributes`: JSON string containing NFT attributes
- `price`, `ron_price`: Pricing information
- `image`, `cdn_image`: Image URLs
- And many more fields...

### Example CSV Row
```csv
token_address,token_id,name,attributes,price,ron_price,...
0xa038c593115f6fcd673f6833e15462b475994879,409192,Archer,"{""tier"": [""1""], ""type"": [""archer""], ""rarity"": [""common""]}",100.5,95.2,...
```

## Elasticsearch Index Mapping

The application expects an Elasticsearch index with the following key features:

- **Flattened attributes field**: For dynamic NFT trait filtering
- **Proper field types**: Optimized for search and aggregation
- **Scalable design**: Handles millions of NFT records

### Create the Index
Before running the migrator, create the Elasticsearch index:

```bash
curl -X PUT "localhost:9300/nft_tokens" -H 'Content-Type: application/json' -d '{
  "mappings": {
    "properties": {
      "token_address": { "type": "keyword" },
      "token_id": { "type": "keyword" },
      "owner": { "type": "keyword" },
      "name": { "type": "text", "fields": { "keyword": { "type": "keyword" } } },
      "attributes": { "type": "flattened", "depth_limit": 20 },
      "price": { "type": "double" },
      "ron_price": { "type": "double" },
      "is_shown": { "type": "boolean" }
    }
  }
}'
```

## Performance

The migrator is optimized for high performance:

- **Concurrent Processing**: Multiple workers process batches in parallel
- **Bulk API**: Uses Elasticsearch's bulk API for efficient indexing
- **Memory Efficient**: Streams CSV data to avoid loading entire file in memory
- **Progress Tracking**: Real-time feedback on migration progress

### Performance Tips

1. **Increase batch size** for larger datasets (2000-5000)
2. **Adjust worker count** based on your CPU cores and Elasticsearch cluster
3. **Monitor Elasticsearch** cluster health during migration
4. **Use SSD storage** for both CSV file and Elasticsearch data

## Example Output

```
2023-09-29T10:30:00Z INFO  Starting ERC721 Elasticsearch migration
2023-09-29T10:30:00Z INFO  CSV file: "nft_data.csv"
2023-09-29T10:30:00Z INFO  Elasticsearch URL: http://localhost:9300
2023-09-29T10:30:00Z INFO  Elasticsearch connection verified
2023-09-29T10:30:05Z INFO  Finished reading 1500000 records from CSV
2023-09-29T10:30:05Z INFO  Processing 1500 batches with 4 workers
[00:05:23] ████████████████████████████████████████ 1500000/1500000 Indexed 1500000 documents
2023-09-29T10:35:28Z INFO  Migration summary:
2023-09-29T10:35:28Z INFO    Total records: 1500000
2023-09-29T10:35:28Z INFO    Successfully processed: 1500000
2023-09-29T10:35:28Z INFO    Successful batches: 1500
2023-09-29T10:35:28Z INFO    Failed batches: 0
2023-09-29T10:35:28Z INFO  All batches processed successfully!
```

## Querying the Data

Once migrated, you can query your NFT data with complex filters:

```bash
# Search by name
curl -X GET "localhost:9300/nft_tokens/_search" -H 'Content-Type: application/json' -d '{
  "query": { "match": { "name": "Archer" } }
}'

# Filter by attributes
curl -X GET "localhost:9300/nft_tokens/_search" -H 'Content-Type: application/json' -d '{
  "query": {
    "bool": {
      "must": [
        { "term": { "attributes.rarity": "common" } },
        { "term": { "attributes.type": "archer" } }
      ]
    }
  }
}'

# Price range query with sorting
curl -X GET "localhost:9300/nft_tokens/_search" -H 'Content-Type: application/json' -d '{
  "query": { "range": { "ron_price": { "gte": 10, "lte": 100 } } },
  "sort": [{ "ron_price": { "order": "asc" } }]
}'
```

## Troubleshooting

### Common Issues

1. **Connection refused**: Ensure Elasticsearch is running on the specified port
2. **Index already exists**: Delete the index first or use a different name
3. **Out of memory**: Reduce batch size or worker count
4. **CSV parsing errors**: Check CSV format and encoding

### Logs

The application provides detailed logging. Set `RUST_LOG=debug` for verbose output:

```bash
RUST_LOG=debug ./target/release/erc721-elasticsearch-migrator --csv-file data.csv
```
