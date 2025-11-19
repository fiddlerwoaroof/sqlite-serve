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
    Positional {
        variable: NginxVariable,
    },
    PositionalLiteral {
        value: String,
    },
    Named {
        name: ParamName,
        variable: NginxVariable,
    },
    NamedLiteral {
        name: ParamName,
        value: String,
    },
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

    // Additional edge case tests for SqlQuery
    #[test]
    fn test_sql_query_with_leading_whitespace() {
        let query = SqlQuery::parse("  \t\n  SELECT * FROM books").unwrap();
        assert!(query.as_str().contains("SELECT"));
    }

    #[test]
    fn test_sql_query_with_trailing_whitespace() {
        let query = SqlQuery::parse("SELECT * FROM books  \n\t  ").unwrap();
        assert_eq!(query.as_str().trim(), "SELECT * FROM books");
    }

    #[test]
    fn test_sql_query_mixed_case() {
        let query = SqlQuery::parse("SeLeCt id, name FrOm books").unwrap();
        assert!(query.as_str().contains("SeLeCt"));
    }

    #[test]
    fn test_sql_query_rejects_drop() {
        let result = SqlQuery::parse("DROP TABLE books");
        assert!(result.is_err());
    }

    #[test]
    fn test_sql_query_rejects_create() {
        let result = SqlQuery::parse("CREATE TABLE books (id INT)");
        assert!(result.is_err());
    }

    #[test]
    fn test_sql_query_rejects_alter() {
        let result = SqlQuery::parse("ALTER TABLE books ADD COLUMN x");
        assert!(result.is_err());
    }

    #[test]
    fn test_sql_query_with_semicolon() {
        // SELECT queries with semicolons are allowed (single statement)
        let query = SqlQuery::parse("SELECT * FROM books;").unwrap();
        assert!(query.as_str().contains("SELECT"));
    }

    #[test]
    fn test_sql_query_only_whitespace() {
        let result = SqlQuery::parse("   \t\n   ");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    // Additional edge case tests for TemplatePath
    #[test]
    fn test_template_path_case_sensitive_extension() {
        // .HBS (uppercase) should be rejected - only .hbs is valid
        let result = TemplatePath::parse("template.HBS");
        assert!(result.is_err());
    }

    #[test]
    fn test_template_path_multiple_dots() {
        let path = TemplatePath::parse("my.template.backup.hbs").unwrap();
        assert!(path.as_str().ends_with(".hbs"));
    }

    #[test]
    fn test_template_path_hidden_file() {
        let path = TemplatePath::parse(".hidden.hbs").unwrap();
        assert_eq!(path.as_str(), ".hidden.hbs");
    }

    #[test]
    fn test_template_path_no_extension() {
        let result = TemplatePath::parse("template");
        assert!(result.is_err());
    }

    #[test]
    fn test_template_path_wrong_extension() {
        let result = TemplatePath::parse("template.handlebars");
        assert!(result.is_err());
    }

    // Additional edge case tests for NginxVariable
    #[test]
    fn test_nginx_variable_with_underscore() {
        let var = NginxVariable::parse("$arg_book_id").unwrap();
        assert_eq!(var.name(), "arg_book_id");
    }

    #[test]
    fn test_nginx_variable_with_numbers() {
        let var = NginxVariable::parse("$arg_id123").unwrap();
        assert_eq!(var.name(), "arg_id123");
    }

    #[test]
    fn test_nginx_variable_common_patterns() {
        // Test common nginx variable patterns
        assert!(NginxVariable::parse("$request_uri").is_ok());
        assert!(NginxVariable::parse("$http_host").is_ok());
        assert!(NginxVariable::parse("$remote_addr").is_ok());
        assert!(NginxVariable::parse("$query_string").is_ok());
    }

    #[test]
    fn test_nginx_variable_dollar_in_middle() {
        // Dollar sign in the middle should be allowed
        let var = NginxVariable::parse("$weird$name").unwrap();
        assert_eq!(var.as_str(), "$weird$name");
    }

    // Additional edge case tests for ParamName
    #[test]
    fn test_param_name_only_colon() {
        let result = ParamName::parse(":");
        // Single colon is valid - the name after : can be empty for positional
        assert!(result.is_ok());
    }

    #[test]
    fn test_param_name_with_numbers() {
        let param = ParamName::parse(":param123").unwrap();
        assert_eq!(param.as_str(), ":param123");
    }

    #[test]
    fn test_param_name_with_underscore() {
        let param = ParamName::parse(":book_id").unwrap();
        assert_eq!(param.as_str(), ":book_id");
    }

    #[test]
    fn test_param_name_positional_is_empty() {
        let param = ParamName::positional();
        assert!(param.is_positional());
        assert!(param.as_str().is_empty());
    }

    #[test]
    fn test_param_name_named_not_positional() {
        let param = ParamName::parse(":id").unwrap();
        assert!(!param.is_positional());
    }

    // Additional edge case tests for DatabasePath
    #[test]
    fn test_database_path_with_directory() {
        let path = DatabasePath::parse("/var/data/test.db").unwrap();
        assert!(path.as_str().contains("test.db"));
    }

    #[test]
    fn test_database_path_relative() {
        let path = DatabasePath::parse("./data/test.db").unwrap();
        assert!(path.as_str().contains("test.db"));
    }

    #[test]
    fn test_database_path_just_filename() {
        let path = DatabasePath::parse("test.db").unwrap();
        assert_eq!(path.as_str(), "test.db");
    }
}
