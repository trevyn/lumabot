use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub url: Option<String>,
    pub event_uid: String,
    pub api_id: Option<String>,
}

impl Event {
    pub fn new(
        summary: String,
        description: Option<String>,
        location: Option<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        url: Option<String>,
    ) -> Self {
        // Generate a deterministic ID for the event based on its content
        // This will create the same ID for the same event each time
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        summary.hash(&mut hasher);
        start.timestamp().hash(&mut hasher);
        if let Some(desc) = &description {
            desc.hash(&mut hasher);
        }
        if let Some(loc) = &location {
            loc.hash(&mut hasher);
        }
        
        let hash = hasher.finish();
        
        let event_uid = format!("{}-{}-{:x}", 
                               summary.replace(" ", "_"), 
                               start.timestamp(),
                               hash);

        Self {
            summary,
            description,
            location,
            start,
            end,
            url,
            event_uid,
            api_id: None,
        }
    }
    
    // Function removed to eliminate unused code warning
    
    // Create an event with an existing UID and API ID
    pub fn with_uid_and_api_id(
        summary: String,
        description: Option<String>,
        location: Option<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        url: Option<String>,
        event_uid: String,
        api_id: Option<String>,
    ) -> Self {
        Self {
            summary,
            description,
            location,
            start,
            end,
            url,
            event_uid,
            api_id,
        }
    }
    
    /// Utility function to clean any string by removing whitespace and newlines
    pub fn clean_string(input: &str) -> String {
        // Process all types of newlines and escaped sequences
        input.replace("\n", "")
             .replace("\r", "")
             .replace("\t", "")
             .replace("\\n", "") // Handle escaped newlines
             .replace("\\r", "") // Handle escaped carriage returns
             .replace("\\t", "") // Handle escaped tabs
             .trim()
             .to_string()
    }
    
    /// Extract the slug from a Luma URL if available
    pub fn extract_slug(&self) -> Option<String> {
        if let Some(url) = &self.url {
            // Clean the URL first
            let clean_url = Self::clean_string(url);
            
            if clean_url.contains("lu.ma") {
                // Try to extract the slug after the last slash
                if let Some(slug) = clean_url.split('/').last() {
                    if !slug.is_empty() {
                        // Make sure the extracted slug is also cleaned
                        return Some(Self::clean_string(slug));
                    }
                }
                
                // For URLs with /e/ pattern
                if clean_url.contains("/e/") {
                    if let Some(slug) = clean_url.split("/e/").last() {
                        if !slug.is_empty() {
                            // Make sure the extracted slug is also cleaned
                            return Some(Self::clean_string(slug));
                        }
                    }
                }
            }
        }
        None
    }
    
    // Function removed to eliminate unused code warning
    
    // Calculate the duration of the event in minutes
    pub fn duration_minutes(&self) -> i64 {
        self.end.signed_duration_since(self.start).num_minutes()
    }
    
    // Update or set the URL for this event
    #[allow(dead_code)]
    pub fn with_url(mut self, url: Option<String>) -> Self {
        self.url = url;
        self
    }
    
    // Get a default URL based on the event UID
    #[allow(dead_code)]
    pub fn default_url(&self) -> String {
        format!("https://lu.ma/e/{}", self.event_uid)
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.summary == other.summary && self.start == other.start && self.end == other.end
    }
}

impl Eq for Event {}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl Hash for Event {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.summary.hash(state);
        self.start.hash(state);
        self.end.hash(state);
        // We don't hash optional fields as they might be None
    }
}
