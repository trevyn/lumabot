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
    
    /// Auto-enrich events with API IDs while storing
    #[clap(short = 'e', long)]
    enrich: bool,
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
    
    /// Test API lookup without database operations
    #[clap(name = "lookup")]
    TestLookup {
        /// The slug to lookup (required)
        #[clap(short, long)]
        slug: String,
    },
    
    /// Add an event to your Luma calendar using its API ID
    #[clap(name = "add")]
    AddEvent {
        /// The event API ID to add to your calendar
        #[clap(short, long)]
        event_id: String,
    },
    
    /// Full sync: fetch events, store in database, enrich with API data, and add to your calendar
    #[clap(name = "sync")]
    FullSync {
        /// URL of the calendar to fetch
        #[clap(short, long)]
        url: Option<String>,
        
        /// Limit to only adding events happening within this many days
        #[clap(short, long, default_value_t = 30)]
        days: u32,
        
        /// Skip adding events to your calendar (only store and enrich)
        #[clap(long)]
        skip_add: bool,
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
                        // Thoroughly clean existing URL
                        let clean_url = models::Event::clean_string(url);
                        new_event.url = Some(clean_url);
                    } else {
                        // Add a default URL pattern: https://lu.ma/e/{event_uid}
                        let default_url = format!("https://lu.ma/e/{}", new_event.event_uid);
                        new_event.url = Some(default_url);
                    }
                    new_event
                }).collect();
                
                // Auto-enrich events with API IDs if --enrich is set
                if cli.enrich {
                    println!("{}", "Auto-enriching events with API IDs...".blue());
                    
                    // Set up Tokio runtime for async operations
                    let rt = match Runtime::new() {
                        Ok(runtime) => runtime,
                        Err(e) => {
                            println!("{}", format!("Failed to create async runtime: {}", e).red());
                            return Err(CalendarError::ParseError(format!("Failed to create runtime: {}", e)));
                        }
                    };
                    
                    // Create API client
                    let api_client = LumaApi::new();
                    
                    // Create a vector to hold enriched events
                    let mut enriched_events = Vec::new();
                    let mut success_count = 0;
                    let mut error_count = 0;
                    
                    for event in events_with_clean_urls.iter() {
                        let mut enriched_event = event.clone();
                        
                        // Skip events that already have an API ID
                        if enriched_event.api_id.is_some() {
                            println!("{}", format!("Event already has API ID: {}", enriched_event.summary).yellow());
                            enriched_events.push(enriched_event);
                            continue;
                        }
                        
                        // Extract slug from URL
                        if let Some(slug) = enriched_event.extract_slug() {
                            // The slug is already clean from extract_slug
                            println!("{}", format!("Looking up API ID for event: {} (slug: '{}')", enriched_event.summary, slug).blue());
                            
                            let api_id = rt.block_on(async {
                                api_client.lookup_event_id(&slug).await
                            });
                            
                            match api_id {
                                Ok(id) => {
                                    println!("{}", format!("Found API ID: {}", id).green());
                                    enriched_event.api_id = Some(id);
                                    success_count += 1;
                                },
                                Err(e) => {
                                    // Slug is already clean
                                    println!("{}", format!("API lookup failed for '{}': {}", slug, e).red());
                                    error_count += 1;
                                }
                            }
                            
                            // Add a small delay to respect rate limits
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        } else {
                            println!("{}", format!("Could not extract slug from URL for event: {}", enriched_event.summary).yellow());
                        }
                        
                        enriched_events.push(enriched_event);
                    }
                    
                    println!("{}", format!("API enrichment complete. Success: {}, Errors: {}", success_count, error_count).blue());
                    
                    // Save enriched events with API IDs
                    match db.save_events(&enriched_events) {
                        Ok(count) => println!("{}", format!("Stored {} new or updated events", count).green()),
                        Err(e) => println!("{}", format!("Failed to store events: {}", e).red()),
                    }
                } else {
                    // Save events with clean URLs without enrichment
                    match db.save_events(&events_with_clean_urls) {
                        Ok(count) => println!("{}", format!("Stored {} new events", count).green()),
                        Err(e) => println!("{}", format!("Failed to store events: {}", e).red()),
                    }
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
        Some(Commands::TestLookup { slug }) => {
            // Set up Tokio runtime for async operations
            let rt = Runtime::new().map_err(|e| {
                CalendarError::ParseError(format!("Failed to create runtime: {}", e))
            })?;
            
            // Create API client
            let api_client = LumaApi::new();
            
            println!("{}", format!("Looking up API ID for slug: {}", slug).blue());
            let api_id = rt.block_on(async {
                api_client.lookup_event_id(slug).await
            });
            
            match api_id {
                Ok(id) => {
                    println!("{}", format!("✅ Successfully found API ID: {}", id).green());
                    println!("{}", "This API ID can be used to access additional event details.".yellow());
                },
                Err(e) => {
                    println!("{}", format!("❌ API lookup failed for '{}': {}", slug, e).red());
                },
            }
        }
        Some(Commands::AddEvent { event_id }) => {
            // Set up Tokio runtime for async operations
            let rt = Runtime::new().map_err(|e| {
                CalendarError::ParseError(format!("Failed to create runtime: {}", e))
            })?;
            
            // Create API client
            let api_client = LumaApi::new();
            
            println!("{}", format!("Adding event with API ID: {} to your calendar...", event_id).blue());
            let result = rt.block_on(async {
                api_client.add_event(&event_id).await
            });
            
            match result {
                Ok(response) => {
                    // Extract calendar_event_id from the response if available
                    let calendar_event_id = response.get("calendar_event_id")
                        .and_then(|id| id.as_str())
                        .unwrap_or("unknown");
                    
                    println!("{}", format!("✅ Successfully added event to your calendar").green());
                    println!("{}", format!("Calendar Event ID: {}", calendar_event_id).green());
                    println!("{}", "The event has been added to your Luma calendar.".yellow());
                },
                Err(e) => {
                    println!("{}", format!("❌ Failed to add event: {}", e).red());
                },
            }
        }
        Some(Commands::FullSync { url, days, skip_add }) => {
            println!("{}", "Starting full sync process...".blue().bold());
            
            // 1. Fetch events from calendar URL
            let calendar_url = url.clone().unwrap_or_else(|| cli.url.clone());
            println!("{}", format!("Fetching events from calendar: {}", calendar_url).blue());
            let events = calendar::fetch_and_parse_calendar(&calendar_url)?;
            println!("{}", format!("Fetched {} events", events.len()).green());
            
            // 2. Clean URLs and prepare events for storage
            let events_with_clean_urls: Vec<_> = events.iter().map(|e| {
                let mut new_event = e.clone();
                // Clean the URL if it exists or add a default one
                if let Some(url) = &e.url {
                    // Thoroughly clean existing URL
                    let clean_url = models::Event::clean_string(url);
                    new_event.url = Some(clean_url);
                } else {
                    // Add a default URL pattern: https://lu.ma/e/{event_uid}
                    let default_url = format!("https://lu.ma/e/{}", new_event.event_uid);
                    new_event.url = Some(default_url);
                }
                new_event
            }).collect();
            
            // 3. Store events in database
            match database::connect_db() {
                Ok(db) => {
                    println!("{}", "Storing events in database...".blue());
                    
                    match db.save_events(&events_with_clean_urls) {
                        Ok(count) => println!("{}", format!("Stored {} new or updated events", count).green()),
                        Err(e) => {
                            println!("{}", format!("Failed to store events: {}", e).red());
                            return Err(CalendarError::ParseError(format!("Failed to store events: {}", e)));
                        }
                    }
                    
                    // 4. Enrich events with API data
                    println!("{}", "Enriching events with API data...".blue());
                    
                    // Set up Tokio runtime for async operations
                    let rt = match Runtime::new() {
                        Ok(runtime) => runtime,
                        Err(e) => {
                            println!("{}", format!("Failed to create async runtime: {}", e).red());
                            return Err(CalendarError::ParseError(format!("Failed to create runtime: {}", e)));
                        }
                    };
                    
                    // Create API client
                    let api_client = LumaApi::new();
                    
                    // Fetch all events from the database
                    let mut db_events = match db.get_all_events() {
                        Ok(events) => events,
                        Err(e) => {
                            println!("{}", format!("Failed to fetch events from database: {}", e).red());
                            return Err(CalendarError::ParseError(format!("Failed to fetch events: {}", e)));
                        }
                    };
                    
                    println!("{}", format!("Found {} events in database", db_events.len()).blue());
                    
                    // Process and enrich events
                    let mut success_count = 0;
                    let mut error_count = 0;
                    let mut added_to_calendar_count = 0;
                    let mut add_error_count = 0;
                    
                    // Filter events based on the days parameter
                    let now = chrono::Utc::now();
                    let future_cutoff = now + chrono::Duration::days(*days as i64);
                    
                    // Track future events for possible addition to calendar
                    let mut events_to_add = Vec::new();
                    
                    for event in db_events.iter_mut() {
                        // Skip events that already have an API ID
                        if event.api_id.is_some() {
                            println!("{}", format!("Event already has API ID: {}", event.summary).yellow());
                            
                            // If event is in the future and has API ID, add it to the list of events to potentially add to calendar
                            if event.start > now && event.start < future_cutoff {
                                events_to_add.push(event.clone());
                            }
                            
                            continue;
                        }
                        
                        // Extract slug from URL
                        if let Some(slug) = event.extract_slug() {
                            println!("{}", format!("Looking up API ID for event: {} (slug: '{}')", event.summary, slug).blue());
                            
                            let api_id = rt.block_on(async {
                                api_client.lookup_event_id(&slug).await
                            });
                            
                            match api_id {
                                Ok(id) => {
                                    println!("{}", format!("Found API ID: {}", id).green());
                                    event.api_id = Some(id.clone());
                                    
                                    // Save the updated event
                                    if let Err(e) = db.save_event(event) {
                                        println!("{}", format!("Failed to save event: {}", e).red());
                                        error_count += 1;
                                    } else {
                                        println!("{}", "Event updated successfully".green());
                                        success_count += 1;
                                        
                                        // If event is in the future, add it to the list of events to potentially add to calendar
                                        if event.start > now && event.start < future_cutoff {
                                            events_to_add.push(event.clone());
                                        }
                                    }
                                },
                                Err(e) => {
                                    println!("{}", format!("API lookup failed for '{}': {}", slug, e).red());
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
                    
                    // 5. Add future events to calendar if not skipped
                    if !*skip_add && !events_to_add.is_empty() {
                        println!("{}", format!("Found {} future events to add to your calendar", events_to_add.len()).blue());
                        
                        for event in events_to_add {
                            if let Some(api_id) = &event.api_id {
                                println!("{}", format!("Adding event to calendar: {} (API ID: {})", event.summary, api_id).blue());
                                
                                let result = rt.block_on(async {
                                    api_client.add_event(api_id).await
                                });
                                
                                match result {
                                    Ok(_) => {
                                        println!("{}", format!("✅ Successfully added event to calendar: {}", event.summary).green());
                                        added_to_calendar_count += 1;
                                    },
                                    Err(e) => {
                                        println!("{}", format!("❌ Failed to add event to calendar: {}", e).red());
                                        add_error_count += 1;
                                    }
                                }
                                
                                // Add a small delay to respect rate limits
                                std::thread::sleep(std::time::Duration::from_millis(1000));
                            }
                        }
                        
                        println!("{}", format!("Calendar addition complete. Success: {}, Errors: {}", added_to_calendar_count, add_error_count).blue());
                    } else if *skip_add {
                        println!("{}", "Skipping adding events to calendar as requested".yellow());
                    } else {
                        println!("{}", "No future events found to add to your calendar".yellow());
                    }
                    
                    println!("{}", "Full sync process completed successfully".green().bold());
                }
                Err(e) => {
                    println!("{}", format!("Database connection failed: {}", e).red());
                    return Err(CalendarError::ParseError(format!("Database connection failed: {}", e)));
                }
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
                                    Err(e) => {
                                        // specific_slug needs cleaning since it's user input
                                        let clean_slug = models::Event::clean_string(specific_slug);
                                        println!("{}", format!("API lookup failed for '{}': {}", clean_slug, e).red());
                                    },
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
                                        // Slug is already clean from extract_slug
                                        println!("{}", format!("Looking up API ID for event: {} (slug: '{}')", event.summary, slug).blue());
                                        
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
                                                // Slug is already clean
                                                println!("{}", format!("API lookup failed for '{}': {}", slug, e).red());
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