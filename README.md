# nginx-test - SQLite Module for NGINX

A dynamic NGINX module written in Rust that integrates SQLite databases with Handlebars templating, enabling data-driven web applications directly from NGINX configuration.

## Features

✅ **SQLite Integration** - Query SQLite databases from NGINX  
✅ **Handlebars Templates** - Render dynamic HTML with template inheritance  
✅ **Parameterized Queries** - Safe SQL parameters from nginx variables  
✅ **Global & Local Templates** - Template reuse with override support  
✅ **Zero Application Server** - Serve data-driven pages directly from NGINX  

## Quick Start

### 1. Build the Module

```bash
direnv exec "$PWD" cargo build
```

### 2. Run the Book Catalog Example

```bash
./start_book_catalog.sh
```

Visit http://localhost:8080/books/all

### 3. Run the Parameters Example

```bash
./start_book_detail.sh
```

Visit http://localhost:8081/book?id=1

## Examples

### Example 1: Book Catalog (Port 8080)

A full-featured catalog with category browsing, global templates, and responsive UI.

**Features:**
- Browse all books or filter by category
- Shared header/footer/card templates
- Modern gradient UI design
- Multiple category pages

**See:** `conf/book_catalog.conf` and `README_BOOK_CATALOG.md`

### Example 2: Parameterized Queries (Port 8081)

Demonstrates dynamic SQL queries with nginx variables.

**Features:**
- Book detail pages by ID
- Genre filtering with query parameters
- Year range searches with multiple parameters
- Safe prepared statement parameter binding

**See:** `conf/book_detail.conf` and `README_PARAMETERS.md`

## Configuration Directives

### `sqlite_db`
Set the SQLite database file path.

**Syntax:** `sqlite_db path;`  
**Context:** `location`

### `sqlite_query`
Define the SQL SELECT query to execute.

**Syntax:** `sqlite_query "SELECT ...";`  
**Context:** `location`  
**Notes:** Use `?` placeholders for parameters

### `sqlite_template`
Specify the Handlebars template file (relative to location path).

**Syntax:** `sqlite_template filename.hbs;`  
**Context:** `location`  
**Notes:** Sets the content handler for the location

### `sqlite_param`
Add a parameter to the SQL query (can be used multiple times).

**Syntax:** `sqlite_param $variable_or_value;`  
**Context:** `location`  
**Notes:** Order matches `?` placeholders in query

### `sqlite_global_templates`
Set a directory for global template files (partials, layouts).

**Syntax:** `sqlite_global_templates directory;`  
**Context:** `http`

## Basic Example

```nginx
http {
    sqlite_global_templates "templates/global";
    
    server {
        listen 8080;
        root "public";
        
        # Simple query without parameters
        location = /books {
            sqlite_db "catalog.db";
            sqlite_query "SELECT * FROM books ORDER BY title";
            sqlite_template "list.hbs";
        }
        
        # Parameterized query
        location = /book {
            sqlite_db "catalog.db";
            sqlite_query "SELECT * FROM books WHERE id = ?";
            sqlite_param $arg_id;
            sqlite_template "detail.hbs";
        }
    }
}
```

## Template System

### Template Resolution

Templates are resolved as: `{document_root}{uri}/{template_name}`

Example:
- `root "public"`
- `location /books`
- `sqlite_template "list.hbs"`
- Resolved to: `public/books/list.hbs`

### Global Templates

Place shared templates (headers, footers, partials) in a global directory:

```nginx
http {
    sqlite_global_templates "templates/shared";
}
```

All `.hbs` files in this directory are automatically loaded as partials (referenced without `.hbs` extension):

```handlebars
{{> header}}
<div class="content">
    {{#each results}}
        {{> card}}
    {{/each}}
</div>
{{> footer}}
```

### Local Templates

Each location can have its own template directory. Local templates override global ones with the same name.

**Directory structure:**
```
public/
├── global/           # Global templates
│   ├── header.hbs
│   └── footer.hbs
└── books/
    ├── list.hbs      # Main template
    └── card.hbs      # Local partial (overrides global if exists)
```

### Template Data

Query results are passed to templates as a `results` array:

```handlebars
<h1>Books ({{results.length}} total)</h1>
<ul>
{{#each results}}
    <li>{{title}} by {{author}} ({{year}})</li>
{{/each}}
</ul>
```

## SQL Query Results

Results are converted to JSON format:

| SQLite Type | JSON Type |
|-------------|-----------|
| NULL        | `null` |
| INTEGER     | Number |
| REAL        | Number |
| TEXT        | String |
| BLOB        | String (hex-encoded) |

## Development

### Build

```bash
direnv exec "$PWD" cargo build
```

### Test

```bash
# Run nginx with configuration
./ngx_src/nginx-1.28.0/objs/nginx -c conf/book_catalog.conf -p .

# Test endpoint
curl http://localhost:8080/books/all

# Stop nginx
./ngx_src/nginx-1.28.0/objs/nginx -s stop -c conf/book_catalog.conf -p .
```

### Debug

Enable debug logging in nginx configuration:

```nginx
error_log logs/error.log debug;
```

Then check the logs:

```bash
tail -f logs/error.log | grep sqlite
```

## Architecture

```
Request → NGINX → Module Handler → SQLite Query
                         ↓
                  Resolve Variables
                         ↓
                  Execute Prepared Statement
                         ↓
                  Load Templates (Global + Local)
                         ↓
                  Render with Handlebars
                         ↓
                  Return HTML Response
```

## Project Structure

```
nginx-test/
├── src/
│   └── lib.rs                     # Module implementation
├── conf/
│   ├── book_catalog.conf          # Static catalog example
│   └── book_detail.conf           # Parameterized queries example
├── server_root/
│   ├── global_templates/          # Shared templates
│   │   ├── header.hbs
│   │   ├── footer.hbs
│   │   └── book_card.hbs
│   ├── books/                     # Category pages
│   ├── book/                      # Detail pages
│   └── genre/                     # Genre pages
├── book_catalog.db                # Sample database
├── setup_book_catalog.sh          # Database setup script
├── start_book_catalog.sh          # Start catalog server
├── start_book_detail.sh           # Start parameters example
├── README_BOOK_CATALOG.md         # Catalog example docs
└── README_PARAMETERS.md           # Parameters feature docs
```

## Dependencies

- **Rust** - 2024 edition
- **ngx** (0.5.0) - Rust bindings for NGINX
- **rusqlite** (0.37.0) - SQLite integration
- **handlebars** (6.3.2) - Template engine
- **serde** & **serde_json** - JSON serialization

## License

See LICENSE file for details.

## Resources

- [NGINX Module Development Guide](https://nginx.org/en/docs/dev/development_guide.html)
- [ngx Rust Crate](https://crates.io/crates/ngx)
- [Handlebars Rust](https://crates.io/crates/handlebars)
- [Rusqlite](https://crates.io/crates/rusqlite)

