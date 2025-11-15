//! Template loading and management

use handlebars::Handlebars;
use std::path::Path;

/// Load all .hbs templates from a directory into the Handlebars registry
///
/// Each template is registered by its filename (without .hbs extension).
/// Returns the number of templates successfully loaded.
pub fn load_templates_from_dir(reg: &mut Handlebars, dir_path: &str) -> std::io::Result<usize> {
    use std::fs;

    let dir = Path::new(dir_path);
    if !dir.exists() || !dir.is_dir() {
        return Ok(0);
    }

    let mut count = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "hbs" {
                    if let Some(stem) = path.file_stem() {
                        if let Some(name) = stem.to_str() {
                            if let Err(e) = reg.register_template_file(name, &path) {
                                eprintln!("Failed to register template {}: {}", path.display(), e);
                            } else {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_templates_from_nonexistent_dir() {
        let mut reg = Handlebars::new();
        let result = load_templates_from_dir(&mut reg, "/nonexistent/path/to/templates");

        // Should succeed but load 0 templates
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_load_templates_from_dir() {
        use std::fs;
        use std::io::Write;

        let temp_dir = "/tmp/test_sqlite_serve_templates";
        let _ = fs::remove_dir_all(temp_dir);
        fs::create_dir_all(temp_dir).unwrap();

        // Create test templates
        let mut file1 = fs::File::create(format!("{}/template1.hbs", temp_dir)).unwrap();
        file1.write_all(b"<h1>Template 1</h1>").unwrap();

        let mut file2 = fs::File::create(format!("{}/template2.hbs", temp_dir)).unwrap();
        file2.write_all(b"<h1>Template 2</h1>").unwrap();

        // Create a non-template file (should be ignored)
        let mut file3 = fs::File::create(format!("{}/readme.txt", temp_dir)).unwrap();
        file3.write_all(b"Not a template").unwrap();

        let mut reg = Handlebars::new();
        let count = load_templates_from_dir(&mut reg, temp_dir).unwrap();

        assert_eq!(count, 2);
        assert!(reg.has_template("template1"));
        assert!(reg.has_template("template2"));
        assert!(!reg.has_template("readme"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_template_rendering_with_results() {
        use serde_json::json;
        use std::fs;
        use std::io::Write;

        let temp_dir = "/tmp/test_sqlite_serve_render";
        let _ = fs::remove_dir_all(temp_dir);
        fs::create_dir_all(temp_dir).unwrap();

        let template_path = format!("{}/list.hbs", temp_dir);
        let mut file = fs::File::create(&template_path).unwrap();
        file.write_all(b"{{#each results}}<li>{{name}}</li>{{/each}}")
            .unwrap();

        let mut reg = Handlebars::new();
        reg.register_template_file("list", &template_path).unwrap();

        let mut results = vec![];
        let mut item1 = std::collections::HashMap::new();
        item1.insert(
            "name".to_string(),
            serde_json::Value::String("Item 1".to_string()),
        );
        results.push(item1);

        let mut item2 = std::collections::HashMap::new();
        item2.insert(
            "name".to_string(),
            serde_json::Value::String("Item 2".to_string()),
        );
        results.push(item2);

        let rendered = reg.render("list", &json!({"results": results})).unwrap();
        assert!(rendered.contains("<li>Item 1</li>"));
        assert!(rendered.contains("<li>Item 2</li>"));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_template_override_behavior() {
        use std::fs;
        use std::io::Write;

        let temp_dir = "/tmp/test_sqlite_serve_override";
        let _ = fs::remove_dir_all(temp_dir);
        fs::create_dir_all(temp_dir).unwrap();

        let template1_path = format!("{}/test.hbs", temp_dir);
        let mut file1 = fs::File::create(&template1_path).unwrap();
        file1.write_all(b"Original").unwrap();

        let mut reg = Handlebars::new();
        reg.register_template_file("test", &template1_path).unwrap();

        let rendered1 = reg.render("test", &serde_json::json!({})).unwrap();
        assert_eq!(rendered1, "Original");

        // Override with new content
        let mut file2 = fs::File::create(&template1_path).unwrap();
        file2.write_all(b"Updated").unwrap();

        // Re-register to override
        reg.register_template_file("test", &template1_path).unwrap();

        let rendered2 = reg.render("test", &serde_json::json!({})).unwrap();
        assert_eq!(rendered2, "Updated");

        let _ = fs::remove_dir_all(temp_dir);
    }
}

