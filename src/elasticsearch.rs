use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

use crate::models::{BulkIndexAction, BulkIndexMetadata, ElasticsearchDocument};

pub async fn bulk_index_documents(
    client: &Client,
    elasticsearch_url: &str,
    index_name: &str,
    documents: Vec<ElasticsearchDocument>,
) -> Result<usize> {
    if documents.is_empty() {
        return Ok(0);
    }

    let mut bulk_body = String::new();
    let mut valid_docs = 0;

    for doc in documents {
        if let Some(token_id) = &doc.token_id {
            let doc_id = token_id.to_string();
            
            // Add index action
            let index_action = BulkIndexAction {
                index: BulkIndexMetadata { id: doc_id },
            };
            bulk_body.push_str(&serde_json::to_string(&index_action)?);
            bulk_body.push('\n');
            
            // Add document
            bulk_body.push_str(&serde_json::to_string(&doc)?);
            bulk_body.push('\n');
            
            valid_docs += 1;
        }
    }

    if valid_docs == 0 {
        return Ok(0);
    }

    let url = format!("{}/{}/_bulk", elasticsearch_url, index_name);
    let response = client
        .post(&url)
        .header("Content-Type", "application/x-ndjson")
        .body(bulk_body)
        .send()
        .await
        .context("Failed to send bulk request")?;

    if response.status().is_success() {
        let result: Value = response.json().await.context("Failed to parse response")?;
        
        if let Some(items) = result["items"].as_array() {
            let errors: Vec<_> = items
                .iter()
                .filter_map(|item| item["index"]["error"].as_object())
                .collect();
            
            if !errors.is_empty() {
                eprintln!("Bulk indexing had {} errors out of {} documents", errors.len(), valid_docs);
            }
        }
        
        Ok(valid_docs)
    } else {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        eprintln!("Bulk indexing failed: HTTP {} - {}", status, error_text);
        Err(anyhow::anyhow!("Bulk indexing failed: HTTP {}", status))
    }
}
