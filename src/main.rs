mod checkpoint;
mod config;
mod elasticsearch;
mod models;
mod models_flexible;
mod collection_config;

use anyhow::{Context, Result};
use csv::ReaderBuilder;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::signal;

use crate::checkpoint::MigrationCheckpoint;
use crate::config::APP_CONFIG;
use crate::elasticsearch::bulk_index_documents;
use crate::models_flexible::{CsvRecord, FlexibleElasticsearchDocument};
use crate::collection_config::get_collection_config;

#[tokio::main]
async fn main() -> Result<()> {
    let csv_file = &APP_CONFIG.csv_file;
    
    // Check for existing checkpoint
    let mut checkpoint = match MigrationCheckpoint::load(csv_file).await? {
        Some(cp) => {
            let resume_point = cp.get_safe_resume_point();
            println!("📁 Found checkpoint: {:.1}% complete ({}/{} records)", 
                     cp.progress_percentage(), cp.processed_records, cp.total_records);
            println!("🔄 Resuming from record {} (safe continuous point)", resume_point);
            cp
        }
        None => {
            println!("🆕 Starting new migration: {}", csv_file);
            // We'll create the checkpoint after reading the CSV
            MigrationCheckpoint::new(csv_file.to_string(), 0)
        }
    };
    
    println!("Config: Elasticsearch={}, Index={}, Batch={}, Workers={}", 
             APP_CONFIG.elasticsearch_url, APP_CONFIG.elasticsearch_index, 
             APP_CONFIG.batch_size, APP_CONFIG.workers);
    let start_time = Instant::now();

    let client = Client::builder()
        .timeout(Duration::from_secs(APP_CONFIG.timeout_secs))
        .build()
        .context("Failed to create HTTP client")?;

    // Test connection
    let health_response = client.get(&format!("{}/_cluster/health", APP_CONFIG.elasticsearch_url)).send().await?;
    if !health_response.status().is_success() {
        return Err(anyhow::anyhow!("Elasticsearch not available"));
    }
    println!("✓ Elasticsearch connected");

    // Read CSV
    let file = std::fs::File::open(csv_file)?;
    let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);
    
    let mut records = Vec::new();
    let mut record_index = 0;
    let resume_point = checkpoint.get_safe_resume_point();
    
    for result in reader.deserialize() {
        let record: CsvRecord = result?;
        
        // Skip records that were already safely processed
        if record_index < resume_point {
            record_index += 1;
            continue;
        }
        
        records.push((record_index, record)); // Store with original index
        record_index += 1;
    }
    
    let total_records = record_index; // Total in CSV
    let remaining_records = records.len(); // Records to process
    
    // Update checkpoint with total if it's new
    if checkpoint.total_records == 0 {
        checkpoint.total_records = total_records;
    }
    
    println!("✓ CSV has {} total records", total_records);
    if remaining_records < total_records {
        println!("✓ Skipping {} safely processed records", total_records - remaining_records);
    }
    println!("✓ Will process {} remaining records", remaining_records);

    if remaining_records == 0 {
        println!("✅ Migration already completed!");
        MigrationCheckpoint::cleanup(csv_file).await?;
        return Ok(());
    }

    // Process in batches
    let processed_count = Arc::new(AtomicU64::new(0));
    let checkpoint_mutex = Arc::new(Mutex::new(checkpoint));
    
    // Create batches with their starting indices
    let mut batches = Vec::new();
    let mut current_batch = Vec::new();
    let mut batch_start_index = 0;
    
    for (record_index, record) in records {
        if current_batch.is_empty() {
            batch_start_index = record_index;
        }
        
        current_batch.push(ElasticsearchDocument::from(record));
        
        if current_batch.len() >= APP_CONFIG.batch_size {
            batches.push((batch_start_index, current_batch));
            current_batch = Vec::new();
        }
    }
    
    // Add remaining records as final batch
    if !current_batch.is_empty() {
        batches.push((batch_start_index, current_batch));
    }

    println!("✓ Processing {} batches with {} workers...", batches.len(), APP_CONFIG.workers);

    // Set up graceful shutdown handler
    let checkpoint_for_shutdown = checkpoint_mutex.clone();
    let csv_file_for_shutdown = csv_file.to_string();
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        println!("\n🛑 Received shutdown signal, saving checkpoint...");
        let checkpoint = checkpoint_for_shutdown.lock().await;
        if let Err(e) = checkpoint.save(&csv_file_for_shutdown).await {
            eprintln!("Failed to save checkpoint: {}", e);
        }
        std::process::exit(1);
    });

    let results = stream::iter(batches.into_iter().enumerate())
        .map(|(batch_num, (start_index, batch))| {
            let client = client.clone();
            let processed_count = processed_count.clone();
            let checkpoint_mutex = checkpoint_mutex.clone();
            let csv_file = csv_file.to_string();
            
            async move {
                let batch_size = batch.len();
                match bulk_index_documents(&client, &APP_CONFIG.elasticsearch_url, &APP_CONFIG.elasticsearch_index, batch).await {
                    Ok(indexed_count) => {
                        let current = processed_count.fetch_add(indexed_count as u64, Ordering::Relaxed);
                        let new_total = current + indexed_count as u64;
                        
                        // Update checkpoint with completed batch range
                        {
                            let mut checkpoint = checkpoint_mutex.lock().await;
                            checkpoint.add_completed_batch(start_index, batch_size);
                            
                            // Save checkpoint every 10 batches or every 10k records
                            if batch_num % 10 == 0 || new_total % 10000 == 0 {
                                if let Err(e) = checkpoint.save(&csv_file).await {
                                    eprintln!("Failed to save checkpoint: {}", e);
                                }
                            }
                        }
                        
                        if new_total % 10000 == 0 || new_total == remaining_records as u64 {
                            let checkpoint = checkpoint_mutex.lock().await;
                            println!("  Migrated: {}/{} remaining ({:.1}% of total)", 
                                   new_total, remaining_records,
                                   ((checkpoint.processed_records as f64 / total_records as f64) * 100.0));
                        }
                        Ok(indexed_count)
                    }
                    Err(e) => {
                        // Update checkpoint for failed batch
                        {
                            let mut checkpoint = checkpoint_mutex.lock().await;
                            checkpoint.add_failed_batch();
                        }
                        eprintln!("Batch failed: {}", e);
                        Err(e)
                    }
                }
            }
        })
        .buffer_unordered(APP_CONFIG.workers)
        .collect::<Vec<_>>()
        .await;

    let successful = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.iter().filter(|r| r.is_err()).count();
    let final_count = processed_count.load(Ordering::Relaxed);
    let duration = start_time.elapsed();

    // Final checkpoint update
    {
        let checkpoint = checkpoint_mutex.lock().await;
        if checkpoint.is_completed() {
            println!("✅ Migration completed successfully!");
            drop(checkpoint);
            MigrationCheckpoint::cleanup(csv_file).await?;
        } else {
            println!("⚠️  Migration incomplete, checkpoint saved for resume");
            checkpoint.save(csv_file).await?;
        }
    }

    println!("\n📊 Migration Summary:");
    println!("   Duration: {:.2}s", duration.as_secs_f64());
    println!("   Records processed this session: {}", final_count);
    println!("   Successful batches: {}", successful);
    println!("   Failed batches: {}", failed);
    if final_count > 0 {
        println!("   Rate: {:.0} records/sec", final_count as f64 / duration.as_secs_f64());
    }
    
    {
        let checkpoint = checkpoint_mutex.lock().await;
        println!("   Total progress: {:.1}% ({}/{})", 
                 checkpoint.progress_percentage(), 
                 checkpoint.processed_records, 
                 checkpoint.total_records);
    }

    Ok(())
}