use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MigrationCheckpoint {
    pub csv_file_path: String,
    pub total_records: usize,
    pub processed_records: usize,
    pub successful_batches: usize,
    pub failed_batches: usize,
    pub completed_batch_ranges: Vec<(usize, usize)>, // (start_index, end_index) pairs
    pub start_time: u64, // Unix timestamp
}

impl MigrationCheckpoint {
    pub fn new(csv_file_path: String, total_records: usize) -> Self {
        Self {
            csv_file_path,
            total_records,
            processed_records: 0,
            successful_batches: 0,
            failed_batches: 0,
            completed_batch_ranges: Vec::new(),
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn get_safe_resume_point(&self) -> usize {
        if self.completed_batch_ranges.is_empty() {
            return 0;
        }

        // Find the highest continuous range from the beginning
        let mut safe_point = 0;
        let mut sorted_ranges = self.completed_batch_ranges.clone();
        sorted_ranges.sort_by_key(|r| r.0);

        for (start, end) in sorted_ranges {
            if start == safe_point {
                safe_point = end;
            } else if start > safe_point {
                // Gap found, can't safely skip beyond this point
                break;
            }
        }

        safe_point
    }

    pub fn add_completed_batch(&mut self, start_index: usize, batch_size: usize) {
        let end_index = start_index + batch_size;
        self.completed_batch_ranges.push((start_index, end_index));
        self.processed_records += batch_size;
        self.successful_batches += 1;
    }

    pub fn checkpoint_file_path(csv_file: &str) -> String {
        format!("{}.checkpoint", csv_file)
    }

    pub async fn save(&self, csv_file: &str) -> Result<()> {
        let checkpoint_path = Self::checkpoint_file_path(csv_file);
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&checkpoint_path, json).await?;
        println!("💾 Checkpoint saved: {} records processed", self.processed_records);
        Ok(())
    }

    pub async fn load(csv_file: &str) -> Result<Option<Self>> {
        let checkpoint_path = Self::checkpoint_file_path(csv_file);
        
        if !Path::new(&checkpoint_path).exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&checkpoint_path).await?;
        let checkpoint: Self = serde_json::from_str(&content)?;
        
        // Verify the checkpoint is for the same CSV file
        if checkpoint.csv_file_path != csv_file {
            println!("⚠️  Checkpoint is for different CSV file, ignoring");
            return Ok(None);
        }

        Ok(Some(checkpoint))
    }

    pub async fn cleanup(csv_file: &str) -> Result<()> {
        let checkpoint_path = Self::checkpoint_file_path(csv_file);
        if Path::new(&checkpoint_path).exists() {
            fs::remove_file(&checkpoint_path).await?;
            println!("🗑️  Checkpoint file removed");
        }
        Ok(())
    }

    pub fn add_failed_batch(&mut self) {
        self.failed_batches += 1;
    }

    pub fn is_completed(&self) -> bool {
        self.processed_records >= self.total_records
    }

    pub fn progress_percentage(&self) -> f64 {
        if self.total_records == 0 {
            return 0.0;
        }
        (self.processed_records as f64 / self.total_records as f64) * 100.0
    }
}
