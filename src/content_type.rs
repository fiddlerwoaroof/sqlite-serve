//! Content type negotiation based on Accept headers

use ngx::http::Request;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Html,
    Json,
}

impl ContentType {
    pub fn content_type_header(&self) -> &'static str {
        match self {
            ContentType::Html => "text/html; charset=utf-8",
            ContentType::Json => "application/json; charset=utf-8",
        }
    }
}

/// Determine response content type based on Accept header
pub fn negotiate_content_type(request: &Request) -> ContentType {
    // Use safe headers_in_iterator() method provided by ngx crate
    for (key, value) in request.headers_in_iterator() {
        if let Ok(key_str) = key.to_str() {
            if key_str.eq_ignore_ascii_case("accept") {
                if let Ok(value_str) = value.to_str() {
                    let value_lower = value_str.to_lowercase();

                    // Check if JSON is preferred over HTML
                    if value_lower.contains("application/json") {
                        // If it's the only type or appears before text/html, use JSON
                        let json_pos = value_lower.find("application/json");
                        let html_pos = value_lower.find("text/html");

                        match (json_pos, html_pos) {
                            (Some(_), None) => return ContentType::Json,
                            (Some(j), Some(h)) if j < h => return ContentType::Json,
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Default to HTML
    ContentType::Html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_header() {
        assert_eq!(
            ContentType::Html.content_type_header(),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            ContentType::Json.content_type_header(),
            "application/json; charset=utf-8"
        );
    }

    #[test]
    fn test_content_type_equality() {
        assert_eq!(ContentType::Html, ContentType::Html);
        assert_eq!(ContentType::Json, ContentType::Json);
        assert_ne!(ContentType::Html, ContentType::Json);
    }
}
