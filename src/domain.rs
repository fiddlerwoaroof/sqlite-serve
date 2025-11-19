//! Pure functional core with dependency injection (Functional Core, Imperative Shell)

use crate::types::{DatabasePath, ParameterBinding, SqlQuery, TemplatePath};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Log levels for structured logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Logger trait for dependency injection (enables logging without Request)
pub trait Logger {
    /// Log a message with context
    fn log(&self, level: LogLevel, module: &str, message: &str);

    /// Convenience method for debug logging
    fn debug(&self, module: &str, message: &str) {
        self.log(LogLevel::Debug, module, message);
    }

    /// Convenience method for info logging
    fn info(&self, module: &str, message: &str) {
        self.log(LogLevel::Info, module, message);
    }

    /// Convenience method for warn logging
    fn warn(&self, module: &str, message: &str) {
        self.log(LogLevel::Warn, module, message);
    }

    /// Convenience method for error logging
    fn error(&self, module: &str, message: &str) {
        self.log(LogLevel::Error, module, message);
    }
}

/// Configuration for a location (validated at parse time)
#[derive(Debug, Clone)]
pub struct ValidatedConfig {
    pub db_path: DatabasePath,
    pub query: SqlQuery,
    pub template_path: TemplatePath,
    pub parameters: Vec<ParameterBinding>,
    pub doc_root: String,
    pub uri: String,
}

impl ValidatedConfig {
    pub fn resolve_template_path(&self) -> ResolvedTemplate {
        let full_path = format!(
            "{}{}/{}",
            self.doc_root,
            self.uri,
            self.template_path.as_str()
        );
        let directory = Path::new(&full_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();

        ResolvedTemplate {
            full_path,
            directory,
        }
    }
}

/// Template resolution result
#[derive(Debug)]
pub struct ResolvedTemplate {
    pub full_path: String,
    pub directory: String,
}

impl ResolvedTemplate {
    pub fn full_path(&self) -> &str {
        &self.full_path
    }

