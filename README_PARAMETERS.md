# Path Parameters Feature

The nginx-test SQLite module supports parameterized SQL queries using nginx variables. This allows you to pass dynamic values from the request (query parameters, path captures, headers, etc.) as safe SQL prepared statement parameters.

## New Directive

### `sqlite_param`

Add parameters to SQL queries. Can be used multiple times to add multiple parameters.

**Syntax:** `sqlite_param variable_or_value;`  
**Context:** `location`  
**Multiple:** Yes (order matches `?` placeholders in query)

## Usage

### Query Parameters (Most Common)

Use nginx's built-in `$arg_*` variables to access query parameters:

```nginx
location = /book {
    sqlite_db "book_catalog.db";
    sqlite_query "SELECT * FROM books WHERE id = ?";
    sqlite_param $arg_id;              # Gets ?id=123 from URL
    sqlite_template "detail.hbs";
}
```

**Request:** `http://localhost/book?id=5`  
**SQL Executed:** `SELECT * FROM books WHERE id = '5'`

### Multiple Parameters

Parameters are bound to `?` placeholders in order:

```nginx
location = /years {
    sqlite_db "book_catalog.db";
    sqlite_query "SELECT * FROM books WHERE year >= ? AND year <= ?";
    sqlite_param $arg_min;             # First ? placeholder
    sqlite_param $arg_max;             # Second ? placeholder
    sqlite_template "list.hbs";
}
```

**Request:** `http://localhost/years?min=2015&max=2024`  
**SQL Executed:** `SELECT * FROM books WHERE year >= '2015' AND year <= '2024'`

### Regex Path Captures

Use numbered captures (`$1`, `$2`, etc.) from regex locations:

```nginx
location ~ ^/book/([0-9]+)$ {
    sqlite_db "book_catalog.db";
    sqlite_query "SELECT * FROM books WHERE id = ?";
    sqlite_param $1;                   # First capture group
    sqlite_template "detail.hbs";
}
```

**Request:** `http://localhost/book/5`  
**SQL Executed:** `SELECT * FROM books WHERE id = '5'`

### Named Captures

Use named captures from regex locations:

```nginx
location ~ ^/author/(?<author_name>[^/]+)/books$ {
    sqlite_db "book_catalog.db";
    sqlite_query "SELECT * FROM books WHERE author LIKE ?";
    sqlite_param $author_name;
    sqlite_template "list.hbs";
}
```

**Request:** `http://localhost/author/Martin/books`  
**SQL Executed:** `SELECT * FROM books WHERE author LIKE 'Martin'`

### Other Nginx Variables

Any nginx variable can be used as a parameter:

```nginx
location = /search {
    sqlite_db "book_catalog.db";
    sqlite_query "SELECT * FROM books WHERE title LIKE '%' || ? || '%'";
    sqlite_param $arg_q;               # Query string parameter
    sqlite_template "search.hbs";
}

location = /client-info {
    sqlite_db "access_log.db";
    sqlite_query "INSERT INTO visits (ip, user_agent) VALUES (?, ?)";
    sqlite_param $remote_addr;         # Client IP
    sqlite_param $http_user_agent;     # User agent header
    sqlite_template "logged.hbs";
}
```

### Literal Values

You can also use literal values (though less common):

```nginx
location = /featured {
    sqlite_db "book_catalog.db";
    sqlite_query "SELECT * FROM books WHERE rating >= ? ORDER BY rating DESC";
    sqlite_param "4.5";                # Literal value
    sqlite_template "list.hbs";
}
```

## Available Nginx Variables

Common nginx variables you can use as parameters:

### Query String
- `$arg_name` - Query parameter (e.g., `?name=value`)
- `$args` - Full query string
- `$query_string` - Same as `$args`

### Request Info
- `$request_method` - GET, POST, etc.
- `$request_uri` - Full request URI with query string
- `$uri` - Request URI without query string
- `$document_uri` - Same as `$uri`

