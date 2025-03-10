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
        Self {
            summary,
            description,
            location,
            start,
            end,
            url,
        }
    }
    
    // Calculate the duration of the event in minutes
    pub fn duration_minutes(&self) -> i64 {
        self.end.signed_duration_since(self.start).num_minutes()
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
