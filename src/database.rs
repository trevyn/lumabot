use crate::errors::{CalendarError, DatabaseError};
use crate::models::Event;
use chrono::{DateTime, Utc};
use std::env;
use tokio::runtime::Runtime;
use tokio_postgres::{Client, NoTls};

/// Database handler for connecting to PostgreSQL
pub struct Database {
    client: Client,
}

impl Database {
    /// Creates a new Database instance
    pub fn new() -> Result<Self, DatabaseError> {
        let database_url = env::var("DATABASE_URL").map_err(|_| {
            DatabaseError::EnvError("DATABASE_URL environment variable not set".to_string())
        })?;

        // Create a runtime for async database operations
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        // Connect to the database
        let (client, connection) = rt.block_on(async {
            tokio_postgres::connect(&database_url, NoTls).await
        }).map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to connect to database: {}", e))
        })?;

        // Spawn a task to drive the connection
        rt.spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Database connection error: {}", e);
            }
        });

        // Create tables if they don't exist
        rt.block_on(async {
            client.execute(
                "CREATE TABLE IF NOT EXISTS events (
                    id SERIAL PRIMARY KEY,
                    summary TEXT NOT NULL,
                    description TEXT,
                    location TEXT,
                    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
                    end_time TIMESTAMP WITH TIME ZONE NOT NULL,
                    url TEXT,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                    UNIQUE(summary, start_time, end_time)
                )",
                &[],
            ).await
        }).map_err(DatabaseError::QueryError)?;

        Ok(Self { client })
    }

    /// Saves an event to the database
    pub fn save_event(&self, event: &Event) -> Result<(), DatabaseError> {
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        rt.block_on(async {
            self.client
                .execute(
                    "INSERT INTO events (summary, description, location, start_time, end_time, url)
                     VALUES ($1, $2, $3, $4, $5, $6)
                     ON CONFLICT (summary, start_time, end_time) DO NOTHING",
                    &[
                        &event.summary,
                        &event.description,
                        &event.location,
                        &event.start,
                        &event.end,
                        &event.url,
                    ],
                )
                .await
        })
        .map_err(DatabaseError::QueryError)?;

        Ok(())
    }

    /// Saves a list of events to the database
    pub fn save_events(&self, events: &[Event]) -> Result<usize, DatabaseError> {
        let mut saved_count = 0;
        for event in events {
            match self.save_event(event) {
                Ok(_) => saved_count += 1,
                Err(e) => eprintln!("Failed to save event: {}", e),
            }
        }
        Ok(saved_count)
    }

    /// Retrieves all events from the database
    pub fn get_all_events(&self) -> Result<Vec<Event>, DatabaseError> {
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        let rows = rt.block_on(async {
            self.client
                .query(
                    "SELECT summary, description, location, start_time, end_time, url
                     FROM events
                     ORDER BY start_time",
                    &[],
                )
                .await
        })
        .map_err(DatabaseError::QueryError)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(Event::new(
                row.get("summary"),
                row.get("description"),
                row.get("location"),
                row.get("start_time"),
                row.get("end_time"),
                row.get("url"),
            ));
        }

        Ok(events)
    }

    /// Retrieves events in a date range
    pub fn get_events_in_range(
        &self,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
    ) -> Result<Vec<Event>, DatabaseError> {
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        let rows = rt.block_on(async {
            self.client
                .query(
                    "SELECT summary, description, location, start_time, end_time, url
                     FROM events
                     WHERE start_time >= $1 AND start_time <= $2
                     ORDER BY start_time",
                    &[&start_date, &end_date],
                )
                .await
        })
        .map_err(DatabaseError::QueryError)?;

        let mut events = Vec::new();
        for row in rows {
            events.push(Event::new(
                row.get("summary"),
                row.get("description"),
                row.get("location"),
                row.get("start_time"),
                row.get("end_time"),
                row.get("url"),
            ));
        }

        Ok(events)
    }

    /// Gets the count of events in the database
    pub fn get_event_count(&self) -> Result<i64, DatabaseError> {
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        let row = rt.block_on(async {
            self.client
                .query_one("SELECT COUNT(*) FROM events", &[])
                .await
        })
        .map_err(DatabaseError::QueryError)?;

        Ok(row.get::<_, i64>(0))
    }
}

/// Helper function to connect to the database
pub fn connect_db() -> Result<Database, CalendarError> {
    Database::new().map_err(|e| {
        CalendarError::ParseError(format!("Database connection error: {}", e))
    })
}