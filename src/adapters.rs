//! Adapter implementations for domain traits (imperative shell)

use crate::domain::{QueryExecutor, TemplateLoader, TemplateRenderer, VariableResolver};
use crate::query;
use crate::template;
use crate::types::{DatabasePath, SqlQuery};
use crate::variable;
use handlebars::Handlebars;
use ngx::http::Request;
use serde_json::Value;
use std::collections::HashMap;

/// Adapter for nginx variable resolution
pub struct NginxVariableResolver<'a> {
    request: &'a mut Request,
}

impl<'a> NginxVariableResolver<'a> {
    pub fn new(request: &'a mut Request) -> Self {
        NginxVariableResolver { request }
    }
}

impl<'a> VariableResolver for NginxVariableResolver<'a> {
    fn resolve(&self, var_name: &str) -> Result<String, String> {
        // SAFETY: We need mutable access but trait requires &self
        // This is safe because nginx variables are read-only from our perspective
        let request_ptr = self.request as *const Request as *mut Request;
        let request = unsafe { &mut *request_ptr };
        variable::resolve_variable(request, var_name)
    }
}

/// Adapter for SQLite query execution
pub struct SqliteQueryExecutor;

impl QueryExecutor for SqliteQueryExecutor {
    fn execute(
        &self,
        db_path: &DatabasePath,
        query: &SqlQuery,
        params: &[(String, String)],
    ) -> Result<Vec<HashMap<String, Value>>, String> {
        query::execute_query(db_path.as_str(), query.as_str(), params).map_err(|e| e.to_string())
    }
}

/// Adapter for Handlebars template operations (using raw pointer for interior mutability)
#[derive(Clone)]
pub struct HandlebarsAdapter {
    registry: *mut Handlebars<'static>,
}

impl HandlebarsAdapter {
    /// Create adapter from mutable handlebars registry
    ///
    /// # Safety
    /// Caller must ensure the registry outlives this adapter
    pub unsafe fn new(registry: *mut Handlebars<'static>) -> Self {
        HandlebarsAdapter { registry }
    }
}

impl TemplateLoader for HandlebarsAdapter {
    fn load_from_dir(&self, dir_path: &str) -> Result<usize, String> {
        unsafe {
            template::load_templates_from_dir(&mut *self.registry, dir_path)
                .map_err(|e| e.to_string())
        }
    }

    fn register_template(&self, name: &str, path: &str) -> Result<(), String> {
        unsafe {
            (*self.registry)
                .register_template_file(name, path)
                .map_err(|e| e.to_string())
        }
    }
}

impl TemplateRenderer for HandlebarsAdapter {
    fn render(&self, template_name: &str, data: &Value) -> Result<String, String> {
        unsafe {
            (*self.registry)
                .render(template_name, data)
                .map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_query_executor() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_adapter_executor.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE test (id INTEGER, name TEXT)", [])
                .unwrap();
            conn.execute("INSERT INTO test VALUES (1, 'test')", [])
                .unwrap();
        }

        let executor = SqliteQueryExecutor;
        let db_path = DatabasePath::parse(temp_path).unwrap();
        let query = SqlQuery::parse("SELECT * FROM test").unwrap();

        let results = executor.execute(&db_path, &query, &[]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].get("name").unwrap(),
            &Value::String("test".to_string())
        );

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_handlebars_adapter() {
        use std::fs;
        use std::io::Write;

        let temp_dir = "/tmp/test_adapter_hbs";
        let _ = fs::remove_dir_all(temp_dir);
        fs::create_dir_all(temp_dir).unwrap();

        let template_path = format!("{}/test.hbs", temp_dir);
        let mut file = fs::File::create(&template_path).unwrap();
        file.write_all(b"Hello {{name}}").unwrap();

        let mut reg = Handlebars::new();
        let reg_ptr: *mut Handlebars<'static> = unsafe { std::mem::transmute(&mut reg) };
        let adapter = unsafe { HandlebarsAdapter::new(reg_ptr) };

        adapter.register_template("test", &template_path).unwrap();

        let data = serde_json::json!({"name": "World"});
        let rendered = adapter.render("test", &data).unwrap();

        assert_eq!(rendered, "Hello World");

        let _ = fs::remove_dir_all(temp_dir);
    }
}
