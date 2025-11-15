//! Type-safe wrappers for domain concepts (Parse, Don't Validate)

use std::path::{Path, PathBuf};

/// A validated database path that exists and is accessible
#[derive(Debug, Clone)]
pub struct DatabasePath(PathBuf);

impl DatabasePath {
    /// Parse and validate a database path
    pub fn parse(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        // For now, just validate it's not empty
        // In production, could check if file exists, is readable, etc.
        if path.as_os_str().is_empty() {
            return Err("database path cannot be empty".to_string());
        }
        Ok(DatabasePath(path.to_path_buf()))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.to_str().unwrap_or("")
    }
}

/// A validated SQL query (must be SELECT)
#[derive(Debug, Clone)]
pub struct SqlQuery(String);

impl SqlQuery {
    /// Parse and validate a SQL query
    pub fn parse(query: impl Into<String>) -> Result<Self, String> {
        let query = query.into();
        let trimmed = query.trim().to_uppercase();
        
        if trimmed.is_empty() {
            return Err("query cannot be empty".to_string());
        }
        
        // Ensure it's a SELECT query (read-only)
        if !trimmed.starts_with("SELECT") {
            return Err("only SELECT queries are allowed".to_string());
        }
        
        Ok(SqlQuery(query))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated template path
#[derive(Debug, Clone)]
pub struct TemplatePath(PathBuf);

impl TemplatePath {
    /// Parse and validate a template path
    pub fn parse(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        
        if path.as_os_str().is_empty() {
            return Err("template path cannot be empty".to_string());
        }
        
        // Ensure it's a .hbs file
        if path.extension().and_then(|e| e.to_str()) != Some("hbs") {
            return Err("template must be a .hbs file".to_string());
        }
        
        Ok(TemplatePath(path.to_path_buf()))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.to_str().unwrap_or("")
    }
}

/// A validated nginx variable name (starts with $)
#[derive(Debug, Clone)]
pub struct NginxVariable(String);

impl NginxVariable {
    /// Parse a nginx variable name
    pub fn parse(name: impl Into<String>) -> Result<Self, String> {
        let name = name.into();
        
        if name.is_empty() {
            return Err("variable name cannot be empty".to_string());
        }
        
        if !name.starts_with('$') {
            return Err(format!("variable name must start with $: {}", name));
        }
        
        // Get the part after $
        let var_name = &name[1..];
        if var_name.is_empty() {
            return Err("variable name after $ cannot be empty".to_string());
        }
        
        Ok(NginxVariable(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
    
    /// Get the variable name without the $ prefix
    pub fn name(&self) -> &str {
        &self.0[1..]
    }
}

/// A SQL parameter name (starts with :)
#[derive(Debug, Clone)]
pub struct ParamName(String);

impl ParamName {
    /// Parse a SQL parameter name
    pub fn parse(name: impl Into<String>) -> Result<Self, String> {
        let name = name.into();
        
        if name.is_empty() {
            return Err("parameter name cannot be empty".to_string());
        }
        
        if !name.starts_with(':') {
            return Err(format!("parameter name must start with :: {}", name));
        }
        
        Ok(ParamName(name))
    }
    
    /// Create an empty (positional) parameter name
    pub fn positional() -> Self {
        ParamName(String::new())
    }
    
    pub fn is_positional(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A parameter binding (param name + variable or literal)
#[derive(Debug, Clone)]
pub enum ParameterBinding {
    Positional { variable: NginxVariable },
    PositionalLiteral { value: String },
    Named { name: ParamName, variable: NginxVariable },
    NamedLiteral { name: ParamName, value: String },
}

impl ParameterBinding {
    pub fn param_name(&self) -> String {
        match self {
            ParameterBinding::Positional { .. } | ParameterBinding::PositionalLiteral { .. } => {
                String::new()
            }
            ParameterBinding::Named { name, .. } | ParameterBinding::NamedLiteral { name, .. } => {
                name.as_str().to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_path_valid() {
        let path = DatabasePath::parse("test.db").unwrap();
        assert_eq!(path.as_str(), "test.db");
    }

    #[test]
    fn test_database_path_empty() {
        let result = DatabasePath::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_sql_query_valid_select() {
        let query = SqlQuery::parse("SELECT * FROM books").unwrap();
        assert_eq!(query.as_str(), "SELECT * FROM books");
    }

    #[test]
    fn test_sql_query_case_insensitive() {
        let query = SqlQuery::parse("select id from test").unwrap();
        assert!(query.as_str().to_uppercase().starts_with("SELECT"));
    }

    #[test]
    fn test_sql_query_rejects_insert() {
        let result = SqlQuery::parse("INSERT INTO books VALUES (1)");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SELECT"));
    }

    #[test]
    fn test_sql_query_rejects_update() {
        let result = SqlQuery::parse("UPDATE books SET title = 'x'");
        assert!(result.is_err());
    }

    #[test]
    fn test_sql_query_rejects_delete() {
        let result = SqlQuery::parse("DELETE FROM books");
        assert!(result.is_err());
    }

    #[test]
    fn test_sql_query_rejects_empty() {
        let result = SqlQuery::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_template_path_valid() {
        let path = TemplatePath::parse("template.hbs").unwrap();
        assert_eq!(path.as_str(), "template.hbs");
    }

    #[test]
    fn test_template_path_with_directory() {
        let path = TemplatePath::parse("views/index.hbs").unwrap();
        assert!(path.as_str().ends_with("index.hbs"));
    }

    #[test]
    fn test_template_path_rejects_non_hbs() {
        let result = TemplatePath::parse("template.html");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(".hbs"));
    }

    #[test]
    fn test_template_path_rejects_empty() {
        let result = TemplatePath::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_nginx_variable_valid() {
        let var = NginxVariable::parse("$arg_id").unwrap();
        assert_eq!(var.as_str(), "$arg_id");
        assert_eq!(var.name(), "arg_id");
    }

    #[test]
    fn test_nginx_variable_rejects_without_dollar() {
        let result = NginxVariable::parse("arg_id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("$"));
    }

    #[test]
    fn test_nginx_variable_rejects_empty() {
        let result = NginxVariable::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_nginx_variable_rejects_only_dollar() {
        let result = NginxVariable::parse("$");
        assert!(result.is_err());
    }

    #[test]
    fn test_param_name_valid() {
        let param = ParamName::parse(":book_id").unwrap();
        assert_eq!(param.as_str(), ":book_id");
    }

    #[test]
    fn test_param_name_positional() {
        let param = ParamName::positional();
        assert!(param.is_positional());
        assert_eq!(param.as_str(), "");
    }

    #[test]
    fn test_param_name_rejects_without_colon() {
        let result = ParamName::parse("book_id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(":"));
    }
}

