//! Adapter implementations for domain traits (imperative shell)

use crate::domain::{QueryExecutor, VariableResolver};
use crate::query;
use crate::types::{DatabasePath, SqlQuery};
use crate::variable;
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
    fn resolve(&mut self, var_name: &str) -> Result<String, String> {
        variable::resolve_variable(self.request, var_name)
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
}
