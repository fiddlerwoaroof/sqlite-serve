#!/usr/bin/env bash
# Setup script for book catalog example

set -e

echo "Setting up book catalog database..."

# Create the database
rm -f book_catalog.db
sqlite3 book_catalog.db <<EOF
-- Create books table
CREATE TABLE books (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    author TEXT NOT NULL,
    isbn TEXT,
    year INTEGER,
    genre TEXT,
    description TEXT,
    rating REAL
);

-- Insert sample book data
INSERT INTO books (title, author, isbn, year, genre, description, rating) VALUES
('The Pragmatic Programmer', 'Andrew Hunt, David Thomas', '978-0135957059', 2019, 'Programming', 'Your journey to mastery in software craftsmanship.', 4.5),
('Clean Code', 'Robert C. Martin', '978-0132350884', 2008, 'Programming', 'A handbook of agile software craftsmanship.', 4.7),
('Design Patterns', 'Gang of Four', '978-0201633610', 1994, 'Programming', 'Elements of reusable object-oriented software.', 4.6),
('The Rust Programming Language', 'Steve Klabnik, Carol Nichols', '978-1718503106', 2023, 'Programming', 'The official book on the Rust programming language.', 4.8),
('Structure and Interpretation of Computer Programs', 'Harold Abelson, Gerald Jay Sussman', '978-0262510871', 1996, 'Computer Science', 'Classic text on programming and computer science.', 4.9),
('Introduction to Algorithms', 'Thomas H. Cormen et al.', '978-0262033848', 2009, 'Computer Science', 'Comprehensive algorithms textbook.', 4.6),
('Code Complete', 'Steve McConnell', '978-0735619678', 2004, 'Programming', 'A practical handbook of software construction.', 4.5),
('Designing Data-Intensive Applications', 'Martin Kleppmann', '978-1449373320', 2017, 'Databases', 'The big ideas behind reliable, scalable, and maintainable systems.', 4.8),
('Database System Concepts', 'Abraham Silberschatz et al.', '978-0078022159', 2019, 'Databases', 'Comprehensive introduction to database systems.', 4.4),
('The Art of Computer Programming, Vol. 1', 'Donald Knuth', '978-0201896831', 1997, 'Computer Science', 'Fundamental algorithms and analysis.', 4.7);

-- Create a view for books by genre
CREATE VIEW books_by_genre AS
SELECT genre, COUNT(*) as count, AVG(rating) as avg_rating
FROM books
GROUP BY genre
ORDER BY count DESC;

EOF

echo "Database created: book_catalog.db"
echo ""
echo "Sample data:"
sqlite3 book_catalog.db "SELECT COUNT(*) as total_books FROM books;"
echo ""
echo "Books by genre:"
sqlite3 book_catalog.db "SELECT * FROM books_by_genre;"

