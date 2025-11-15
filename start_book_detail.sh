#!/usr/bin/env bash
# Start script for the book detail example with path parameters

set -e

echo "📚 Starting Book Detail Example with Path Parameters..."
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
echo "Starting nginx on http://localhost:8081"
./ngx_src/nginx-1.28.0/objs/nginx -c conf/book_detail.conf -p .

echo ""
echo "✅ Book Detail Example is running!"
echo ""
echo "Try these URLs:"
echo "  • http://localhost:8081/book?id=1           - View book #1"
echo "  • http://localhost:8081/book?id=5           - View book #5"
echo "  • http://localhost:8081/genre?genre=Programming   - Programming books"
echo "  • http://localhost:8081/genre?genre=Databases     - Database books"
echo "  • http://localhost:8081/years?min=2000&max=2010   - Books from 2000-2010"
echo "  • http://localhost:8081/years?min=2015&max=2024   - Books from 2015-2024"
echo ""
echo "To stop: ./ngx_src/nginx-1.28.0/objs/nginx -s stop -c conf/book_detail.conf -p ."

