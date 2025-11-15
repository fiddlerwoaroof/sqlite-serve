# Book Catalog Example

A complete example demonstrating the sqlite-serve module with a read-only book catalog.

## Features

- **SQLite Database**: Stores book information (title, author, ISBN, year, genre, description, rating)
- **Multiple Views**: Different locations for browsing by category
- **Template Inheritance**: Global templates (header, footer, book_card) shared across all pages
- **Local Templates**: Category-specific styling and layouts
- **Responsive Design**: Modern, gradient-styled UI

## Setup

### 1. Create and populate the database

```bash
chmod +x setup_book_catalog.sh
./setup_book_catalog.sh
```

This creates `book_catalog.db` with 10 sample technical books across three genres:
- Programming
- Databases  
- Computer Science

### 2. Build the module

```bash
direnv exec "$PWD" cargo build
```

### 3. Start nginx

```bash
./ngx_src/nginx-1.28.0/objs/nginx -c conf/book_catalog.conf -p .
```

### 4. Visit the catalog

Open your browser to:
- http://localhost:8080/ (redirects to all books)
- http://localhost:8080/books/all
- http://localhost:8080/books/programming
- http://localhost:8080/books/databases
- http://localhost:8080/books/computer-science

### 5. Stop nginx

```bash
./ngx_src/nginx-1.28.0/objs/nginx -s stop -c conf/book_catalog.conf -p .
```

## Directory Structure

```
sqlite-serve/
├── book_catalog.db                      # SQLite database
├── setup_book_catalog.sh                # Database setup script
├── conf/
│   └── book_catalog.conf                # Nginx configuration
└── server_root/
    ├── global_templates/                # Shared templates
    │   ├── header.hbs                   # Page header with navigation
    │   ├── footer.hbs                   # Page footer
    │   └── book_card.hbs                # Reusable book card partial
    └── books/
        ├── all/
        │   └── list.hbs                 # All books page
        ├── programming/
        │   └── list.hbs                 # Programming books page
        ├── databases/
        │   └── list.hbs                 # Database books page
        └── computer-science/
            └── list.hbs                 # CS books page
```

## How It Works

### Template Loading Order

For each request, the module:

1. **Loads global templates** from `server_root/global_templates/`:
   - `header.hbs` - Page structure and navigation
   - `footer.hbs` - Page footer
   - `book_card.hbs` - Book display component

2. **Loads local templates** from the location's directory:
   - Each category has its own `list.hbs` with custom styling
   - Local templates can override global ones

3. **Renders the main template** with SQL query results

### Template Usage

**In list.hbs:**
```handlebars
{{> header}}

<div class="book-grid">
    {{#each results}}
        {{> book_card}}
    {{/each}}
</div>

{{> footer}}
```

The `{{> header}}`, `{{> book_card}}`, and `{{> footer}}` are partials loaded from the global templates directory.

### SQL Queries

Each location runs a different SQL query:

- **All books**: `SELECT * FROM books ORDER BY rating DESC, title`
- **Programming**: `SELECT * FROM books WHERE genre = 'Programming' ...`
- **Databases**: `SELECT * FROM books WHERE genre = 'Databases' ...`
- **Computer Science**: `SELECT * FROM books WHERE genre = 'Computer Science' ...`

Results are passed to the template as a `results` array.

## Customization

### Adding More Books

```bash
sqlite3 book_catalog.db
```

```sql
INSERT INTO books (title, author, isbn, year, genre, description, rating) 
VALUES ('Your Book', 'Author Name', '978-XXXXXXXXXX', 2024, 'Programming', 'Description here', 4.5);
```

### Adding New Categories

1. Create a new genre in the database
2. Add a location block in `book_catalog.conf`
3. Create a template directory under `server_root/books/`
4. Add the category to the navigation in `header.hbs`

### Styling

Each category's `list.hbs` contains embedded CSS. Modify the `<style>` section to change colors, layouts, etc.

## Architecture Notes

- **Read-only**: All SQL queries are SELECT statements only
- **Performance**: Templates are loaded fresh for each request (suitable for development)
- **Security**: No user input is processed; all queries are predefined
- **Scalability**: SQLite is suitable for read-heavy workloads with moderate traffic

Enjoy exploring the book catalog! 📚