### Client Info
- `$remote_addr` - Client IP address
- `$remote_port` - Client port
- `$remote_user` - HTTP basic auth username

### Headers
- `$http_name` - Any HTTP header (e.g., `$http_user_agent`, `$http_referer`)
- `$content_type` - Content-Type header
- `$content_length` - Content-Length header

### Path Captures
- `$1`, `$2`, ..., `$9` - Numbered regex captures
- `$name` - Named regex captures (`(?<name>...)`)

### Server Info
- `$server_name` - Server name
- `$server_port` - Server port
- `$scheme` - http or https
- `$hostname` - Hostname

See [nginx variables documentation](http://nginx.org/en/docs/http/ngx_http_core_module.html#variables) for complete list.

## Security

**SQL Injection Protection:**
- All parameters are passed through SQLite's prepared statement mechanism
- Values are properly escaped and quoted by SQLite
- **SAFE:** `sqlite_param $arg_id` with query `SELECT * FROM books WHERE id = ?`
- **SAFE:** Multiple parameters are bound separately to each `?`

**Never concatenate variables into the query string:**
- **UNSAFE:** `sqlite_query "SELECT * FROM books WHERE id = $arg_id"`  ❌
- **SAFE:** Use `sqlite_param` instead  ✓

## Examples

### Book Detail Page

```nginx
location = /book {
    sqlite_db "catalog.db";
    sqlite_query "SELECT * FROM books WHERE id = ?";
    sqlite_param $arg_id;
    sqlite_template "detail.hbs";
}
```

Visit: `http://localhost/book?id=42`

### Search by Multiple Criteria

```nginx
location = /search {
    sqlite_db "catalog.db";
    sqlite_query "
        SELECT * FROM books 
        WHERE title LIKE '%' || ? || '%' 
        AND year >= ? 
        AND rating >= ?
        ORDER BY rating DESC
    ";
    sqlite_param $arg_title;
    sqlite_param $arg_year;
    sqlite_param $arg_rating;
    sqlite_template "results.hbs";
}
```

Visit: `http://localhost/search?title=rust&year=2020&rating=4.5`

### Category with Pagination

```nginx
location = /category {
    sqlite_db "catalog.db";
    sqlite_query "
        SELECT * FROM books 
        WHERE genre = ? 
        ORDER BY title
        LIMIT ? OFFSET ?
    ";
    sqlite_param $arg_genre;
    sqlite_param $arg_limit;
    sqlite_param $arg_offset;
    sqlite_template "list.hbs";
}
```

Visit: `http://localhost/category?genre=Programming&limit=10&offset=0`

## Error Handling

### Missing Parameters

If a required nginx variable is not set, the module returns `400 Bad Request`:

```nginx
location = /book {
    sqlite_param $arg_id;  # If ?id= is not provided
}
```

**Response:** 400 Bad Request

### Invalid SQL

If parameter values cause SQL errors (e.g., type mismatch), returns `500 Internal Server Error`:

```nginx
sqlite_query "SELECT * FROM books WHERE id = ?";
sqlite_param $arg_id;  # If ?id=abc (not a number)
```

**Response:** 500 Internal Server Error (check nginx error log)

### Variable Not Found

If a variable name doesn't exist in nginx, returns `400 Bad Request` with log message.

## Complete Example

See `conf/book_detail.conf` for a working example with:
- Single parameter (book by ID)
- String parameter (genre filtering)
- Multiple parameters (year range search)

Run it with:
```bash
./start_book_detail.sh
```

## Implementation Details

- Parameters are resolved at request time using `ngx_http_get_variable()`
- UTF-8 validation is performed on all variable values
- Parameters are bound using rusqlite's prepared statement API
- All SQL placeholders must be `?` (positional parameters)
- Parameters match placeholders in order of `sqlite_param` directives

## Limitations

- Only supports `?` positional parameters (not named parameters like `:name`)
- Parameters must be provided in the exact order they appear in the query
- All parameter values are treated as strings (SQLite performs type coercion)
- Complex SQL values (arrays, JSON) should be constructed in the query itself

