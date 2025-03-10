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