use std::fmt;

use chromiumoxide::cdp::browser_protocol::network::{CookieParam, CookieSameSite, TimeSinceEpoch};

use crate::Result;

/// Error for when parsing a cookie file
#[derive(Debug, Clone)]
struct CookieFileParseError {
    error_message: String,
}
impl CookieFileParseError {
    fn new(error_message: String) -> CookieFileParseError {
        CookieFileParseError {
            error_message: error_message,
        }
    }
}
impl fmt::Display for CookieFileParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error parsing Cookie file: {}", self.error_message)
    }
}
impl std::error::Error for CookieFileParseError {
    fn description(&self) -> &str {
        &self.error_message
    }
}

/// Parse a cookie file
/// As specified in https://curl.se/docs/http-cookies.html
///
/// # Arguments
/// * `file_contents` - The contents of the cookie file
///
/// # Returns
/// * A vector of CookieParam structs
pub fn parse_cookie_file(file_contents: &str) -> Result<Vec<CookieParam>> {
    let mut cookies: Vec<CookieParam> = Vec::new();
    // https://curl.se/docs/http-cookies.html
    for line_unchanged in file_contents.lines() {
        let mut cookie_builder = CookieParam::builder().source_port(-1);
        let mut line = line_unchanged;

        if line.starts_with("#HttpOnly_") {
            line = &line[10..];
            cookie_builder = cookie_builder.http_only(true);
        } else if line.starts_with("#") {
            continue;
        }

        let cookie_args: Vec<&str> = line.split('\t').collect();
        if cookie_args.len() != 7 {
            tracing::error!(
                "Error parsing cookie line (Wrong number of arguments): '{}'",
                line
            );
            return Err(Box::new(CookieFileParseError::new(format!(
                "Error parsing cookie line (Wrong number of arguments): '{}'",
                line
            ))));
        }

        cookie_builder = cookie_builder
            .domain(cookie_args[0].to_string())
            .same_site(if cookie_args[1].eq("TRUE") {
                CookieSameSite::Strict
            } else {
                CookieSameSite::Lax
            })
            .path(cookie_args[2].to_string())
            .http_only(cookie_args[3].eq("TRUE"))
            .expires(TimeSinceEpoch::new(match cookie_args[4].parse::<f64>() {
                Ok(value) => value,
                Err(err) => {
                    return Err(Box::new(CookieFileParseError::new(format!(
                        "Error parsing cookie line: '{}' Could not convert time: '{}'",
                        line, err
                    ))));
                }
            }))
            .name(cookie_args[5].to_string())
            .value(cookie_args[6].to_string());

        let cookie = cookie_builder.build()?;

        tracing::trace!("Parsed cookie line: {:?} to {:?}", line_unchanged, cookie);

        cookies.push(cookie);
    }
    Ok(cookies)
}
