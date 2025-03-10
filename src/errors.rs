use thiserror::Error;

#[derive(Error, Debug)]
pub enum CalendarError {
    #[error("Failed to fetch calendar: {0}")]
    FetchError(#[from] reqwest::Error),
    
    #[error("Failed to parse calendar: {0}")]
    ParseError(String),
    
    #[error("Failed to convert time: {0}")]
    TimeConversionError(String),
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] tokio_postgres::Error),
    
    #[error("Error loading environment variable: {0}")]
    EnvError(String),
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Query error: {0}")]
    QueryError(#[from] tokio_postgres::Error),
    
    #[error("Error loading environment variable: {0}")]
    EnvError(String),
    
    #[error("Data conversion error: {0}")]
    DataConversionError(String),
}