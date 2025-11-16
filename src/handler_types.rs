//! Handler-specific types that guarantee correctness

use crate::adapters::{HandlebarsAdapter, NginxVariableResolver, SqliteQueryExecutor};
use crate::config::{MainConfig, ModuleConfig};
use crate::domain::{RequestProcessor, ValidatedConfig};
use crate::nginx_helpers::{get_doc_root_and_uri, send_response};
use crate::parsing;
use crate::{domain, Module};
use ngx::core::Status;
use ngx::http::{HttpModuleLocationConf, HttpModuleMainConf};
use ngx::ngx_log_debug_http;

/// Proof that we have valid configuration (Ghost of Departed Proofs)
pub struct ValidConfigToken<'a> {
    config: &'a ModuleConfig,
}

impl<'a> ValidConfigToken<'a> {
    /// Try to create a token - returns None if config is invalid
    pub fn new(config: &'a ModuleConfig) -> Option<Self> {
        if config.db_path.is_empty() || config.query.is_empty() || config.template_path.is_empty() {
            return None;
        }
        Some(ValidConfigToken { config })
    }

    pub fn get(&self) -> &ModuleConfig {
        self.config
    }
}

/// Process a request with guaranteed valid configuration
/// Returns Status directly - no Result needed, types prove correctness
pub fn process_request(
    request: &mut ngx::http::Request,
    config: ValidConfigToken,
) -> Status {
    // Parse config into validated types
    // If this fails, it's a programming error (config should be valid per token)
    let validated_config = match parsing::parse_config(config.get()) {
        Ok(c) => c,
        Err(e) => {
            ngx_log_debug_http!(request, "unexpected parse error: {}", e);
            return ngx::http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    // Get nginx paths - can fail due to nginx state, not our logic
    let (doc_root, uri) = match get_doc_root_and_uri(request) {
        Ok(paths) => paths,
        Err(e) => {
            ngx_log_debug_http!(request, "nginx path error: {}", e);
            return ngx::http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    // Pure function - cannot fail
    let resolved_template = domain::resolve_template_path(&doc_root, &uri, &validated_config.template_path);

    // Resolve parameters - can fail if nginx variable doesn't exist
    let var_resolver = NginxVariableResolver::new(request);
    let resolved_params = match domain::resolve_parameters(&validated_config.parameters, &var_resolver) {
        Ok(params) => params,
        Err(e) => {
            ngx_log_debug_http!(request, "parameter error: {}", e);
            return ngx::http::HTTPStatus::BAD_REQUEST.into();
        }
    };

    // Create processor and run - uses dependency injection
    let html = execute_with_processor(&validated_config, &resolved_template, &resolved_params, request);
    
    // Send response - proven correct by types
    send_response(request, &html)
}

/// Execute query and render with proper dependency injection
fn execute_with_processor(
    config: &ValidatedConfig,
    resolved_template: &domain::ResolvedTemplate,
    resolved_params: &[(String, String)],
    request: &mut ngx::http::Request,
) -> String {
    let mut reg = handlebars::Handlebars::new();
    let reg_ptr: *mut handlebars::Handlebars<'static> = unsafe { std::mem::transmute(&mut reg) };
    let hbs_adapter = unsafe { HandlebarsAdapter::new(reg_ptr) };
    
    let processor = RequestProcessor::new(
        SqliteQueryExecutor,
        hbs_adapter,
        hbs_adapter,
    );

    let main_conf = Module::main_conf(request).expect("main config is none");
    let global_dir = if !main_conf.global_templates_dir.is_empty() {
        Some(main_conf.global_templates_dir.as_str())
    } else {
        None
    };

    // If processor fails, return error HTML instead of panicking
    processor
        .process(config, resolved_template, resolved_params, global_dir)
        .unwrap_or_else(|e| {
            format!(
                "<html><body><h1>Error</h1><pre>{}</pre></body></html>",
                e
            )
        })
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

        let token = ValidConfigToken::new(&config);
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

        let token = ValidConfigToken::new(&config);
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

        let token = ValidConfigToken::new(&config);
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

        let token = ValidConfigToken::new(&config);
        assert!(token.is_none());
    }
}

