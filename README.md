# Luma Calendar CLI

A Rust-based CLI application that fetches, processes, and displays Luma calendar events in a clean, Unix-style terminal interface.

## Features

- Fetch calendar events from a Luma API URL
- Display events in a formatted, colorful terminal interface
- Filter events by day, week, or custom date range
- Store events in a PostgreSQL database for offline access
- Command-line arguments for customization

## Prerequisites

- Rust (latest stable version)
- PostgreSQL database
- OpenSSL

## Installation

1. Clone the repository
2. Set up environment variables for database connection:
   - `DATABASE_URL` - PostgreSQL connection string (e.g., `postgres://username:password@localhost:5432/dbname`)
   - Alternatively, you can set individual variables:
     - `PGUSER` - PostgreSQL username
     - `PGPASSWORD` - PostgreSQL password
     - `PGHOST` - PostgreSQL host (default: localhost)
     - `PGPORT` - PostgreSQL port (default: 5432)
     - `PGDATABASE` - PostgreSQL database name

3. Build the project:
   ```
   cargo build --release
   ```

## Usage

Basic usage:

```
luma-calendar-cli [OPTIONS] [COMMAND]
```

### Options

- `-u, --url <URL>` - Calendar URL (default: Luma calendar URL)
- `-l, --limit <LIMIT>` - Limit number of events displayed (default: 10)
- `-v, --verbose` - Show detailed information for each event
- `-s, --store` - Store events in the database

### Commands

- `today` - Show today's events
- `week` - Show events for the current week
- `next [DAYS]` - Show events for the next N days (default: 7)
- `db` - Database options:
  - `--all` - Show all events from the database

### Examples

Display the next 10 events from the default Luma calendar:
```
luma-calendar-cli
```

Show today's events with detailed information:
```
luma-calendar-cli today --verbose
```

Show events for the next 14 days, limiting to 5 events, and store them in the database:
```
luma-calendar-cli next 14 --limit 5 --store
```

Display events for the current week:
```
luma-calendar-cli week
```

Show all events stored in the database:
```
luma-calendar-cli db --all
```

## Development

The project is organized into several modules:

- `main.rs` - CLI interface and command processing
- `calendar.rs` - Calendar fetching and parsing
- `display.rs` - Formatting and displaying events
- `database.rs` - Database operations
- `models.rs` - Data structures and models
- `errors.rs` - Error handling

## License

MIT