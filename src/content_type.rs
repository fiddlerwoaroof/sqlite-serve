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
    // For now, check query parameter as a simple way to request JSON
    // Full Accept header parsing would require more nginx FFI work
    let r: *const ngx::ffi::ngx_http_request_t = request.into();
    
    unsafe {
        // Check args for format=json
        let args = (*r).args;
        if args.len > 0 && !args.data.is_null() {
            let args_slice = std::slice::from_raw_parts(args.data, args.len);
            if let Ok(args_str) = std::str::from_utf8(args_slice) {
                if args_str.contains("format=json") {
                    return ContentType::Json;
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
        assert_eq!(ContentType::Html.content_type_header(), "text/html; charset=utf-8");
        assert_eq!(ContentType::Json.content_type_header(), "application/json; charset=utf-8");
    }

    #[test]
    fn test_content_type_equality() {
        assert_eq!(ContentType::Html, ContentType::Html);
        assert_eq!(ContentType::Json, ContentType::Json);
        assert_ne!(ContentType::Html, ContentType::Json);
    }
}

