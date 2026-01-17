use nom::{
    bytes::complete::tag,
    character::complete::digit1,
    combinator::map_res,
    sequence::{delimited, separated_pair},
    IResult, Parser,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
struct Timestamp {
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
        self.minutes * 60 + self.seconds
    }
}

fn parse_number(input: &str) -> IResult<&str, u32> {
    map_res(digit1, |s: &str| s.parse::<u32>()).parse(input)
}

fn parse_timestamp(input: &str) -> IResult<&str, Timestamp> {
    let (input, (minutes, seconds)) = separated_pair(parse_number, tag(":"), parse_number).parse(input)?;
    Ok((input, Timestamp { minutes, seconds }))
}

fn parse_range(input: &str) -> IResult<&str, u32> {
    let (input, (start, end)) = delimited(
        tag("("),
        separated_pair(parse_timestamp, tag("-"), parse_timestamp),
        tag(")"),
    ).parse(input)?;
    
    let duration = end.to_seconds() - start.to_seconds();
    Ok((input, duration))
}

// Represents a parsed timestamp range with its position and text in the input
struct ParsedRange {
    start_pos: usize,
    end_pos: usize,
    text: String,
    duration: u32,
}

fn find_all_ranges(input: &str) -> Vec<ParsedRange> {
    let mut ranges = Vec::new();
    let mut search_start = 0;

    while let Some(paren_pos) = input[search_start..].find('(') {
        let abs_start = search_start + paren_pos;
        let remaining = &input[abs_start..];
        
        if let Ok((rest, duration)) = parse_range(remaining) {
            let range_len = remaining.len() - rest.len();
            let text = input[abs_start..abs_start + range_len].to_string();
            ranges.push(ParsedRange {
                start_pos: abs_start,
                end_pos: abs_start + range_len,
                text,
                duration,
            });
            search_start = abs_start + range_len;
        } else {
            search_start = abs_start + 1;
        }
    }

    ranges
}

fn format_duration(seconds: u32) -> String {
    let mins = seconds / 60;
    let secs = seconds % 60;
    format!("{}:{:02}", mins, secs)
}

pub fn calculate_durations(input: &str) -> ParseOutput {
    let ranges = find_all_ranges(input);
    
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

    ParseOutput {
        lines,
        total: DurationResult {
            seconds: grand_total,
            format: format_duration(grand_total),
        },
    }
}