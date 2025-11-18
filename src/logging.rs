//! Structured logging utilities for sqlite-serve

use ngx::http::Request;
use ngx::ngx_log_error;

/// Log levels matching nginx conventions
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Log a message with context using nginx's native logging
pub fn log(request: &mut Request, level: LogLevel, module: &str, message: &str) {
    let log_level = match level {
        LogLevel::Error => 3, // NGX_LOG_ERR
        LogLevel::Warn => 4,  // NGX_LOG_WARN
        LogLevel::Info => 6,  // NGX_LOG_INFO
        LogLevel::Debug => 7, // NGX_LOG_DEBUG
    };

    let r: *mut ngx::ffi::ngx_http_request_t = request.into();
    unsafe {
        let connection = (*r).connection;
        if !connection.is_null() {
            let log = (*connection).log;
            if !log.is_null() {
                ngx_log_error!(log_level, log, "[sqlite-serve:{}] {}", module, message);
            }
        }
    }
}

pub fn debug(request: &mut Request, module: &str, message: &str) {
    log(request, LogLevel::Debug, module, message);
}

/// Log configuration parsing error
pub fn log_config_error(request: &mut Request, field: &str, value: &str, error: &str) {
    log(
        request,
        LogLevel::Error,
        "config",
        &format!("Invalid {}: '{}' - {}", field, value, error),
    );
}

/// Log query execution error
pub fn log_query_error(request: &mut Request, query: &str, error: &str) {
    log(
        request,
        LogLevel::Error,
        "query",
        &format!("Query failed: {} - Error: {}", query, error),
    );
}

/// Log template error
pub fn log_template_error(request: &mut Request, template_path: &str, error: &str) {
    log(
        request,
        LogLevel::Error,
        "template",
        &format!("Template '{}' failed: {}", template_path, error),
    );
}

/// Log parameter resolution error
pub fn log_param_error(request: &mut Request, param: &str, error: &str) {
    log(
        request,
        LogLevel::Warn,
        "params",
        &format!("Parameter '{}' resolution failed: {}", param, error),
    );
}

/// Log successful request processing
pub fn log_request_success(
    request: &mut Request,
    query: &str,
    param_count: usize,
    result_count: usize,
    template: &str,
) {
    log(
        request,
        LogLevel::Info,
        "request",
        &format!(
            "Processed: query='{}' params={} results={} template='{}'",
            query, param_count, result_count, template
        ),
    );
}

/// Log template loading info
pub fn log_template_loading(request: &mut Request, source: &str, count: usize, dir: &str) {
    log(
        request,
        LogLevel::Debug,
        "templates",
        &format!("Loaded {} {} templates from '{}'", count, source, dir),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering() {
        // Just verify we can construct log levels
        let levels = vec![
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ];
        assert_eq!(levels.len(), 4);
    }
}
