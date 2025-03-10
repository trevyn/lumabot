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
    },
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
                match db.save_events(&events) {
                    Ok(count) => println!("{}", format!("Stored {} new events", count).green()),
                    Err(e) => println!("{}", format!("Failed to store events: {}", e).red()),
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
        Some(Commands::Database { all }) => {
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
                                display::display_events(&db_events, cli.limit, cli.verbose);
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
        None => {
            // Default behavior: display all events
            display::display_events(&events, cli.limit, cli.verbose);
        }
    }

    Ok(())
}