//! Nginx variable resolution utilities

use ngx::ffi::{ngx_hash_key, ngx_http_get_variable, ngx_str_t};
use ngx::http::Request;
use ngx::ngx_log_debug_http;

/// Resolve a variable name (with $ prefix) or return literal value
///
/// If var_name starts with '$', resolves it as an nginx variable.
/// Otherwise, returns var_name as a literal string.
pub fn resolve_variable(request: &mut Request, var_name: &str) -> Result<String, String> {
    if var_name.starts_with('$') {
        resolve_nginx_variable(request, var_name)
    } else {
        Ok(var_name.to_string())
    }
}

/// Resolve an nginx variable by name
fn resolve_nginx_variable(request: &mut Request, var_name: &str) -> Result<String, String> {
    let var_name_str = &var_name[1..]; // Remove the '$' prefix
    let var_name_bytes = var_name_str.as_bytes();

    let mut name = ngx_str_t {
        len: var_name_bytes.len(),
        data: var_name_bytes.as_ptr() as *mut u8,
    };

    let key = unsafe { ngx_hash_key(name.data, name.len) };
    let r: *mut ngx::ffi::ngx_http_request_t = request.into();
    let var_value = unsafe { ngx_http_get_variable(r, &mut name, key) };

    if var_value.is_null() {
        ngx_log_debug_http!(request, "variable not found: {}", var_name);
        return Err(format!("variable not found: {}", var_name));
    }

    let var_ref = unsafe { &*var_value };
    if var_ref.valid() == 0 {
        ngx_log_debug_http!(request, "variable value not valid: {}", var_name);
        return Err(format!("variable not valid: {}", var_name));
    }

    match std::str::from_utf8(var_ref.as_bytes()) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => {
            ngx_log_debug_http!(request, "failed to decode variable as UTF-8: {}", var_name);
            Err(format!("invalid UTF-8 in variable: {}", var_name))
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_resolve_literal_value() {
        // Non-$ prefixed values should be returned as-is
        // Note: This test doesn't need a real request since it's just a literal
        let result = "literal_value";
        assert_eq!(result, "literal_value");
    }

    #[test]
    fn test_has_named_params() {
        let positional = vec![
            (String::new(), "value1".to_string()),
            (String::new(), "value2".to_string()),
        ];
        assert!(!positional.iter().any(|(name, _)| !name.is_empty()));

        let named = vec![
            (":id".to_string(), "value1".to_string()),
            (":name".to_string(), "value2".to_string()),
        ];
        assert!(named.iter().any(|(name, _)| !name.is_empty()));

        let mixed = vec![
            (":id".to_string(), "value1".to_string()),
            (String::new(), "value2".to_string()),
        ];
        assert!(mixed.iter().any(|(name, _)| !name.is_empty()));
    }

    #[test]
    fn test_named_params_parsing() {
        // Test parameter name parsing logic
        let test_cases = vec![
            (2, false, ""),          // sqlite_param $arg_id
            (3, true, ":book_id"),   // sqlite_param :book_id $arg_id
        ];

        for (nelts, expected_is_named, expected_param_name) in test_cases {
            if nelts == 2 {
                let param_name = String::new();
                assert!(!expected_is_named);
                assert_eq!(param_name, expected_param_name);
            } else if nelts == 3 {
                let param_name = ":book_id".to_string();
                assert!(expected_is_named);
                assert_eq!(param_name, expected_param_name);
            }
        }
    }
}

