#!/usr/bin/env bash
# Production start script for sqlite-serve

set -e

echo "🚀 Starting sqlite-serve..."
echo ""

# Check if database exists
if [ ! -f "book_catalog.db" ]; then
    echo "📊 Database not found. Running setup..."
    ./setup_book_catalog.sh
    echo ""
fi

# Build if needed
if [ ! -f "target/debug/libsqlite_serve.dylib" ]; then
    echo "🔨 Building module..."
    direnv exec "$PWD" cargo build
    echo ""
fi

# Stop any existing instance
./ngx_src/nginx-1.28.0/objs/nginx -s stop -c conf/sqlite_serve.conf -p . 2>/dev/null || true
sleep 1

# Start nginx
echo "▶️  Starting NGINX on http://localhost:8080"
./ngx_src/nginx-1.28.0/objs/nginx -c conf/sqlite_serve.conf -p .

echo ""
echo "✅ sqlite-serve is running!"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Endpoints:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  📚 Browse"
echo "     http://localhost:8080/books"
echo "     → All books in catalog"
echo ""
echo "  🔍 Search"
echo "     http://localhost:8080/search?q=Rust"
echo "     → Search by title"
echo ""
echo "  📖 Book Detail"
echo "     http://localhost:8080/book?id=1"
echo "     → View individual book"
echo ""
echo "  🏷️  Filter by Genre"
echo "     http://localhost:8080/genre?genre=Programming"
echo "     → Programming books"
echo ""
echo "  ⭐ Top Rated"
echo "     http://localhost:8080/top?min=4.7"
echo "     → Books rated 4.7 or higher"
echo ""
echo "  📅 By Era"
echo "     http://localhost:8080/era?from=2015&to=2024"
echo "     → Books from 2015-2024"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Features Demonstrated:"
echo "  ✓ SQLite integration with parameterized queries"
echo "  ✓ Named & positional SQL parameters"
echo "  ✓ Handlebars templates with inheritance"
echo "  ✓ Global & local template overrides"
echo "  ✓ Type-safe configuration (Parse, Don't Validate)"
echo "  ✓ Dependency injection architecture"
echo "  ✓ 103 unit tests"
echo ""
echo "To stop: ./ngx_src/nginx-1.28.0/objs/nginx -s stop -c conf/sqlite_serve.conf -p ."
echo ""

