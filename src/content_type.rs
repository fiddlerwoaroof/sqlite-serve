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
    let r: *const ngx::ffi::ngx_http_request_t = request.into();

    unsafe {
        let headers_in = &(*r).headers_in;

        // Iterate through all headers to find Accept
        let mut current = headers_in.headers.part.elts as *mut ngx::ffi::ngx_table_elt_t;
        let nelts = headers_in.headers.part.nelts;

        for _ in 0..nelts {
            if current.is_null() {
                break;
            }

            let header = &*current;
            if let Ok(key) = header.key.to_str() {
                if key.eq_ignore_ascii_case("accept") {
                    if let Ok(value) = header.value.to_str() {
                        let value_lower = value.to_lowercase();

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

            current = current.add(1);
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
