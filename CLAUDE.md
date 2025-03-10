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
cargo check               # Check for errors without building
cargo clippy              # Lint code
cargo test                # Run all tests
cargo test module::test   # Run a specific test
```

## Code Style Guidelines
- **Modules**: Organize code into domain-specific modules (calendar, database, display, models)
- **Errors**: Use thiserror for error handling with proper error propagation
- **CLI Commands**: Structure using clap with Subcommand pattern
- **Naming**: Use snake_case for functions/variables, CamelCase for types/structs/enums
- **Comments**: Include doc comments (///) for public functions and CLI arguments
- **Imports**: Group by std, external crates, then internal modules
- **Types**: Specify return types explicitly; use `Result<T, E>` for fallible functions
- **Database**: Use Postgres types; prefer strong typing for database fields
- **Formatting**: Follow standard Rust formatting (rustfmt)
- **Date Formats**: Always include year in date display formats (%Y)