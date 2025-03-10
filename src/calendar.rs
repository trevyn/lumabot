use crate::errors::CalendarError;
use crate::models::Event;
use chrono::{DateTime, TimeZone, Utc};
use ical::parser::ical::component::IcalCalendar;
use ical::parser::ical::IcalParser;
use reqwest::blocking::Client;
use std::io::BufReader;

/// Fetches and parses a calendar from a URL
pub fn fetch_and_parse_calendar(url: &str) -> Result<Vec<Event>, CalendarError> {
    // Fetch the calendar
    let response = Client::new()
        .get(url)
        .header("User-Agent", "Luma-Calendar-CLI/0.1.0")
        .send()
        .map_err(CalendarError::FetchError)?;

    if !response.status().is_success() {
        return Err(CalendarError::ParseError(
            format!("Failed to fetch calendar: HTTP {}", response.status())
        ));
    }

    // Parse the calendar
    let content = response.text().map_err(CalendarError::FetchError)?;
    let buf_reader = BufReader::new(content.as_bytes());
    let parser = IcalParser::new(buf_reader);

    let mut events = Vec::new();

    for calendar in parser {
        match calendar {
            Ok(cal) => {
                let parsed_events = parse_calendar_events(&cal)?;
                events.extend(parsed_events);
            }
            Err(e) => {
                return Err(CalendarError::ParseError(format!(
                    "Failed to parse calendar: {}",
                    e
                )));
            }
        }
    }

    // Sort events by start time
    events.sort_by(|a, b| a.start.cmp(&b.start));
    Ok(events)
}

/// Parses events from a calendar
fn parse_calendar_events(calendar: &IcalCalendar) -> Result<Vec<Event>, CalendarError> {
    let mut events = Vec::new();

    for component in &calendar.events {
        // Extract event properties
        let summary = component
            .properties
            .iter()
            .find(|p| p.name == "SUMMARY")
            .and_then(|p| p.value.clone())
            .unwrap_or_else(|| "Untitled Event".to_string());

        let description = component
            .properties
            .iter()
            .find(|p| p.name == "DESCRIPTION")
            .and_then(|p| p.value.clone());

        let location = component
            .properties
            .iter()
            .find(|p| p.name == "LOCATION")
            .and_then(|p| p.value.clone());

        let url = component
            .properties
            .iter()
            .find(|p| p.name == "URL")
            .and_then(|p| p.value.clone());

        // Parse start and end times
        let start = component
            .properties
            .iter()
            .find(|p| p.name == "DTSTART")
            .and_then(|p| p.value.clone())
            .ok_or_else(|| {
                CalendarError::ParseError("Event missing DTSTART property".to_string())
            })?;

        let end = component
            .properties
            .iter()
            .find(|p| p.name == "DTEND")
            .and_then(|p| p.value.clone())
            .ok_or_else(|| CalendarError::ParseError("Event missing DTEND property".to_string()))?;

        // Parse dates in format: 20220101T120000Z
        let start_time = parse_ical_datetime(&start)?;
        let end_time = parse_ical_datetime(&end)?;

        // Create a new event
        events.push(Event::new(
            summary,
            description,
            location,
            start_time,
            end_time,
            url,
        ));
    }

    Ok(events)
}

/// Parses an iCal datetime string
fn parse_ical_datetime(dt_str: &str) -> Result<DateTime<Utc>, CalendarError> {
    // Handle different date formats
    let cleaned = dt_str.replace("Z", "").replace("T", "");

    if cleaned.len() != 14 && cleaned.len() != 8 {
        return Err(CalendarError::TimeConversionError(format!(
            "Invalid datetime format: {}",
            dt_str
        )));
    }

    let (year, month, day, hour, minute, second) = if cleaned.len() == 14 {
        // Format: YYYYMMDDHHMMSS
        (
            &cleaned[0..4],
            &cleaned[4..6],
            &cleaned[6..8],
            &cleaned[8..10],
            &cleaned[10..12],
            &cleaned[12..14],
        )
    } else {
        // Format: YYYYMMDD (date only)
        (&cleaned[0..4], &cleaned[4..6], &cleaned[6..8], "00", "00", "00")
    };

    // Parse components
    let year = year.parse::<i32>().map_err(|e| {
        CalendarError::TimeConversionError(format!("Invalid year: {} - {}", year, e))
    })?;
    let month = month.parse::<u32>().map_err(|e| {
        CalendarError::TimeConversionError(format!("Invalid month: {} - {}", month, e))
    })?;
    let day = day.parse::<u32>().map_err(|e| {
        CalendarError::TimeConversionError(format!("Invalid day: {} - {}", day, e))
    })?;
    let hour = hour.parse::<u32>().map_err(|e| {
        CalendarError::TimeConversionError(format!("Invalid hour: {} - {}", hour, e))
    })?;
    let minute = minute.parse::<u32>().map_err(|e| {
        CalendarError::TimeConversionError(format!("Invalid minute: {} - {}", minute, e))
    })?;
    let second = second.parse::<u32>().map_err(|e| {
        CalendarError::TimeConversionError(format!("Invalid second: {} - {}", second, e))
    })?;

    // Create DateTime in UTC
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .ok_or_else(|| {
            CalendarError::TimeConversionError(format!(
                "Invalid date/time combination: {}-{}-{} {}:{}:{}",
                year, month, day, hour, minute, second
            ))
        })
}