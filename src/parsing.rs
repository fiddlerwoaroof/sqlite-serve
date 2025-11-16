//! Parse raw configuration strings into validated domain types

use crate::config::ModuleConfig;
use crate::domain::ValidatedConfig;
use crate::types::{DatabasePath, NginxVariable, ParamName, ParameterBinding, SqlQuery, TemplatePath};

/// Parse raw configuration into validated domain configuration
pub fn parse_config(config: &ModuleConfig) -> Result<ValidatedConfig, String> {
    let db_path = DatabasePath::parse(&config.db_path)
        .map_err(|e| format!("invalid db_path: {}", e))?;

    let query = SqlQuery::parse(&config.query)
        .map_err(|e| format!("invalid query: {}", e))?;

    let template_path = TemplatePath::parse(&config.template_path)
        .map_err(|e| format!("invalid template_path: {}", e))?;

    let parameters = parse_parameter_bindings(&config.query_params)?;

    Ok(ValidatedConfig {
        db_path,
        query,
        template_path,
        parameters,
    })
}

/// Parse parameter configuration into typed bindings
fn parse_parameter_bindings(
    params: &[(String, String)],
) -> Result<Vec<ParameterBinding>, String> {
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

        let validated = parse_config(&config).unwrap();
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

        let result = parse_config(&config);
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

        let result = parse_config(&config);
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
}

