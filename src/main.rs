mod api;
mod calendar;
mod database;
mod display;
mod errors;
mod models;

use clap::{Parser, Subcommand};
use colored::Colorize;
use errors::CalendarError;
use tokio::runtime::Runtime;
use api::LumaApi;

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
    
    /// Enrich database events with API data
    #[clap(name = "api")]
    EnrichApi {
        /// Limit to a specific number of events
        #[clap(short, long)]
        limit: Option<usize>,
        
        /// The slug to lookup (optional, if not provided, the command will attempt to enrich all events)
        #[clap(short, long)]
        slug: Option<String>,
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
                
                // Save events with clean URLs (using ON CONFLICT DO NOTHING to prevent duplicates)
                match db.save_events(&events_with_clean_urls) {
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
        Some(Commands::EnrichApi { limit, slug }) => {
            // Set up Tokio runtime for async operations
            let rt = Runtime::new().map_err(|e| {
                CalendarError::ParseError(format!("Failed to create runtime: {}", e))
            })?;
            
            // Create API client
            let api_client = LumaApi::new();
            
            // Connect to database
            match database::connect_db() {
                Ok(db) => {
                    // Fetch events from database
                    match db.get_all_events() {
                        Ok(mut db_events) => {
                            println!("{}", format!("Found {} events in database", db_events.len()).blue());
                            
                            // Limit events if specified
                            let events_to_process = match limit {
                                Some(lim) => {
                                    println!("{}", format!("Processing only the first {} events", lim).yellow());
                                    db_events.truncate(*lim);
                                    &mut db_events
                                },
                                None => &mut db_events,
                            };
                            
                            // Process events
                            if let Some(specific_slug) = slug {
                                // Process a single event with the given slug
                                println!("{}", format!("Looking up API ID for slug: {}", specific_slug).yellow());
                                let api_id = rt.block_on(async {
                                    api_client.lookup_event_id(&specific_slug).await
                                });
                                
                                match api_id {
                                    Ok(id) => {
                                        println!("{}", format!("Found API ID: {}", id).green());
                                        // Look for an event with this slug
                                        let mut found = false;
                                        for event in events_to_process.iter_mut() {
                                            if let Some(url) = &event.url {
                                                if url.contains(&*specific_slug) {
                                                    println!("{}", format!("Updating event: {}", event.summary).green());
                                                    event.api_id = Some(id.clone());
                                                    found = true;
                                                    
                                                    // Save the updated event
                                                    if let Err(e) = db.save_event(event) {
                                                        println!("{}", format!("Failed to save event: {}", e).red());
                                                    } else {
                                                        println!("{}", "Event updated successfully".green());
                                                    }
                                                    
                                                    break;
                                                }
                                            }
                                        }
                                        
                                        if !found {
                                            println!("{}", format!("No event found with slug: {}", specific_slug).yellow());
                                        }
                                    },
                                    Err(e) => println!("{}", format!("API lookup failed: {}", e).red()),
                                }
                            } else {
                                // Process all events
                                println!("{}", "Processing all events...".blue());
                                let mut success_count = 0;
                                let mut error_count = 0;
                                
                                for event in events_to_process.iter_mut() {
                                    // Skip events that already have an API ID
                                    if event.api_id.is_some() {
                                        println!("{}", format!("Event already has API ID: {}", event.summary).yellow());
                                        continue;
                                    }
                                    
                                    // Extract slug from URL
                                    if let Some(slug) = event.extract_slug() {
                                        println!("{}", format!("Looking up API ID for event: {} (slug: {})", event.summary, slug).blue());
                                        
                                        let api_id = rt.block_on(async {
                                            api_client.lookup_event_id(&slug).await
                                        });
                                        
                                        match api_id {
                                            Ok(id) => {
                                                println!("{}", format!("Found API ID: {}", id).green());
                                                event.api_id = Some(id);
                                                
                                                // Save the updated event
                                                if let Err(e) = db.save_event(event) {
                                                    println!("{}", format!("Failed to save event: {}", e).red());
                                                    error_count += 1;
                                                } else {
                                                    println!("{}", "Event updated successfully".green());
                                                    success_count += 1;
                                                }
                                            },
                                            Err(e) => {
                                                println!("{}", format!("API lookup failed for {}: {}", slug, e).red());
                                                error_count += 1;
                                            }
                                        }
                                        
                                        // Add a small delay to respect rate limits
                                        std::thread::sleep(std::time::Duration::from_millis(500));
                                    } else {
                                        println!("{}", format!("Could not extract slug from URL for event: {}", event.summary).yellow());
                                    }
                                }
                                
                                println!("{}", format!("API enrichment complete. Success: {}, Errors: {}", success_count, error_count).blue());
                            }
                        }
                        Err(e) => println!("{}", format!("Failed to fetch events from database: {}", e).red()),
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