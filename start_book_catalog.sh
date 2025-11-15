#!/usr/bin/env bash
# Quick start script for the book catalog example

set -e

echo "📚 Starting Book Catalog..."
echo ""

# Check if database exists
if [ ! -f "book_catalog.db" ]; then
    echo "Database not found. Running setup..."
    ./setup_book_catalog.sh
    echo ""
fi

# Check if module is built
if [ ! -f "target/debug/libsqlite_serve.dylib" ]; then
    echo "Module not built. Building..."
    direnv exec "$PWD" cargo build
    echo ""
fi

# Start nginx
echo "Starting nginx on http://localhost:8080"
./ngx_src/nginx-1.28.0/objs/nginx -c conf/book_catalog.conf -p .

echo ""
echo "✅ Book Catalog is running!"
echo ""
echo "Visit:"
echo "  • http://localhost:8080/books/all           - All books"
echo "  • http://localhost:8080/books/programming   - Programming books"
echo "  • http://localhost:8080/books/databases     - Database books"
echo "  • http://localhost:8080/books/computer-science - CS books"
echo ""
echo "To stop: ./ngx_src/nginx-1.28.0/objs/nginx -s stop -c conf/book_catalog.conf -p ."

