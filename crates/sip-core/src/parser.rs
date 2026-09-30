use crate::error::{ParseError, Result};
use crate::headers::{Header, Headers};
use crate::message::{Request, Response, SipMessage};
use crate::method::Method;
use crate::uri::parse_uri;
use std::str::FromStr;

/// Parses a full SIP message (Request or Response) from a byte slice or string
pub fn parse_message(input: &[u8]) -> Result<SipMessage> {
    // 1. Unfold headers and separate headers part from body part
    let (header_lines, body_start_offset) = extract_header_lines(input)?;

    if header_lines.is_empty() {
        return Err(ParseError::MalformedStartLine("Empty message".into()));
    }

    let first_line = &header_lines[0];
    let mut headers = Headers::new();

    for line in &header_lines[1..] {
        if let Some((name, val)) = line.split_once(':') {
            let parsed_headers = Header::parse(name, val)?;
            for h in parsed_headers {
                headers.push(h);
            }
        } else {
            // Malformed header line without colon
            return Err(ParseError::InvalidHeader(format!(
                "Missing colon in header: {}",
                line
            )));
        }
    }

    // Determine body
    let remaining_body = &input[body_start_offset..];
    let body = if let Some(cl) = headers.content_length() {
        if remaining_body.len() >= cl {
            remaining_body[..cl].to_vec()
        } else {
            remaining_body.to_vec()
        }
    } else {
        remaining_body.to_vec()
    };

    // Determine whether request or response
    if first_line.starts_with("SIP/2.0") || first_line.starts_with("sip/2.0") {
        let mut parts = first_line.split_whitespace();
        let version = parts.next().unwrap_or("SIP/2.0").to_string();
        let status_code_str = parts.next().ok_or_else(|| {
            ParseError::MalformedStartLine(format!("Missing status code in: {}", first_line))
        })?;
        let status_code = status_code_str.parse::<u16>().map_err(|_| {
            ParseError::InvalidStatusCode(format!("Invalid status code: {}", status_code_str))
        })?;

        // The rest of the line is the reason phrase
        let rest_offset = first_line.find(status_code_str).unwrap() + status_code_str.len();
        let reason_phrase = first_line[rest_offset..].trim().to_string();

        Ok(SipMessage::Response(Response {
            version,
            status_code,
            reason_phrase,
            headers,
            body,
        }))
    } else {
        let mut parts = first_line.split_whitespace();
        let method_str = parts.next().ok_or_else(|| {
            ParseError::MalformedStartLine(format!("Missing method in: {}", first_line))
        })?;
        let uri_str = parts.next().ok_or_else(|| {
            ParseError::MalformedStartLine(format!("Missing URI in: {}", first_line))
        })?;
        let version = parts.next().unwrap_or("SIP/2.0").to_string();

        let method = Method::from_str(method_str)?;
        let uri = parse_uri(uri_str)?;

        Ok(SipMessage::Request(Request {
            method,
            uri,
            version,
            headers,
            body,
        }))
    }
}

/// Helper that extracts lines, handles CRLF / LF / CR, and un-folds header continuations (leading space/tab)
fn extract_header_lines(input: &[u8]) -> Result<(Vec<String>, usize)> {
    let mut lines = Vec::new();
    let mut idx = 0;
    let len = input.len();
    let mut current_line = String::new();

    while idx < len {
        // Read until next \r, \n, or \r\n
        let line_start = idx;
        let line_end;
        let next_idx;

        loop {
            if idx >= len {
                line_end = len;
                next_idx = len;
                break;
            }
            let b = input[idx];
            if b == b'\r' {
                line_end = idx;
                if idx + 1 < len && input[idx + 1] == b'\n' {
                    next_idx = idx + 2;
                } else {
                    next_idx = idx + 1;
                }
                break;
            } else if b == b'\n' {
                line_end = idx;
                next_idx = idx + 1;
                break;
            }
            idx += 1;
        }

        let slice = &input[line_start..line_end];
        let line_str = String::from_utf8_lossy(slice);

        // Check for empty line ending the headers
        if line_str.trim().is_empty() {
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            return Ok((lines, next_idx));
        }

        // Check if this line is a folded header (starts with space or tab)
        if (line_str.starts_with(' ') || line_str.starts_with('\t'))
            && !lines.is_empty()
            && current_line.is_empty()
        {
            // Append continuation to the last line in lines
            let last_idx = lines.len() - 1;
            lines[last_idx].push(' ');
            lines[last_idx].push_str(line_str.trim());
        } else if (line_str.starts_with(' ') || line_str.starts_with('\t'))
            && !current_line.is_empty()
        {
            current_line.push(' ');
            current_line.push_str(line_str.trim());
        } else {
            // Push previous line if any
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            current_line = line_str.to_string();
        }

        idx = next_idx;
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    Ok((lines, len))
}
