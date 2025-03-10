use crate::errors::{CalendarError, DatabaseError};
use crate::models::Event;
use chrono::{DateTime, Utc};
use std::env;
use tokio::runtime::Runtime;
use deadpool_postgres::{Config, Pool, PoolConfig, Runtime as PoolRuntime, Client as PoolClient};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;

/// Database handler for connecting to PostgreSQL
pub struct Database {
    pool: Pool,
    #[allow(dead_code)]
    client: Option<PoolClient>,
}

impl Database {
    /// Creates a new Database instance
    pub fn new() -> Result<Self, DatabaseError> {
        // Get database connection info from environment variables
        let host = env::var("PGHOST").map_err(|_| {
            DatabaseError::EnvError("PGHOST environment variable not set".to_string())
        })?;
        
        let user = env::var("PGUSER").map_err(|_| {
            DatabaseError::EnvError("PGUSER environment variable not set".to_string())
        })?;
        
        let password = env::var("PGPASSWORD").map_err(|_| {
            DatabaseError::EnvError("PGPASSWORD environment variable not set".to_string())
        })?;
        
        let dbname = env::var("PGDATABASE").map_err(|_| {
            DatabaseError::EnvError("PGDATABASE environment variable not set".to_string())
        })?;
        
        let port = env::var("PGPORT")
            .map_err(|_| DatabaseError::EnvError("PGPORT environment variable not set".to_string()))?
            .parse::<u16>()
            .map_err(|e| DatabaseError::EnvError(format!("Invalid PGPORT: {}", e)))?;

        // Create a configuration for the connection pool
        let mut cfg = Config::new();
        cfg.host = Some(host);
        cfg.user = Some(user);
        cfg.password = Some(password);
        cfg.dbname = Some(dbname);
        cfg.port = Some(port);
        cfg.ssl_mode = Some(deadpool_postgres::SslMode::Require);

        // Configure pool settings
        cfg.pool = Some(PoolConfig::new(5)); // Max 5 connections in the pool

        // Create a runtime for async database operations
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        // Set up TLS connector for secure connection
        let tls_connector = rt.block_on(async {
            let tls_connector = TlsConnector::builder()
                .danger_accept_invalid_certs(true) // Allow self-signed certificates for development
                .build()
                .map_err(|e| DatabaseError::ConnectionError(format!("TLS error: {}", e)))?;
            
            Ok::<_, DatabaseError>(MakeTlsConnector::new(tls_connector))
        })?;

        // Create the connection pool
        let pool = rt.block_on(async {
            cfg.create_pool(Some(PoolRuntime::Tokio1), tls_connector)
                .map_err(|e| DatabaseError::ConnectionError(format!("Failed to create connection pool: {}", e)))
        })?;

        // Get a client from the pool to initialize the database
        let client = rt.block_on(async {
            pool.get().await
                .map_err(|e| DatabaseError::ConnectionError(format!("Failed to get connection from pool: {}", e)))
        })?;

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
                    event_uid TEXT NOT NULL UNIQUE,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
                )",
                &[],
            ).await
        }).map_err(DatabaseError::QueryError)?;

        Ok(Self { 
            pool,
            client: Some(client),
        })
    }

    /// Saves an event to the database
    #[allow(dead_code)]
    pub fn save_event(&self, event: &Event) -> Result<(), DatabaseError> {
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        // Use client if available, otherwise get a new connection from pool
        if let Some(ref client) = self.client {
            // If we have a client already, use it
            // Clean URL if it exists - force cleaned strings to avoid any newlines
            let clean_url = match &event.url {
                Some(url) => {
                    let cleaned = url.replace("\n", "").replace("\r", "").trim().to_string();
                    Some(cleaned)
                },
                None => None
            };
            
            rt.block_on(async {
                client
                    .execute(
                        "INSERT INTO events (summary, description, location, start_time, end_time, url, event_uid)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)
                         ON CONFLICT (event_uid) DO NOTHING",
                        &[
                            &event.summary,
                            &event.description,
                            &event.location,
                            &event.start,
                            &event.end,
                            &clean_url,
                            &event.event_uid,
                        ],
                    )
                    .await
            })
            .map_err(DatabaseError::QueryError)?;
        } else {
            // Get a fresh connection from the pool
            rt.block_on(async {
                let client = self.pool.get().await
                    .map_err(|e| DatabaseError::ConnectionError(format!("Failed to get connection from pool: {}", e)))?;
                
                // Clean URL if it exists - force cleaned strings to avoid any newlines
                let clean_url = match &event.url {
                    Some(url) => {
                        let cleaned = url.replace("\n", "").replace("\r", "").trim().to_string();
                        Some(cleaned)
                    },
                    None => None
                };
                
                client
                    .execute(
                        "INSERT INTO events (summary, description, location, start_time, end_time, url, event_uid)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)
                         ON CONFLICT (event_uid) DO NOTHING",
                        &[
                            &event.summary,
                            &event.description,
                            &event.location,
                            &event.start,
                            &event.end,
                            &clean_url,
                            &event.event_uid,
                        ],
                    )
                    .await
                    .map_err(DatabaseError::QueryError)
            })?;
        }

        Ok(())
    }

    /// Saves a list of events to the database
    pub fn save_events(&self, events: &[Event]) -> Result<usize, DatabaseError> {
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        // Get a fresh connection from the pool for the batch operation
        let client = rt.block_on(async {
            self.pool.get().await
                .map_err(|e| DatabaseError::ConnectionError(format!("Failed to get connection from pool: {}", e)))
        })?;

        let mut saved_count = 0;
        for event in events {
            // Clean URL if it exists - force cleaned strings to avoid any newlines
            let clean_url = match &event.url {
                Some(url) => {
                    let cleaned = url.replace("\n", "").replace("\r", "").trim().to_string();
                    Some(cleaned)
                },
                None => None
            };
            
            let result = rt.block_on(async {
                client
                    .execute(
                        "INSERT INTO events (summary, description, location, start_time, end_time, url, event_uid)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)
                         ON CONFLICT (event_uid) DO NOTHING",
                        &[
                            &event.summary,
                            &event.description,
                            &event.location,
                            &event.start,
                            &event.end,
                            &clean_url,
                            &event.event_uid,
                        ],
                    )
                    .await
            });

            match result {
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

        // Get a fresh connection from the pool
        let client = rt.block_on(async {
            self.pool.get().await
                .map_err(|e| DatabaseError::ConnectionError(format!("Failed to get connection from pool: {}", e)))
        })?;

        let rows = rt.block_on(async {
            client
                .query(
                    "SELECT summary, description, location, start_time, end_time, url, event_uid
                     FROM events
                     ORDER BY start_time",
                    &[],
                )
                .await
        })
        .map_err(DatabaseError::QueryError)?;

        let mut events = Vec::new();
        for row in rows {
            // Get the URL and clean it if needed - ensure all newlines and carriage returns are removed
            let url: Option<String> = row.get("url");
            let cleaned_url = url.map(|u| u.replace("\n", "").replace("\r", "").trim().to_string());
            
            events.push(Event::with_uid(
                row.get("summary"),
                row.get("description"),
                row.get("location"),
                row.get("start_time"),
                row.get("end_time"),
                cleaned_url,
                row.get("event_uid"),
            ));
        }

        Ok(events)
    }

    /// Retrieves events in a date range
    #[allow(dead_code)]
    pub fn get_events_in_range(
        &self,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
    ) -> Result<Vec<Event>, DatabaseError> {
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        // Get a fresh connection from the pool
        let client = rt.block_on(async {
            self.pool.get().await
                .map_err(|e| DatabaseError::ConnectionError(format!("Failed to get connection from pool: {}", e)))
        })?;

        let rows = rt.block_on(async {
            client
                .query(
                    "SELECT summary, description, location, start_time, end_time, url, event_uid
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
            // Get the URL and clean it if needed - ensure all newlines and carriage returns are removed
            let url: Option<String> = row.get("url");
            let cleaned_url = url.map(|u| u.replace("\n", "").replace("\r", "").trim().to_string());
            
            events.push(Event::with_uid(
                row.get("summary"),
                row.get("description"),
                row.get("location"),
                row.get("start_time"),
                row.get("end_time"),
                cleaned_url,
                row.get("event_uid"),
            ));
        }

        Ok(events)
    }

    /// Gets the count of events in the database
    pub fn get_event_count(&self) -> Result<i64, DatabaseError> {
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        // Get a fresh connection from the pool
        let client = rt.block_on(async {
            self.pool.get().await
                .map_err(|e| DatabaseError::ConnectionError(format!("Failed to get connection from pool: {}", e)))
        })?;

        let row = rt.block_on(async {
            client
                .query_one("SELECT COUNT(*) FROM events", &[])
                .await
        })
        .map_err(DatabaseError::QueryError)?;

        Ok(row.get::<_, i64>(0))
    }
    
    /// Clears all events from the database
    pub fn clear_all_events(&self) -> Result<u64, DatabaseError> {
        let rt = Runtime::new().map_err(|e| {
            DatabaseError::ConnectionError(format!("Failed to create runtime: {}", e))
        })?;

        // Get a fresh connection from the pool
        let client = rt.block_on(async {
            self.pool.get().await
                .map_err(|e| DatabaseError::ConnectionError(format!("Failed to get connection from pool: {}", e)))
        })?;

        let result = rt.block_on(async {
            client
                .execute("DELETE FROM events", &[])
                .await
        })
        .map_err(DatabaseError::QueryError)?;

        Ok(result)
    }
}

/// Helper function to connect to the database
pub fn connect_db() -> Result<Database, CalendarError> {
    Database::new().map_err(|e| {
        CalendarError::ParseError(format!("Database connection error: {}", e))
    })
}