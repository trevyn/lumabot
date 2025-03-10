mod calendar;
mod database;
mod display;
mod errors;
mod models;

use clap::{Parser, Subcommand};
use colored::Colorize;
use errors::CalendarError;

//use tokio::runtime::Runtime;

use std::{process, time::Instant};

// Define the CLI arguments
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,

    /// URL of the calendar to fetch
    #[clap(short, long, default_value = "https://api.lu.ma/ics/get?entity=calendar&id=cal-4dWxlBFjW9Cd6ou")]
    url: String,

    /// Limit the number of events displayed
    #[clap(short, long, default_value_t = 10)]
    limit: usize,

    /// Show detailed information about events
    #[clap(short, long)]
    verbose: bool,

    /// Store events in the database
    #[clap(short, long)]
    store: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show today's events
    Today,

    /// Show events for the current week
    Week,

    /// Show events coming up in the next N days
    #[clap(name = "next")]
    Next {
        /// Number of days to look ahead
        #[clap(default_value_t = 7)]
        days: u32,
    },

    /// Show events from the database
    #[clap(name = "db")]
    Database {
        /// Show all events
        #[clap(long)]
        all: bool,
        
        /// Limit the number of events displayed
        #[clap(short, long, default_value_t = 10)]
        limit: usize,
        
        /// Show detailed information about events
        #[clap(short, long)]
        verbose: bool,
    },
    
    /// Clear all events from the database
    #[clap(name = "clear")]
    ClearDb,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Measure execution time
    let start_time = Instant::now();

    match run(cli) {
        Ok(_) => {
            let duration = start_time.elapsed();
            println!("\n{}", format!("Execution time: {:.2?}", duration).dimmed());
            Ok(())
        }
        Err(e) => {
            eprintln!("{}: {}", "Error".bright_red().bold(), e);
            process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<(), CalendarError> {
    let events = calendar::fetch_and_parse_calendar(&cli.url)?;
    
    // Handle database operations if --store is set
    if cli.store {
        match database::connect_db() {
            Ok(db) => {
                println!("{}", "Storing events in database...".blue());
                
                // Debug: Count events with URLs
                let events_with_urls = events.iter().filter(|e| e.url.is_some()).count();
                println!("{}", format!("Found {} events with URLs out of {}", events_with_urls, events.len()).yellow());
                
                // Add default URL to events that don't have one - Luma base URL and clean existing URLs
                let events_with_clean_urls: Vec<_> = events.iter().map(|e| {
                    let mut new_event = e.clone();
                    // Clean the URL if it exists or add a default one
                    if let Some(url) = &e.url {
                        // Clean existing URL
                        let clean_url = url.replace("\n", "").replace("\r", "").trim().to_string();
                        new_event.url = Some(clean_url);
                    } else {
                        // Add a default URL pattern: https://lu.ma/e/{event_uid}
                        let default_url = format!("https://lu.ma/e/{}", new_event.event_uid);
                        new_event.url = Some(default_url);
                    }
                    new_event
                }).collect();
                
                // First, clear the database to ensure we have clean data
                match db.clear_all_events() {
                    Ok(_) => {
                        // Then save the events with clean URLs
                        match db.save_events(&events_with_clean_urls) {
                            Ok(count) => println!("{}", format!("Stored {} new events", count).green()),
                            Err(e) => println!("{}", format!("Failed to store events: {}", e).red()),
                        }
                    },
                    Err(e) => println!("{}", format!("Failed to clear database: {}", e).red()),
                }
            }
            Err(e) => println!("{}", format!("Database connection failed: {}", e).red()),
        }
    }

    // Handle subcommands or default display
    match &cli.command {
        Some(Commands::Today) => {
            display::display_today_events(&events, cli.verbose);
        }
        Some(Commands::Week) => {
            display::display_week_events(&events, cli.verbose);
        }
        Some(Commands::Next { days }) => {
            display::display_upcoming_events(&events, *days, cli.limit, cli.verbose);
        }
        Some(Commands::Database { all, limit, verbose }) => {
            match database::connect_db() {
                Ok(db) => {
                    if *all {
                        match db.get_all_events() {
                            Ok(db_events) => {
                                println!(
                                    "{}",
                                    format!("Displaying all {} events from database", db_events.len())
                                        .blue()
                                );
                                display::display_events(&db_events, *limit, *verbose);
                            }
                            Err(e) => println!("{}", format!("Failed to fetch events: {}", e).red()),
                        }
                    } else {
                        match db.get_event_count() {
                            Ok(count) => {
                                println!(
                                    "{}",
                                    format!("Database contains {} events", count).blue()
                                );
                            }
                            Err(e) => {
                                println!("{}", format!("Failed to count events: {}", e).red())
                            }
                        }
                    }
                }
                Err(e) => println!("{}", format!("Database connection failed: {}", e).red()),
            }
        }
        Some(Commands::ClearDb) => {
            match database::connect_db() {
                Ok(db) => {
                    match db.clear_all_events() {
                        Ok(count) => {
                            println!("{}", format!("Successfully cleared {} events from database", count).green());
                        }
                        Err(e) => {
                            println!("{}", format!("Failed to clear database: {}", e).red());
                        }
                    }
                }
                Err(e) => println!("{}", format!("Database connection failed: {}", e).red()),
            }
        }
        None => {
            // Default behavior: display all events
            display::display_events(&events, cli.limit, cli.verbose);
        }
    }

    Ok(())
}