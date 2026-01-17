use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::digit1,
    combinator::map_res,
    sequence::{delimited, separated_pair},
    IResult, Parser,
};
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
struct Timestamp {
    hours: u32,
    minutes: u32,
    seconds: u32,
}

#[derive(Debug, Serialize)]
pub struct LineResult {
    pub id: usize,
    pub input: String,
    pub result: DurationResult,
}

#[derive(Debug, Serialize)]
pub struct DurationResult {
    pub seconds: u32,
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct ParseOutput {
    pub lines: Vec<LineResult>,
    pub total: DurationResult,
}

impl Timestamp {
    fn to_seconds(&self) -> u32 {
        self.hours * 3600 + self.minutes * 60 + self.seconds
    }
}

fn parse_number(input: &str) -> IResult<&str, u32> {
    map_res(digit1, |s: &str| s.parse::<u32>()).parse(input)
}

// Parse H:MM:SS format
fn parse_hms(input: &str) -> IResult<&str, Timestamp> {
    let (input, (hours, _, minutes, _, seconds)) = (
        parse_number,
        tag(":"),
        parse_number,
        tag(":"),
        parse_number,
    ).parse(input)?;
    Ok((input, Timestamp { hours, minutes, seconds }))
}

// Parse M:SS format
fn parse_ms(input: &str) -> IResult<&str, Timestamp> {
    let (input, (minutes, seconds)) = separated_pair(parse_number, tag(":"), parse_number).parse(input)?;
    Ok((input, Timestamp { hours: 0, minutes, seconds }))
}

// Try H:MM:SS first, then fall back to M:SS
fn parse_timestamp(input: &str) -> IResult<&str, Timestamp> {
    alt((parse_hms, parse_ms)).parse(input)
}

// Parse any dash type: hyphen (-), en-dash (–), or em-dash (—)
fn parse_dash(input: &str) -> IResult<&str, &str> {
    alt((tag("-"), tag("–"), tag("—"))).parse(input)
}

enum RangeError {
    None,
    EndBeforeStart,
    InvalidSeconds(u32),
    InvalidMinutes(u32),
}

struct RangeResult {
    duration: u32,
    error: RangeError,
}

fn parse_range(input: &str) -> IResult<&str, RangeResult> {
    let (input, (start, end)) = delimited(
        tag("("),
        separated_pair(parse_timestamp, parse_dash, parse_timestamp),
        tag(")"),
    ).parse(input)?;
    
    // Validate minutes <= 59 (only hours can be unlimited)
    if start.minutes > 59 {
        return Ok((input, RangeResult { duration: 0, error: RangeError::InvalidMinutes(start.minutes) }));
    }
    if end.minutes > 59 {
        return Ok((input, RangeResult { duration: 0, error: RangeError::InvalidMinutes(end.minutes) }));
    }
    
    // Validate seconds <= 59
    if start.seconds > 59 {
        return Ok((input, RangeResult { duration: 0, error: RangeError::InvalidSeconds(start.seconds) }));
    }
    if end.seconds > 59 {
        return Ok((input, RangeResult { duration: 0, error: RangeError::InvalidSeconds(end.seconds) }));
    }
    
    let start_secs = start.to_seconds();
    let end_secs = end.to_seconds();
    
    if end_secs < start_secs {
        Ok((input, RangeResult { duration: 0, error: RangeError::EndBeforeStart }))
    } else {
        Ok((input, RangeResult { duration: end_secs - start_secs, error: RangeError::None }))
    }
}

// Represents a parsed timestamp range with its position and text in the input
struct ParsedRange {
    start_pos: usize,
    end_pos: usize,
    text: String,
    duration: u32,
    error: RangeError,
}

fn find_all_ranges(input: &str) -> Result<Vec<ParsedRange>, String> {
    let mut ranges = Vec::new();
    let mut search_start = 0;
    
    // Pattern to detect things that look like timestamp ranges (includes unicode dashes)
    let timestamp_pattern = Regex::new(r"\([^)]*:[^)]*[-–—][^)]*:[^)]*\)").unwrap();

    while let Some(paren_pos) = input[search_start..].find('(') {
        let abs_start = search_start + paren_pos;
        let remaining = &input[abs_start..];
        
        if let Ok((rest, result)) = parse_range(remaining) {
            let range_len = remaining.len() - rest.len();
            let text = input[abs_start..abs_start + range_len].to_string();
            ranges.push(ParsedRange {
                start_pos: abs_start,
                end_pos: abs_start + range_len,
                text,
                duration: result.duration,
                error: result.error,
            });
            search_start = abs_start + range_len;
        } else if let Some(m) = timestamp_pattern.find(remaining) {
            // Looks like a timestamp but failed to parse - malformed
            if m.start() == 0 {
                return Err(format!("Malformed timestamp: {}", m.as_str()));
            }
            search_start = abs_start + 1;
        } else {
            search_start = abs_start + 1;
        }
    }

    Ok(ranges)
}

fn format_duration(seconds: u32) -> String {
    let mins = seconds / 60;
    let secs = seconds % 60;
    format!("{}:{:02}", mins, secs)
}

pub fn calculate_durations(input: &str) -> Result<ParseOutput, String> {
    let ranges = find_all_ranges(input)?;
    
    // Check for invalid ranges
    for range in &ranges {
        match &range.error {
            RangeError::EndBeforeStart => {
                return Err(format!("Invalid timestamp range: {} (end time is before start time)", range.text));
            }
            RangeError::InvalidMinutes(mins) => {
                return Err(format!("Invalid timestamp range: {} (minutes {} exceeds 59, use H:MM:SS format)", range.text, mins));
            }
            RangeError::InvalidSeconds(secs) => {
                return Err(format!("Invalid timestamp range: {} (seconds {} exceeds 59)", range.text, secs));
            }
            RangeError::None => {}
        }
    }
    
    let mut lines = Vec::new();
    let mut grand_total = 0;
    let mut id = 1;

    let mut i = 0;
    while i < ranges.len() {
        let mut group_texts = vec![ranges[i].text.clone()];
        let mut group_duration = ranges[i].duration;
        let mut last_end = ranges[i].end_pos;

        // Check for consecutive ranges connected by " + "
        while i + 1 < ranges.len() {
            let between = &input[last_end..ranges[i + 1].start_pos];
            if between.trim() == "+" {
                i += 1;
                group_texts.push(ranges[i].text.clone());
                group_duration += ranges[i].duration;
                last_end = ranges[i].end_pos;
            } else {
                break;
            }
        }

        let input_text = group_texts.join(" + ");
        lines.push(LineResult {
            id,
            input: input_text,
            result: DurationResult {
                seconds: group_duration,
                format: format_duration(group_duration),
            },
        });
        grand_total += group_duration;
        id += 1;
        i += 1;
    }

    Ok(ParseOutput {
        lines,
        total: DurationResult {
            seconds: grand_total,
            format: format_duration(grand_total),
        },
    })
}

pub fn clean_script(input: &str) -> String {
    let ranges = find_all_ranges(input);
    
    // If parsing fails or no ranges found, return original
    let ranges = match ranges {
        Ok(r) if !r.is_empty() => r,
        _ => return input.to_string(),
    };
    
    let mut result = String::with_capacity(input.len());
    let mut last_pos = 0;
    let mut i = 0;
    
    while i < ranges.len() {
        let range = &ranges[i];
        let text_start = last_pos;
        let mut text_end = range.start_pos;
        let mut skip_to = range.end_pos;
        
        // Check if there's a space before the timestamp
        let before_space = text_end > 0 && input.as_bytes().get(text_end - 1) == Some(&b' ');
        if before_space {
            text_end -= 1; // Exclude the space before
        }
        
        // Check if this range is connected to the next one with " + "
        if i + 1 < ranges.len() {
            let between = &input[range.end_pos..ranges[i + 1].start_pos];
            if between.trim() == "+" {
                // Skip this range, the " + ", and the next range
                skip_to = ranges[i + 1].end_pos;
                i += 1; // Skip the next range in the loop
            }
        }
        
        // Check if there's a space after the timestamp
        let after_space = skip_to < input.len() && input.as_bytes().get(skip_to) == Some(&b' ');
        if after_space {
            skip_to += 1; // Skip the space after
        }
        
        // Add text before this range (excluding space before if present)
        result.push_str(&input[text_start..text_end]);
        
        // If we removed spaces on both sides and we're joining words, add a single space
        let before_char = if text_end > 0 { 
            input[text_end - 1..].chars().next() 
        } else { 
            None 
        };
        let after_char = if skip_to < input.len() { 
            input[skip_to..].chars().next() 
        } else { 
            None 
        };
        
        // Check if we need to add a space (joining two alphanumeric characters)
        let needs_space = before_char.is_some() 
            && after_char.is_some()
            && before_char.unwrap().is_alphanumeric()
            && after_char.unwrap().is_alphanumeric()
            && !before_space && !after_space;
        
        if needs_space {
            result.push(' ');
        }
        
        last_pos = skip_to;
        i += 1;
    }
    
    // Add remaining text after last range
    result.push_str(&input[last_pos..]);
    
    // Clean up any leftover " + " that might be orphaned
    result = result.replace(" + ", " ").replace("+ ", "").replace(" +", "");
    
    result
}