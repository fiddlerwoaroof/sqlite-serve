//! Parse raw configuration strings into validated domain types

use crate::config::ModuleConfig;
use crate::domain::ValidatedConfig;
use crate::types::{
    DatabasePath, NginxVariable, ParamName, ParameterBinding, SqlQuery, TemplatePath,
};

/// Parse raw configuration into validated domain configuration
pub fn parse_config(
    config: &ModuleConfig,
    doc_root: String,
    uri: String,
) -> Result<ValidatedConfig, String> {
    let db_path =
        DatabasePath::parse(&config.db_path).map_err(|e| format!("invalid db_path: {}", e))?;

    let query = SqlQuery::parse(&config.query).map_err(|e| format!("invalid query: {}", e))?;

    let template_path = TemplatePath::parse(&config.template_path)
        .map_err(|e| format!("invalid template_path: {}", e))?;

    let parameters = parse_parameter_bindings(&config.query_params)?;

    Ok(ValidatedConfig {
        db_path,
        query,
        template_path,
        parameters,
        doc_root,
        uri,
    })
}

/// Parse parameter configuration into typed bindings
fn parse_parameter_bindings(params: &[(String, String)]) -> Result<Vec<ParameterBinding>, String> {
    let mut bindings = Vec::new();

    for (param_name, var_name) in params {
        let binding = if var_name.starts_with('$') {
            // Variable reference
            let variable = NginxVariable::parse(var_name)
                .map_err(|e| format!("invalid variable '{}': {}", var_name, e))?;

            if param_name.is_empty() {
                ParameterBinding::Positional { variable }
            } else {
                let name = ParamName::parse(param_name)
                    .map_err(|e| format!("invalid param name '{}': {}", param_name, e))?;
                ParameterBinding::Named { name, variable }
            }
        } else {
            // Literal value
            if param_name.is_empty() {
                ParameterBinding::PositionalLiteral {
                    value: var_name.clone(),
                }
            } else {
                let name = ParamName::parse(param_name)
                    .map_err(|e| format!("invalid param name '{}': {}", param_name, e))?;
                ParameterBinding::NamedLiteral {
                    name,
                    value: var_name.clone(),
                }
            }
        };

        bindings.push(binding);
    }

    Ok(bindings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config_valid() {
        let config = ModuleConfig {
            db_path: "test.db".to_string(),
            query: "SELECT * FROM books".to_string(),
            template_path: "list.hbs".to_string(),
            query_params: vec![],
        };

        let validated = parse_config(&config, "".into(), "".into()).unwrap();
        assert_eq!(validated.db_path.as_str(), "test.db");
        assert!(validated.query.as_str().contains("SELECT"));
    }

    #[test]
    fn test_parse_config_invalid_query() {
        let config = ModuleConfig {
            db_path: "test.db".to_string(),
            query: "DELETE FROM books".to_string(),
            template_path: "list.hbs".to_string(),
            query_params: vec![],
        };

        let result = parse_config(&config, "".into(), "".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SELECT"));
    }

    #[test]
    fn test_parse_config_invalid_template() {
        let config = ModuleConfig {
            db_path: "test.db".to_string(),
            query: "SELECT * FROM books".to_string(),
            template_path: "list.html".to_string(),
            query_params: vec![],
        };

        let result = parse_config(&config, "".into(), "".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(".hbs"));
    }

    #[test]
    fn test_parse_parameter_bindings_positional() {
        let params = vec![(String::new(), "$arg_id".to_string())];
        let bindings = parse_parameter_bindings(&params).unwrap();

        assert_eq!(bindings.len(), 1);
        match &bindings[0] {
            ParameterBinding::Positional { variable } => {
                assert_eq!(variable.name(), "arg_id");
            }
            _ => panic!("expected positional binding"),
        }
    }

    #[test]
    fn test_parse_parameter_bindings_named() {
        let params = vec![(":book_id".to_string(), "$arg_id".to_string())];
        let bindings = parse_parameter_bindings(&params).unwrap();

        assert_eq!(bindings.len(), 1);
        match &bindings[0] {
            ParameterBinding::Named { name, variable } => {
                assert_eq!(name.as_str(), ":book_id");
                assert_eq!(variable.name(), "arg_id");
            }
            _ => panic!("expected named binding"),
        }
    }

    #[test]
    fn test_parse_parameter_bindings_literal() {
        let params = vec![(String::new(), "constant".to_string())];
        let bindings = parse_parameter_bindings(&params).unwrap();

        assert_eq!(bindings.len(), 1);
        match &bindings[0] {
            ParameterBinding::PositionalLiteral { value } => {
                assert_eq!(value, "constant");
            }
            _ => panic!("expected positional literal binding"),
        }
    }

    #[test]
    fn test_parse_parameter_bindings_invalid_variable() {
        let params = vec![(String::new(), "arg_id".to_string())];
        let bindings = parse_parameter_bindings(&params).unwrap();

        // Without $, it's treated as a literal
        match &bindings[0] {
            ParameterBinding::PositionalLiteral { value } => {
                assert_eq!(value, "arg_id");
            }
            _ => panic!("expected literal binding"),
        }
    }

    // Additional edge case tests
    #[test]
    fn test_parse_parameter_bindings_multiple_mixed() {
        let params = vec![
            (":id".to_string(), "$arg_id".to_string()),
            (String::new(), "$arg_limit".to_string()),
            (":status".to_string(), "active".to_string()),
            (String::new(), "100".to_string()),
        ];
        let bindings = parse_parameter_bindings(&params).unwrap();

        assert_eq!(bindings.len(), 4);

        // First: named variable
        match &bindings[0] {
            ParameterBinding::Named { name, variable } => {
                assert_eq!(name.as_str(), ":id");
                assert_eq!(variable.name(), "arg_id");
            }
            _ => panic!("expected named binding"),
        }

        // Second: positional variable
        match &bindings[1] {
            ParameterBinding::Positional { variable } => {
                assert_eq!(variable.name(), "arg_limit");
            }
            _ => panic!("expected positional binding"),
        }

        // Third: named literal
        match &bindings[2] {
            ParameterBinding::NamedLiteral { name, value } => {
                assert_eq!(name.as_str(), ":status");
                assert_eq!(value, "active");
            }
            _ => panic!("expected named literal"),
        }

        // Fourth: positional literal
        match &bindings[3] {
            ParameterBinding::PositionalLiteral { value } => {
                assert_eq!(value, "100");
            }
            _ => panic!("expected positional literal"),
        }
    }

    #[test]
    fn test_parse_parameter_bindings_empty() {
        let params = vec![];
        let bindings = parse_parameter_bindings(&params).unwrap();
        assert_eq!(bindings.len(), 0);
    }

    #[test]
    fn test_parse_parameter_bindings_all_literals() {
        let params = vec![
            (String::new(), "literal1".to_string()),
            (":name".to_string(), "literal2".to_string()),
            (String::new(), "123".to_string()),
        ];
        let bindings = parse_parameter_bindings(&params).unwrap();

        assert_eq!(bindings.len(), 3);
        assert!(matches!(bindings[0], ParameterBinding::PositionalLiteral { .. }));
        assert!(matches!(bindings[1], ParameterBinding::NamedLiteral { .. }));
        assert!(matches!(bindings[2], ParameterBinding::PositionalLiteral { .. }));
    }

    #[test]
    fn test_parse_parameter_bindings_all_variables() {
        let params = vec![
            (String::new(), "$arg_a".to_string()),
            (":name".to_string(), "$arg_b".to_string()),
            (String::new(), "$arg_c".to_string()),
        ];
        let bindings = parse_parameter_bindings(&params).unwrap();

        assert_eq!(bindings.len(), 3);
        assert!(matches!(bindings[0], ParameterBinding::Positional { .. }));
        assert!(matches!(bindings[1], ParameterBinding::Named { .. }));
        assert!(matches!(bindings[2], ParameterBinding::Positional { .. }));
    }

    #[test]
    fn test_parse_config_with_parameters() {
        let config = ModuleConfig {
            db_path: "test.db".to_string(),
            query: "SELECT * FROM books WHERE id = ?".to_string(),
            template_path: "book.hbs".to_string(),
            query_params: vec![(String::new(), "$arg_id".to_string())],
        };

        let validated = parse_config(&config, "/var/www".into(), "/books".into()).unwrap();
        assert_eq!(validated.parameters.len(), 1);
        assert_eq!(validated.doc_root, "/var/www");
        assert_eq!(validated.uri, "/books");
    }

    #[test]
    fn test_parse_config_with_multiple_parameters() {
        let config = ModuleConfig {
            db_path: "catalog.db".to_string(),
            query: "SELECT * FROM items WHERE category = :cat AND status = :status".to_string(),
            template_path: "items.hbs".to_string(),
            query_params: vec![
                (":cat".to_string(), "$arg_category".to_string()),
                (":status".to_string(), "active".to_string()),
            ],
        };

        let validated = parse_config(&config, "public".into(), "/api/items".into()).unwrap();
        assert_eq!(validated.parameters.len(), 2);
    }

    #[test]
    fn test_parse_config_empty_strings() {
        let config = ModuleConfig {
            db_path: "test.db".to_string(),
            query: "SELECT 1".to_string(),
            template_path: "simple.hbs".to_string(),
            query_params: vec![],
        };

        let validated = parse_config(&config, "".into(), "".into()).unwrap();
        assert_eq!(validated.doc_root, "");
        assert_eq!(validated.uri, "");
    }

    #[test]
    fn test_parse_config_invalid_empty_db() {
        let config = ModuleConfig {
            db_path: "".to_string(),
            query: "SELECT 1".to_string(),
            template_path: "test.hbs".to_string(),
            query_params: vec![],
        };

        let result = parse_config(&config, "".into(), "".into());
        assert!(result.is_err());
    }
}
