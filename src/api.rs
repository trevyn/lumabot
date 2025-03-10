use crate::errors::CalendarError;
use crate::models::Event;
use reqwest::{Client, StatusCode, header};
use serde_json::Value;
use std::time::Duration;
use std::env;

const API_ENDPOINT: &str = "https://api.lu.ma/public/v1/entity/lookup?slug=";
const API_KEY_ENV: &str = "LUMA_API_KEY";

/// API handler for interacting with the Luma API
pub struct LumaApi {
    client: Client,
    api_key: Option<String>, // Luma API key
    #[allow(dead_code)]
    rate_limit_ms: u64, // Rate limiting in milliseconds
}

impl LumaApi {
    /// Creates a new API client
    pub fn new() -> Self {
        // Try to get API key from environment
        let api_key = env::var(API_KEY_ENV).ok();
        
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_key,
            rate_limit_ms: 1000, // Default to 1 request per second
        }
    }
    
    /// Check if an API key is available
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Lookup API ID for an event using its slug
    pub async fn lookup_event_id(&self, slug: &str) -> Result<String, CalendarError> {
        // Check if API key is available
        let api_key = self.api_key.as_ref().ok_or_else(|| {
            CalendarError::ParseError(format!("No API key available. Set {} environment variable", API_KEY_ENV))
        })?;
        
        let url = format!("{}{}", API_ENDPOINT, slug);
        
        let response = self.client
            .get(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| {
                CalendarError::ParseError(format!("API request failed: {}", e))
            })?;
        
        match response.status() {
            StatusCode::OK => {
                let json: Value = response.json().await.map_err(|e| {
                    CalendarError::ParseError(format!("Failed to parse API response: {}", e))
                })?;
                
                // Extract the API ID from the response path: entity.event.api_id
                if let Some(entity) = json.get("entity") {
                    if let Some(event) = entity.get("event") {
                        if let Some(api_id) = event.get("api_id").and_then(|id| id.as_str()) {
                            return Ok(api_id.to_string());
                        }
                    }
                }
                
                // If we reach here, the API ID wasn't found
                Err(CalendarError::ParseError("API ID not found in response".to_string()))
            },
            status => {
                Err(CalendarError::ParseError(format!("API request failed with status: {}", status)))
            }
        }
    }
    
    /// Enrich an event with API data
    pub async fn enrich_event(&self, event: &mut Event) -> Result<(), CalendarError> {
        // If the event already has an API ID, no need to fetch it again
        if event.api_id.is_some() {
            return Ok(());
        }
        
        // Extract slug from URL
        if let Some(slug) = event.extract_slug() {
            // Add a small delay for rate limiting
            tokio::time::sleep(Duration::from_millis(self.rate_limit_ms)).await;
            
            // Lookup the API ID
            let api_id = self.lookup_event_id(&slug).await?;
            
            // Update the event with the API ID
            event.api_id = Some(api_id);
            
            Ok(())
        } else {
            Err(CalendarError::ParseError("Could not extract slug from event URL".to_string()))
        }
    }
    
    /// Batch enrich multiple events with API data
    #[allow(dead_code)]
    pub async fn enrich_events(&self, events: &mut [Event]) -> Vec<Result<(), CalendarError>> {
        let mut results = Vec::with_capacity(events.len());
        
        for event in events {
            let result = self.enrich_event(event).await;
            results.push(result);
            
            // Add a small delay for rate limiting
            tokio::time::sleep(Duration::from_millis(self.rate_limit_ms)).await;
        }
        
        results
    }
}

impl Default for LumaApi {
    fn default() -> Self {
        Self::new()
    }
}