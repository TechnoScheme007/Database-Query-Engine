# SQLEngine

An in-memory SQL database engine written in Rust with **zero dependencies**. Features a hand-written tokenizer, recursive-descent parser, and query execution engine with JSON file persistence.

## Features

### SQL Statements
- **CREATE TABLE** — with column types (`INT`, `FLOAT`, `TEXT`, `BOOLEAN`, `VARCHAR`) and `PRIMARY KEY`
- **DROP TABLE**
- **INSERT** — single and multi-row inserts, optional column lists
- **SELECT** — full query support:
  - `WHERE` with complex expressions
  - `ORDER BY` (ASC/DESC, multiple columns)
  - `GROUP BY` with aggregate functions
  - `HAVING` clause
  - `LIMIT` and `OFFSET`
  - `DISTINCT`
  - Column aliases (`AS`)
- **JOIN** — `INNER JOIN`, `LEFT JOIN`, `RIGHT JOIN` with `ON` conditions
- **UPDATE** — with `SET` and optional `WHERE`
- **DELETE** — with optional `WHERE`

### Expressions & Operators
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `=`, `!=`, `<>`, `<`, `>`, `<=`, `>=`
- Logical: `AND`, `OR`, `NOT`
- `IS NULL` / `IS NOT NULL`
- `IN (value_list)`
- `BETWEEN a AND b`
- `LIKE` with `%` and `_` wildcards
- Parenthesized expressions

### Aggregate Functions
`COUNT(*)`, `COUNT(column)`, `COUNT(DISTINCT column)`, `SUM()`, `AVG()`, `MIN()`, `MAX()`

### Scalar Functions
`UPPER()`, `LOWER()`, `LENGTH()`, `ABS()`, `COALESCE()`

### Persistence
- Auto-saves to `database.json` on exit
- Auto-loads on startup
- Custom file path via command-line argument

## Build & Run

```bash
cargo build --release
cargo run --release

# Or with a custom database file:
cargo run --release -- mydata.json
```

## Interactive REPL

```
SQLEngine v0.1.0 — In-memory SQL database
Type SQL statements, or use these commands:
  .tables    — list all tables
  .schema    — show table schemas
  .save      — save database to file
  .quit      — exit (auto-saves)

sql> CREATE TABLE users (id INT PRIMARY KEY, name TEXT, age INT);
Table 'users' created

sql> INSERT INTO users VALUES (1, 'Alice', 30), (2, 'Bob', 25), (3, 'Charlie', 35);
3 row(s) inserted

sql> SELECT name, age FROM users WHERE age > 25 ORDER BY age DESC;
 name    | age
---------+----
 Charlie | 35
 Alice   | 30
(2 rows)

sql> SELECT COUNT(*) AS total, AVG(age) AS avg_age FROM users;
 total | avg_age
-------+--------
 3     | 30
(1 rows)
```

### Join Example

```sql
CREATE TABLE orders (id INT, user_id INT, amount INT);
INSERT INTO orders VALUES (1, 1, 100), (2, 1, 200), (3, 2, 150);

SELECT u.name, SUM(o.amount) AS total
FROM users u
INNER JOIN orders o ON u.id = o.user_id
GROUP BY u.name
ORDER BY total DESC;
```

## Architecture

```
src/
  main.rs        — REPL and CLI entry point
  lib.rs         — module declarations
  tokenizer.rs   — SQL lexer (keywords, literals, operators)
  ast.rs         — Abstract syntax tree types
  parser.rs      — Recursive-descent SQL parser
  engine.rs      — Query executor (filtering, joins, aggregation, sorting)
  storage.rs     — JSON serialization/deserialization (hand-written)
```

All components are built from scratch with no external crates.

## Running Tests

```bash
cargo test
```

26 tests cover the tokenizer, parser, engine (all statement types, joins, aggregation, LIKE, BETWEEN, IN, IS NULL, DISTINCT), and storage round-trip.

## License

MIT
