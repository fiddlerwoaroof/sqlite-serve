//! Handler-specific types that guarantee correctness

use crate::adapters::{NginxLogger, NginxVariableResolver, SqliteQueryExecutor};
use crate::config::ModuleConfig;
use crate::content_type::{ContentType, negotiate_content_type};
use crate::domain::{Logger, RequestProcessor, ValidatedConfig};
use crate::logging;
use crate::nginx_helpers::{send_json_response, send_response};
use crate::parsing;
use crate::template::HandlebarsAdapter;
use crate::{Module, domain};
use ngx::core::Status;
use ngx::http::HttpModuleMainConf;

pub struct ValidConfigToken {
    config: ValidatedConfig,
}

impl ValidConfigToken {
    /// Try to create a token - returns None if config is invalid
    pub fn new(config: &ModuleConfig, doc_root: String, uri: String) -> Option<Self> {
        if config.db_path.is_empty() || config.query.is_empty() || config.template_path.is_empty() {
            return None;
        }
        parsing::parse_config(config, doc_root, uri)
            .map(|c| ValidConfigToken { config: c })
            .ok()
    }

    pub fn get(&self) -> &ValidatedConfig {
        &self.config
    }
}

/// Process a request with guaranteed valid configuration
/// Returns Status directly - no Result needed, types prove correctness
pub fn process_request(
    request: &mut ngx::http::Request,
    validated_config: &ValidatedConfig,
) -> Status {
    let mut logger = NginxLogger::new(request);
    
    logger.debug(
        "handler",
        &format!("Processing request for {}", validated_config.uri),
    );

    // Resolve template path (pure function - cannot fail)
    let resolved_template = domain::resolve_template_path(&validated_config);

    logger.debug(
        "template",
        &format!("Resolved template: {}", resolved_template.full_path()),
    );

    // Resolve parameters
    let mut var_resolver = NginxVariableResolver::new(request);
    let resolved_params =
        match domain::resolve_parameters(&validated_config.parameters, &mut var_resolver) {
            Ok(params) => {
                if !params.is_empty() {
                    logger.debug(
                        "params",
                        &format!("Resolved {} parameters", params.len()),
                    );
                }
                params
            }
            Err(e) => {
                logger.error("params", &format!("Parameter resolution failed: {}", e));
                return ngx::http::HTTPStatus::BAD_REQUEST.into();
            }
        };

    // Negotiate content type based on Accept header
    let content_type = negotiate_content_type(request);

    // Execute query and format response
    match content_type {
        ContentType::Json => {
            let json = execute_json(&validated_config, &resolved_params, request);
            send_json_response(request, &json)
        }
        ContentType::Html => {
            let html = execute_with_processor(
                &validated_config,
                &resolved_template,
                &resolved_params,
                request,
            );
            send_response(request, &html)
        }
    }
}

/// Execute query and render with proper dependency injection
fn execute_with_processor(
    config: &ValidatedConfig,
    resolved_template: &domain::ResolvedTemplate,
    resolved_params: &[(String, String)],
    request: &mut ngx::http::Request,
) -> String {
    let reg = HandlebarsAdapter::new();
    let logger = NginxLogger::new(request);

    let mut processor = RequestProcessor::new(SqliteQueryExecutor, reg, logger);

    let main_conf = Module::main_conf(request).expect("main config is none");
    let global_dir = if !main_conf.global_templates_dir.is_empty() {
        Some(main_conf.global_templates_dir.as_str())
    } else {
        None
    };

    // Process through functional core
    match processor.process(config, resolved_template, resolved_params, global_dir) {
        Ok(html) => {
            // Success is already logged in the processor
            html
        }
        Err(e) => {
            // Errors are already logged in the processor
            // Return user-friendly error page
            format!(
                r#"<!DOCTYPE html>
<html>
<head><title>Error - sqlite-serve</title></head>
<body style="font-family: monospace; max-width: 800px; margin: 2rem auto; padding: 0 1rem;">
    <h1 style="color: #CC9393;">Request Processing Error</h1>
    <p style="color: #A6A689;">An error occurred while processing your request.</p>
    <details style="margin-top: 1rem; background: #1111; padding: 1rem; border-left: 3px solid #CC9393;">
        <summary style="cursor: pointer; color: #DFAF8F; font-weight: bold;">Error Details</summary>
        <pre style="margin-top: 1rem; color: #DCDCCC; overflow-x: auto;">{}</pre>
    </details>
    <p style="margin-top: 2rem;"><a href="/" style="color: #7CB8BB;">← Back to Home</a></p>
</body>
</html>"#,
                e
            )
        }
    }
}

/// Execute query and return JSON (no template rendering)
fn execute_json(
    config: &ValidatedConfig,
    resolved_params: &[(String, String)],
    request: &mut ngx::http::Request,
) -> String {
    use crate::domain::QueryExecutor;

    let mut logger = NginxLogger::new(request);
    let executor = SqliteQueryExecutor;

    logger.debug("query", &format!("Executing query for JSON: {}", config.query.as_str()));

    match executor.execute(&config.db_path, &config.query, resolved_params) {
        Ok(results) => {
            logger.info(
                "success",
                &format!(
                    "Returned {} JSON results with {} params",
                    results.len(),
                    resolved_params.len()
                ),
            );
            serde_json::to_string_pretty(&results).unwrap_or_else(|e| {
                logger.error("json", &format!("JSON serialization failed: {}", e));
                "[]".to_string()
            })
        }
        Err(e) => {
            logger.error("query", &format!("Query failed: {} - Error: {}", config.query.as_str(), e));
            let error_obj = serde_json::json!({
                "error": "Query execution failed",
                "details": e
            });
            serde_json::to_string(&error_obj)
                .unwrap_or_else(|_| r#"{"error":"serialization failed"}"#.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_config_token_accepts_valid() {
        let config = ModuleConfig {
            db_path: "test.db".to_string(),
            query: "SELECT * FROM test".to_string(),
            template_path: "test.hbs".to_string(),
            query_params: vec![],
        };

        let token = ValidConfigToken::new(&config, "".into(), "".into());
        assert!(token.is_some());
    }

    #[test]
    fn test_valid_config_token_rejects_empty_db() {
        let config = ModuleConfig {
            db_path: String::new(),
            query: "SELECT * FROM test".to_string(),
            template_path: "test.hbs".to_string(),
            query_params: vec![],
        };

        let token = ValidConfigToken::new(&config, "".into(), "".into());
        assert!(token.is_none());
    }

    #[test]
    fn test_valid_config_token_rejects_empty_query() {
        let config = ModuleConfig {
            db_path: "test.db".to_string(),
            query: String::new(),
            template_path: "test.hbs".to_string(),
            query_params: vec![],
        };

        let token = ValidConfigToken::new(&config, "".into(), "".into());
        assert!(token.is_none());
    }

    #[test]
    fn test_valid_config_token_rejects_empty_template() {
        let config = ModuleConfig {
            db_path: "test.db".to_string(),
            query: "SELECT * FROM test".to_string(),
            template_path: String::new(),
            query_params: vec![],
        };

        let token = ValidConfigToken::new(&config, "".into(), "".into());
        assert!(token.is_none());
    }
}
