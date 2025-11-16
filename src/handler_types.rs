//! Handler-specific types that guarantee correctness

use crate::adapters::{HandlebarsAdapter, NginxVariableResolver, SqliteQueryExecutor};
use crate::config::{MainConfig, ModuleConfig};
use crate::domain::{RequestProcessor, ValidatedConfig};
use crate::logging;
use crate::nginx_helpers::{get_doc_root_and_uri, send_response};
use crate::parsing;
use crate::{domain, Module};
use ngx::core::Status;
use ngx::http::{HttpModuleLocationConf, HttpModuleMainConf};

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
    logging::log(
        request,
        logging::LogLevel::Debug,
        "handler",
        &format!("Processing request for {}", request.unparsed_uri().to_str().unwrap_or("unknown")),
    );

    // Parse config into validated types
    let validated_config = match parsing::parse_config(config.get()) {
        Ok(c) => c,
        Err(e) => {
            logging::log_config_error(request, "configuration", "", &e);
            return ngx::http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    // Get nginx paths
    let (doc_root, uri) = match get_doc_root_and_uri(request) {
        Ok(paths) => paths,
        Err(e) => {
            logging::log(request, logging::LogLevel::Error, "nginx", &format!("Path resolution failed: {}", e));
            return ngx::http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    // Resolve template path (pure function - cannot fail)
    let resolved_template = domain::resolve_template_path(&doc_root, &uri, &validated_config.template_path);

    logging::log(
        request,
        logging::LogLevel::Debug,
        "template",
        &format!("Resolved template: {}", resolved_template.full_path()),
    );

    // Resolve parameters
    let var_resolver = NginxVariableResolver::new(request);
    let resolved_params = match domain::resolve_parameters(&validated_config.parameters, &var_resolver) {
        Ok(params) => {
            if !params.is_empty() {
                logging::log(
                    request,
                    logging::LogLevel::Debug,
                    "params",
                    &format!("Resolved {} parameters", params.len()),
                );
            }
            params
        }
        Err(e) => {
            logging::log_param_error(request, "variable", &e);
            return ngx::http::HTTPStatus::BAD_REQUEST.into();
        }
    };

    // Execute and render
    let html = execute_with_processor(&validated_config, &resolved_template, &resolved_params, request);
    
    // Send response
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
        logging::log_template_loading(
            request,
            "global",
            0,
            &main_conf.global_templates_dir,
        );
        Some(main_conf.global_templates_dir.as_str())
    } else {
        None
    };

    // Process through functional core
    match processor.process(config, resolved_template, resolved_params, global_dir) {
        Ok(html) => {
            // Count results for logging (parse HTML or trust it worked)
            logging::log(
                request,
                logging::LogLevel::Info,
                "success",
                &format!(
                    "Rendered {} with {} params",
                    resolved_template.full_path().split('/').last().unwrap_or("template"),
                    resolved_params.len()
                ),
            );
            html
        }
        Err(e) => {
            // Log detailed error information
            if e.contains("query") {
                logging::log_query_error(request, config.query.as_str(), &e);
            } else if e.contains("template") {
                logging::log_template_error(request, resolved_template.full_path(), &e);
            } else {
                logging::log(
                    request,
                    logging::LogLevel::Error,
                    "processing",
                    &format!("Request processing failed: {}", e),
                );
            }

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

