//! Configuration structures for the sqlite-serve module

use ngx::http::MergeConfigError;

/// Location-specific configuration
#[derive(Debug, Default)]
pub struct ModuleConfig {
    pub db_path: String,
    pub query: String,
    pub template_path: String,
    pub query_params: Vec<(String, String)>, // (param_name, variable_name) pairs
}

/// Global (HTTP main) configuration for shared templates
#[derive(Debug, Default)]
pub struct MainConfig {
    pub global_templates_dir: String,
}

impl ngx::http::Merge for ModuleConfig {
    fn merge(&mut self, prev: &ModuleConfig) -> Result<(), MergeConfigError> {
        if self.db_path.is_empty() {
            self.db_path = prev.db_path.clone();
        }

        if self.query.is_empty() {
            self.query = prev.query.clone();
        }

        if self.template_path.is_empty() {
            self.template_path = prev.template_path.clone();
        }

        if self.query_params.is_empty() {
            self.query_params = prev.query_params.clone();
        }

        Ok(())
    }
}

impl ngx::http::Merge for MainConfig {
    fn merge(&mut self, prev: &MainConfig) -> Result<(), MergeConfigError> {
        if self.global_templates_dir.is_empty() {
            self.global_templates_dir = prev.global_templates_dir.clone();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ngx::http::Merge;

    #[test]
    fn test_module_config_default() {
        let config = ModuleConfig::default();
        assert!(config.db_path.is_empty());
        assert!(config.query.is_empty());
        assert!(config.template_path.is_empty());
        assert!(config.query_params.is_empty());
    }

    #[test]
    fn test_module_config_merge() {
        let mut config = ModuleConfig {
            db_path: String::new(),
            query: String::new(),
            template_path: String::new(),
            query_params: vec![],
        };

        let prev = ModuleConfig {
            db_path: "test.db".to_string(),
            query: "SELECT * FROM test".to_string(),
            template_path: "test.hbs".to_string(),
            query_params: vec![("id".to_string(), "$arg_id".to_string())],
        };

        config.merge(&prev).unwrap();

        assert_eq!(config.db_path, "test.db");
        assert_eq!(config.query, "SELECT * FROM test");
        assert_eq!(config.template_path, "test.hbs");
        assert_eq!(config.query_params.len(), 1);
    }

    #[test]
    fn test_module_config_merge_preserves_existing() {
        let mut config = ModuleConfig {
            db_path: "existing.db".to_string(),
            query: "SELECT 1".to_string(),
            template_path: "existing.hbs".to_string(),
            query_params: vec![],
        };

        let prev = ModuleConfig {
            db_path: "prev.db".to_string(),
            query: "SELECT 2".to_string(),
            template_path: "prev.hbs".to_string(),
            query_params: vec![],
        };

        config.merge(&prev).unwrap();

        // Should keep existing values
        assert_eq!(config.db_path, "existing.db");
        assert_eq!(config.query, "SELECT 1");
        assert_eq!(config.template_path, "existing.hbs");
    }

    #[test]
    fn test_main_config_default() {
        let config = MainConfig::default();
        assert!(config.global_templates_dir.is_empty());
    }

    #[test]
    fn test_main_config_merge() {
        let mut config = MainConfig {
            global_templates_dir: String::new(),
        };

        let prev = MainConfig {
            global_templates_dir: "templates/global".to_string(),
        };

        config.merge(&prev).unwrap();
        assert_eq!(config.global_templates_dir, "templates/global");
    }
}

