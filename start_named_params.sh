#!/usr/bin/env bash
# Start script for named parameters example

set -e

echo "📚 Starting Named Parameters Example..."
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
echo "Starting nginx on http://localhost:8082"
./ngx_src/nginx-1.28.0/objs/nginx -c conf/book_named_params.conf -p .

echo ""
echo "✅ Named Parameters Example is running!"
echo ""
echo "Named Parameter Examples:"
echo "  • http://localhost:8082/book?id=1"
echo "    Query: SELECT * FROM books WHERE id = :book_id"
echo "    Param: :book_id = \$arg_id"
echo ""
echo "  • http://localhost:8082/genre?genre=Programming"
echo "    Query: ... WHERE genre = :genre_name"
echo "    Param: :genre_name = \$arg_genre"
echo ""
echo "  • http://localhost:8082/years?min=2015&max=2024"
echo "    Query: ... WHERE year >= :min_year AND year <= :max_year"
echo "    Params: :min_year = \$arg_min, :max_year = \$arg_max"
echo ""
echo "  • http://localhost:8082/search?q=Rust"
echo "    Search by title with named parameter"
echo ""
echo "  • http://localhost:8082/top-rated?rating=4.7"
echo "    Filter by minimum rating"
echo ""
echo "To stop: ./ngx_src/nginx-1.28.0/objs/nginx -s stop -c conf/book_named_params.conf -p ."

