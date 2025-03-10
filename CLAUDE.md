# Luma Calendar CLI Development Guide

## Build & Run Commands
```bash
cargo build --release     # Build for release
cargo run                 # Run with default settings
cargo run -- --help       # Show help
cargo run -- today        # Show today's events
cargo run -- week         # Show events for current week
cargo run -- next 14      # Show events for next 14 days
cargo run -- db --all     # Show all events from database
cargo run -- api          # Enrich database events with API data
cargo run -- lookup --slug <SLUG>  # Test API lookup for a specific event
cargo check               # Check for errors without building
cargo clippy              # Lint code
cargo test                # Run all tests
cargo test module::test   # Run a specific test
```

## Code Style Guidelines
- **Modules**: Organize code into domain-specific modules (calendar, database, display, models, api)
- **Errors**: Use thiserror for error handling with proper error propagation (CalendarError, DatabaseError)
- **CLI Commands**: Structure using clap with Subcommand pattern; document with /// comments
- **Naming**: Use snake_case for functions/variables, CamelCase for types/structs/enums
- **Comments**: Include doc comments (///) for public functions and CLI arguments
- **Imports**: Group by (1) std, (2) external crates, (3) internal modules
- **Types**: Specify return types explicitly; use `Result<T, E>` for fallible functions
- **Database**: Use Postgres types; prefer strong typing for database fields
- **API**: Follow REST API best practices; handle rate limiting with sleep between requests
- **Formatting**: Follow standard Rust formatting (rustfmt)
- **Date Formats**: Always include year in date display formats (%Y)
- **Command Pattern**: Follow UNIX-style CLI commands with options and subcommands
- **Error Handling**: Use ? operator for immediate returns; pattern match for complex cases