    pub fn directory(&self) -> &str {
        &self.directory
    }
}

/// Resolve template path relative to document root and URI (pure function)
pub fn resolve_template_path(config: &ValidatedConfig) -> ResolvedTemplate {
    config.resolve_template_path()
}

/// Parameter resolution strategy (dependency injection)
pub trait VariableResolver {
    fn resolve(&mut self, var_name: &str) -> Result<String, String>;
}

/// Resolve all parameters using the provided resolver
pub fn resolve_parameters(
    bindings: &[ParameterBinding],
    resolver: &mut dyn VariableResolver,
) -> Result<Vec<(String, String)>, String> {
    let mut resolved = Vec::new();

    for binding in bindings {
        match binding {
            ParameterBinding::Positional { variable } => {
                let value = resolver.resolve(variable.as_str())?;
                resolved.push((String::new(), value));
            }
            ParameterBinding::PositionalLiteral { value } => {
                resolved.push((String::new(), value.clone()));
            }
            ParameterBinding::Named { name, variable } => {
                let value = resolver.resolve(variable.as_str())?;
                resolved.push((name.as_str().to_string(), value));
            }
            ParameterBinding::NamedLiteral { name, value } => {
                resolved.push((name.as_str().to_string(), value.clone()));
            }
        }
    }

    Ok(resolved)
}

/// Query execution strategy (dependency injection)
pub trait QueryExecutor {
    fn execute(
        &self,
        db_path: &DatabasePath,
        query: &SqlQuery,
        params: &[(String, String)],
    ) -> Result<Vec<HashMap<String, Value>>, String>;
}

/// Template loading strategy (dependency injection)
pub trait TemplateLoader {
    fn load_from_dir(&mut self, dir_path: &str) -> Result<usize, String>;
    fn register_template(&mut self, name: &str, path: &str) -> Result<(), String>;
}

/// Template rendering strategy (dependency injection)
pub trait TemplateRenderer {
    fn render(&self, template_name: &str, data: &Value) -> Result<String, String>;
}

/// Pure business logic for request handling
pub struct RequestProcessor<Q, L: TemplateLoader + TemplateRenderer, Log: Logger> {
    query_executor: Q,
    template_loader: L,
    logger: Log,
}

impl<Q, L, Log> RequestProcessor<Q, L, Log>
where
    Q: QueryExecutor,
    L: TemplateLoader + TemplateRenderer,
    Log: Logger,
{
    pub fn new(query_executor: Q, template_loader: L, logger: Log) -> Self {
        RequestProcessor {
            query_executor,
            template_loader,
            logger,
        }
    }

    /// Process a request (pure, testable business logic)
    pub fn process(
        &mut self,
        config: &ValidatedConfig,
        resolved_template: &ResolvedTemplate,
        resolved_params: &[(String, String)],
        global_template_dir: Option<&str>,
    ) -> Result<String, String> {
        self.logger.debug(
            "processor",
            &format!("Processing request for {}", config.uri),
        );

        // Execute query
        self.logger.debug(
            "query",
            &format!("Executing query: {}", config.query.as_str()),
        );
        let results = self
            .query_executor
            .execute(&config.db_path, &config.query, resolved_params)
            .map_err(|e| {
                self.logger
                    .error("query", &format!("Query execution failed: {}", e));
                format!("query execution failed: {}", e)
            })?;

        self.logger
            .debug("query", &format!("Query returned {} rows", results.len()));

        // Load global templates if provided
        if let Some(dir) = global_template_dir {
            self.logger.debug(
                "templates",
                &format!("Loading global templates from: {}", dir),
            );
            match self.template_loader.load_from_dir(dir) {
                Ok(count) => {
                    self.logger.info(
                        "templates",
                        &format!("Loaded {} global template(s) from '{}'", count, dir),
                    );
                }
                Err(e) => {
                    self.logger.warn(
                        "templates",
                        &format!("Failed to load global templates from '{}': {}", dir, e),
                    );
                }
            }
        }

        // Load local templates
        self.logger.debug(
            "templates",
            &format!(
                "Loading local templates from: {}",
                resolved_template.directory()
            ),
        );
        match self
            .template_loader
            .load_from_dir(resolved_template.directory())
        {
            Ok(count) => {
                self.logger
                    .debug("templates", &format!("Loaded {} local template(s)", count));
            }
            Err(e) => {
                self.logger.warn(
                    "templates",
                    &format!("Failed to load local templates: {}", e),
                );
            }
        }

        // Register main template
        self.logger.debug(
            "templates",
            &format!(
                "Registering main template: {}",
                resolved_template.full_path()
            ),
        );
        self.template_loader
            .register_template("template", resolved_template.full_path())
            .map_err(|e| {
                self.logger.error(
                    "template",
                    &format!(
                        "Failed to register template '{}': {}",
                        resolved_template.full_path(),
                        e
                    ),
                );
                format!("failed to register template: {}", e)
            })?;

        // Render
        self.logger
            .debug("render", "Rendering template with query results");
        let data = serde_json::json!({"results": results});
        self.template_loader.render("template", &data).map_err(|e| {
            self.logger
                .error("render", &format!("Template rendering failed: {}", e));
            format!("rendering failed: {}", e)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NginxVariable, ParamName};

    #[test]
    fn test_resolve_template_path() {
        let template = TemplatePath::parse("list.hbs").unwrap();
        let resolved = resolve_template_path(&ValidatedConfig {
            db_path: DatabasePath::parse("asdf").expect("fail"),
            query: SqlQuery::parse("SELECT whatever").expect("fail"),
            template_path: template,
            parameters: Vec::new(),
            doc_root: "server_root".into(),
            uri: "/books".into(),
        });

        assert_eq!(resolved.full_path(), "server_root/books/list.hbs");
        assert_eq!(resolved.directory(), "server_root/books");
    }

    #[test]
    fn test_resolve_template_path_with_trailing_slash() {
        let template = TemplatePath::parse("index.hbs").unwrap();
        let resolved = resolve_template_path(&ValidatedConfig {
            db_path: DatabasePath::parse("asdf").expect("fail"),
            query: SqlQuery::parse("SELECT whatever").expect("fail"),
            template_path: template,
            parameters: Vec::new(),
            doc_root: "public/".into(),
            uri: "/docs/".into(),
        });

        assert!(resolved.full_path().contains("public//docs/"));
    }

    // Mock implementations for testing
    struct MockVariableResolver;
    impl VariableResolver for MockVariableResolver {
        fn resolve(&mut self, var_name: &str) -> Result<String, String> {
            match var_name {
                "$arg_id" => Ok("123".to_string()),
                "$arg_genre" => Ok("Fiction".to_string()),
                _ => Err(format!("unknown variable: {}", var_name)),
            }
        }
    }

    struct MockQueryExecutor;
    impl QueryExecutor for MockQueryExecutor {
        fn execute(
            &self,
            _db_path: &DatabasePath,
            _query: &SqlQuery,
            _params: &[(String, String)],
        ) -> Result<Vec<HashMap<String, Value>>, String> {
            let mut row = HashMap::new();
            row.insert("id".to_string(), Value::Number(1.into()));
            row.insert("title".to_string(), Value::String("Test Book".to_string()));
            Ok(vec![row])
        }
    }

    struct MockTemplateSystem;
    impl TemplateLoader for MockTemplateSystem {
        fn load_from_dir(&mut self, _dir_path: &str) -> Result<usize, String> {
            Ok(0)
        }
        fn register_template(&mut self, _name: &str, _path: &str) -> Result<(), String> {
            Ok(())
        }
    }

    impl TemplateRenderer for MockTemplateSystem {
        fn render(&self, _template_name: &str, data: &Value) -> Result<String, String> {
            Ok(format!("Rendered: {:?}", data))
        }
    }

    struct MockLogger;
    impl Logger for MockLogger {
        fn log(&self, _level: LogLevel, _module: &str, _message: &str) {
            // No-op for tests
        }
    }

    #[test]
    fn test_resolve_parameters_positional() {
        let bindings = vec![ParameterBinding::Positional {
            variable: NginxVariable::parse("$arg_id").unwrap(),
        }];

        let mut resolver = MockVariableResolver;
        let resolved = resolve_parameters(&bindings, &mut resolver).unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, ""); // Positional (no name)
        assert_eq!(resolved[0].1, "123");
    }

    #[test]
    fn test_resolve_parameters_named() {
        let bindings = vec![ParameterBinding::Named {
            name: ParamName::parse(":book_id").unwrap(),
            variable: NginxVariable::parse("$arg_id").unwrap(),
        }];

        let mut resolver = MockVariableResolver;
        let resolved = resolve_parameters(&bindings, &mut resolver).unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, ":book_id");
        assert_eq!(resolved[0].1, "123");
    }

    #[test]
    fn test_resolve_parameters_literal() {
        let bindings = vec![ParameterBinding::PositionalLiteral {
            value: "constant".to_string(),
        }];

        let mut resolver = MockVariableResolver;
        let resolved = resolve_parameters(&bindings, &mut resolver).unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].1, "constant");
    }

    #[test]
    fn test_request_processor_integration() {
        let config = ValidatedConfig {
            db_path: DatabasePath::parse("test.db").unwrap(),
            query: SqlQuery::parse("SELECT * FROM books").unwrap(),
            template_path: TemplatePath::parse("list.hbs").unwrap(),
            parameters: vec![],
            doc_root: "".into(),
            uri: "".into(),
        };

        let resolved_template = ResolvedTemplate {
            full_path: "templates/list.hbs".to_string(),
            directory: "templates".to_string(),
        };

        let mut processor =
            RequestProcessor::new(MockQueryExecutor, MockTemplateSystem, MockLogger);

        let result = processor.process(&config, &resolved_template, &[], None);

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Rendered"));
    }
}
