//! Pure functional core with dependency injection (Functional Core, Imperative Shell)

use crate::types::{DatabasePath, ParameterBinding, SqlQuery, TemplatePath};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Configuration for a location (validated at parse time)
#[derive(Debug, Clone)]
pub struct ValidatedConfig {
    pub db_path: DatabasePath,
    pub query: SqlQuery,
    pub template_path: TemplatePath,
    pub parameters: Vec<ParameterBinding>,
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
pub fn resolve_template_path(
    doc_root: &str,
    uri: &str,
    template: &TemplatePath,
) -> ResolvedTemplate {
    let full_path = format!("{}{}/{}", doc_root, uri, template.as_str());
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

/// Parameter resolution strategy (dependency injection)
pub trait VariableResolver {
    fn resolve(&self, var_name: &str) -> Result<String, String>;
}

/// Resolve all parameters using the provided resolver
pub fn resolve_parameters(
    bindings: &[ParameterBinding],
    resolver: &dyn VariableResolver,
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
    fn load_from_dir(&self, dir_path: &str) -> Result<usize, String>;
    fn register_template(&self, name: &str, path: &str) -> Result<(), String>;
}

/// Template rendering strategy (dependency injection)
pub trait TemplateRenderer {
    fn render(&self, template_name: &str, data: &Value) -> Result<String, String>;
}

/// Pure business logic for request handling
pub struct RequestProcessor<Q, L, R> {
    query_executor: Q,
    template_loader: L,
    renderer: R,
}

impl<Q, L, R> RequestProcessor<Q, L, R>
where
    Q: QueryExecutor,
    L: TemplateLoader,
    R: TemplateRenderer,
{
    pub fn new(query_executor: Q, template_loader: L, renderer: R) -> Self {
        RequestProcessor {
            query_executor,
            template_loader,
            renderer,
        }
    }

    /// Process a request (pure, testable business logic)
    pub fn process(
        &self,
        config: &ValidatedConfig,
        resolved_template: &ResolvedTemplate,
        resolved_params: &[(String, String)],
        global_template_dir: Option<&str>,
    ) -> Result<String, String> {
        // Execute query
        let results = self
            .query_executor
            .execute(&config.db_path, &config.query, resolved_params)
            .map_err(|e| format!("query execution failed: {}", e))?;

        // Load global templates if provided
        if let Some(dir) = global_template_dir {
            self.template_loader.load_from_dir(dir).ok();
        }

        // Load local templates
        self.template_loader
            .load_from_dir(resolved_template.directory())
            .ok();

        // Register main template
        self.template_loader
            .register_template("template", resolved_template.full_path())
            .map_err(|e| format!("failed to register template: {}", e))?;

        // Render
        let data = serde_json::json!({"results": results});
        self.renderer
            .render("template", &data)
            .map_err(|e| format!("rendering failed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NginxVariable, ParamName};

    #[test]
    fn test_resolve_template_path() {
        let template = TemplatePath::parse("list.hbs").unwrap();
        let resolved = resolve_template_path("server_root", "/books", &template);

        assert_eq!(resolved.full_path(), "server_root/books/list.hbs");
        assert_eq!(resolved.directory(), "server_root/books");
    }

    #[test]
    fn test_resolve_template_path_with_trailing_slash() {
        let template = TemplatePath::parse("index.hbs").unwrap();
        let resolved = resolve_template_path("public/", "/docs/", &template);

        assert!(resolved.full_path().contains("public//docs/"));
    }

    // Mock implementations for testing
    struct MockVariableResolver;
    impl VariableResolver for MockVariableResolver {
        fn resolve(&self, var_name: &str) -> Result<String, String> {
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

    struct MockTemplateLoader;
    impl TemplateLoader for MockTemplateLoader {
        fn load_from_dir(&self, _dir_path: &str) -> Result<usize, String> {
            Ok(0)
        }
        fn register_template(&self, _name: &str, _path: &str) -> Result<(), String> {
            Ok(())
        }
    }

    struct MockTemplateRenderer;
    impl TemplateRenderer for MockTemplateRenderer {
        fn render(&self, _template_name: &str, data: &Value) -> Result<String, String> {
            Ok(format!("Rendered: {:?}", data))
        }
    }

    #[test]
    fn test_resolve_parameters_positional() {
        let bindings = vec![ParameterBinding::Positional {
            variable: NginxVariable::parse("$arg_id").unwrap(),
        }];

        let resolver = MockVariableResolver;
        let resolved = resolve_parameters(&bindings, &resolver).unwrap();

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

        let resolver = MockVariableResolver;
        let resolved = resolve_parameters(&bindings, &resolver).unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, ":book_id");
        assert_eq!(resolved[0].1, "123");
    }

    #[test]
    fn test_resolve_parameters_literal() {
        let bindings = vec![ParameterBinding::PositionalLiteral {
            value: "constant".to_string(),
        }];

        let resolver = MockVariableResolver;
        let resolved = resolve_parameters(&bindings, &resolver).unwrap();

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
        };

        let resolved_template = ResolvedTemplate {
            full_path: "templates/list.hbs".to_string(),
            directory: "templates".to_string(),
        };

        let processor =
            RequestProcessor::new(MockQueryExecutor, MockTemplateLoader, MockTemplateRenderer);

        let result = processor.process(&config, &resolved_template, &[], None);

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Rendered"));
    }
}
