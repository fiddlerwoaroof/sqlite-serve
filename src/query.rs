//! SQL query execution with parameter binding

use rusqlite::{Connection, Result};
use serde_json::Value;
use std::collections::HashMap;

/// Execute a SQL query with parameters and return results as JSON-compatible data
///
/// Supports both positional (?) and named (:name) parameters.
/// If any parameter has a non-empty name, all parameters are treated as named.
pub fn execute_query(
    db_path: &str,
    query: &str,
    params: &[(String, String)], // (param_name, value) pairs
) -> Result<Vec<HashMap<String, Value>>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(query)?;

    let column_count = stmt.column_count();
    let column_names: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).unwrap_or("").to_string())
        .collect();

    // Bind parameters (either positional or named)
    let has_named_params = params.iter().any(|(name, _)| !name.is_empty());

    // Convert row to JSON map
    let row_to_map = |row: &rusqlite::Row| -> rusqlite::Result<HashMap<String, Value>> {
        let mut map = HashMap::new();
        for (i, col_name) in column_names.iter().enumerate() {
            let value: Value = match row.get_ref(i)? {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(v) => Value::Number(v.into()),
                rusqlite::types::ValueRef::Real(v) => {
                    serde_json::Number::from_f64(v)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
                rusqlite::types::ValueRef::Text(v) => {
                    Value::String(String::from_utf8_lossy(v).to_string())
                }
                rusqlite::types::ValueRef::Blob(v) => {
                    // Convert blob to hex string
                    let hex_string = v.iter().map(|b| format!("{:02x}", b)).collect::<String>();
                    Value::String(hex_string)
                }
            };
            map.insert(col_name.clone(), value);
        }
        Ok(map)
    };

    let rows = if has_named_params {
        // Use named parameters
        let named_params: Vec<(&str, &dyn rusqlite::ToSql)> = params
            .iter()
            .map(|(name, value)| (name.as_str(), value as &dyn rusqlite::ToSql))
            .collect();
        stmt.query_map(named_params.as_slice(), row_to_map)?
    } else {
        // Use positional parameters
        let positional_params: Vec<&dyn rusqlite::ToSql> = params
            .iter()
            .map(|(_, value)| value as &dyn rusqlite::ToSql)
            .collect();
        stmt.query_map(positional_params.as_slice(), row_to_map)?
    };

    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_query_empty_db() {
        // Test with a non-existent database - should return error
        let result = execute_query("/nonexistent/test.db", "SELECT 1", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_query_with_memory_db() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute(
                "CREATE TABLE test (id INTEGER, name TEXT, value REAL)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO test VALUES (1, 'first', 1.5), (2, 'second', 2.5)",
                [],
            )
            .unwrap();
        }

        let results = execute_query(temp_path, "SELECT * FROM test ORDER BY id", &[]).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].get("id").unwrap(),
            &Value::Number(1.into())
        );
        assert_eq!(
            results[0].get("name").unwrap(),
            &Value::String("first".to_string())
        );

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_with_positional_params() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_params.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE books (id INTEGER, title TEXT)", [])
                .unwrap();
            conn.execute(
                "INSERT INTO books VALUES (1, 'Book One'), (2, 'Book Two'), (3, 'Book Three')",
                [],
            )
            .unwrap();
        }

        let params = vec![(String::new(), "2".to_string())];
        let results =
            execute_query(temp_path, "SELECT * FROM books WHERE id = ?", &params).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].get("title").unwrap(),
            &Value::String("Book Two".to_string())
        );

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_with_named_params() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_named.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE books (id INTEGER, title TEXT, year INTEGER)", [])
                .unwrap();
            conn.execute(
                "INSERT INTO books VALUES (1, 'Old Book', 2000), (2, 'New Book', 2020), (3, 'Newer Book', 2023)",
                [],
            )
            .unwrap();
        }

        let params = vec![
            (":min_year".to_string(), "2015".to_string()),
            (":max_year".to_string(), "2024".to_string()),
        ];
        let results = execute_query(
            temp_path,
            "SELECT * FROM books WHERE year >= :min_year AND year <= :max_year ORDER BY year",
            &params,
        )
        .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].get("title").unwrap(),
            &Value::String("New Book".to_string())
        );
        assert_eq!(
            results[1].get("title").unwrap(),
            &Value::String("Newer Book".to_string())
        );

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_data_types() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_types.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute(
                "CREATE TABLE types (id INTEGER, name TEXT, price REAL, data BLOB, nullable TEXT)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO types VALUES (42, 'test', 3.14, X'DEADBEEF', NULL)",
                [],
            )
            .unwrap();
        }

        let results = execute_query(temp_path, "SELECT * FROM types", &[]).unwrap();
        assert_eq!(results.len(), 1);

        let row = &results[0];

        assert_eq!(row.get("id").unwrap(), &Value::Number(42.into()));
        assert_eq!(
            row.get("name").unwrap(),
            &Value::String("test".to_string())
        );
        assert_eq!(row.get("price").unwrap().as_f64().unwrap(), 3.14);
        assert_eq!(
            row.get("data").unwrap(),
            &Value::String("deadbeef".to_string())
        );
        assert_eq!(row.get("nullable").unwrap(), &Value::Null);

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_multiple_named_params() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_multi.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE books (id INTEGER, genre TEXT, rating REAL)", [])
                .unwrap();
            conn.execute(
                "INSERT INTO books VALUES 
                    (1, 'Fiction', 4.5),
                    (2, 'Science', 4.8),
                    (3, 'Fiction', 4.9),
                    (4, 'Science', 4.2)",
                [],
            )
            .unwrap();
        }

        let params = vec![
            (":min_rating".to_string(), "4.5".to_string()),
            (":genre".to_string(), "Fiction".to_string()),
        ];

        let results = execute_query(
            temp_path,
            "SELECT * FROM books WHERE genre = :genre AND rating >= :min_rating ORDER BY rating DESC",
            &params,
        )
        .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].get("rating").unwrap().as_f64().unwrap(), 4.9);
        assert_eq!(results[1].get("rating").unwrap().as_f64().unwrap(), 4.5);

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_with_like_operator() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_like.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE books (title TEXT)", []).unwrap();
            conn.execute(
                "INSERT INTO books VALUES ('The Rust Book'), ('Clean Code'), ('Rust in Action')",
                [],
            )
            .unwrap();
        }

        let params = vec![(":search".to_string(), "Rust".to_string())];
        let results = execute_query(
            temp_path,
            "SELECT * FROM books WHERE title LIKE '%' || :search || '%'",
            &params,
        )
        .unwrap();

        assert_eq!(results.len(), 2);

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_empty_results() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_empty.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE test (id INTEGER)", []).unwrap();
        }

        let results = execute_query(temp_path, "SELECT * FROM test", &[]).unwrap();
        assert_eq!(results.len(), 0);

        let _ = fs::remove_file(temp_path);
    }
}

