use crate::models::Event;
use chrono::{Datelike, Duration, Local, NaiveDate, Utc};
use colored::Colorize;
use std::collections::HashMap;

/// Displays a list of events with a limit
pub fn display_events(events: &[Event], limit: usize, verbose: bool) {
    println!("{}", "Upcoming Events".bright_blue().bold());
    println!("{}", "═".repeat(80).bright_blue());
    
    let limited_events = if limit > 0 && limit < events.len() {
        &events[0..limit]
    } else {
        events
    };
    
    // Convert &[Event] to Vec<&Event> for display_event_list
    let event_refs: Vec<&Event> = limited_events.iter().collect();
    display_event_list(&event_refs, verbose);
    
    if limit > 0 && limit < events.len() {
        println!("\n{}", format!("Showing {}/{} events. Use --limit to see more.", limit, events.len()).yellow());
    }
}

/// Displays today's events
pub fn display_today_events(events: &[Event], verbose: bool) {
    let today = Local::now().date_naive();
    let today_events: Vec<&Event> = events
        .iter()
        .filter(|e| {
            let event_date = e.start.with_timezone(&Local).date_naive();
            event_date == today
        })
        .collect();
    
    println!("{}", format!("Events for Today ({})", today.format("%A, %B %d, %Y")).bright_blue().bold());
    println!("{}", "═".repeat(80).bright_blue());
    
    if today_events.is_empty() {
        println!("{}", "No events scheduled for today.".yellow());
        return;
    }
    
    display_event_list(&today_events, verbose);
}

/// Displays events for the current week
pub fn display_week_events(events: &[Event], verbose: bool) {
    let today = Local::now().date_naive();
    let days_since_monday = today.weekday().num_days_from_monday();
    let monday = today - Duration::days(days_since_monday as i64);
    let sunday = monday + Duration::days(6);
    
    let week_events: Vec<&Event> = events
        .iter()
        .filter(|e| {
            let event_date = e.start.with_timezone(&Local).date_naive();
            event_date >= monday && event_date <= sunday
        })
        .collect();
    
    println!(
        "{}",
        format!(
            "Events for This Week ({} - {})",
            monday.format("%b %d"),
            sunday.format("%b %d, %Y")
        )
        .bright_blue()
        .bold()
    );
    println!("{}", "═".repeat(80).bright_blue());
    
    if week_events.is_empty() {
        println!("{}", "No events scheduled for this week.".yellow());
        return;
    }
    
    // Group events by day
    let mut events_by_day: HashMap<NaiveDate, Vec<&Event>> = HashMap::new();
    
    for event in week_events {
        let date = event.start.with_timezone(&Local).date_naive();
        events_by_day.entry(date).or_default().push(event);
    }
    
    // Display events by day
    let mut dates: Vec<NaiveDate> = events_by_day.keys().cloned().collect();
    dates.sort();
    
    for date in dates {
        let day_events = events_by_day.get(&date).unwrap();
        
        // Format day header
        let day_str = if date == today {
            format!("{} (Today)", date.format("%A, %B %d"))
        } else {
            date.format("%A, %B %d").to_string()
        };
        
        println!("\n{}", day_str.bright_green().bold());
        println!("{}", "-".repeat(day_str.len()).bright_green());
        
        // Use the reference to the Vec directly, as it's already a Vec<&Event>
        display_event_list(&day_events, verbose);
    }
}

/// Displays upcoming events limited by days and count
pub fn display_upcoming_events(events: &[Event], days: u32, limit: usize, verbose: bool) {
    let today = Utc::now();
    let end_date = today + Duration::days(days as i64);
    
    let filtered_events: Vec<&Event> = events
        .iter()
        .filter(|e| e.start >= today && e.start <= end_date)
        .take(if limit > 0 { limit } else { events.len() })
        .collect();
    
    println!(
        "{}",
        format!(
            "Upcoming Events (Next {} Days)",
            days
        )
        .bright_blue()
        .bold()
    );
    println!("{}", "═".repeat(80).bright_blue());
    
    if filtered_events.is_empty() {
        println!("{}", "No upcoming events found in the specified time period.".yellow());
        return;
    }
    
    display_event_list(&filtered_events, verbose);
    
    if filtered_events.len() < events.len() {
        let total_in_range: usize = events
            .iter()
            .filter(|e| e.start >= today && e.start <= end_date)
            .count();
            
        if limit > 0 && limit < total_in_range {
            println!(
                "\n{}",
                format!(
                    "Showing {}/{} events in the next {} days. Use --limit to see more.",
                    filtered_events.len(),
                    total_in_range,
                    days
                )
                .yellow()
            );
        }
    }
}

/// Helper function to display a list of events
fn display_event_list(events: &[&Event], verbose: bool) {
    if events.is_empty() {
        println!("{}", "No events to display.".yellow());
        return;
    }
    
    for event in events {
        let local_start = event.start.with_timezone(&Local);
        let local_end = event.end.with_timezone(&Local);
        
        // Format date and time
        let date_format = local_start.format("%a, %b %d").to_string();
        let time_format = format!(
            "{} - {}",
            local_start.format("%I:%M %p"),
            local_end.format("%I:%M %p")
        );
        
        println!(
            "{} | {} | {}",
            date_format.bright_yellow(),
            time_format.bright_cyan(),
            event.summary.white().bold()
        );
        
        if verbose {
            if let Some(location) = &event.location {
                println!("  {}: {}", "Location".blue(), location);
            }
            
            if let Some(url) = &event.url {
                let clean_url = url.replace("\n", "").replace("\r", "").trim().to_string();
                println!("  {}: {}", "URL".blue(), clean_url);
            }
            
            if let Some(description) = &event.description {
                // Trim and format description
                let desc = description.trim();
                if !desc.is_empty() {
                    println!("  {}: {}", "Description".blue(), desc);
                }
            }
            
            println!("  {}: {} minutes", "Duration".blue(), event.duration_minutes());
            println!();
        }
    }
}