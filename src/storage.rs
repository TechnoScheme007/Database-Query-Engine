use crate::ast::{ColumnDef, DataType};
use crate::engine::{Database, Table, TableSchema, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write as IoWrite, BufWriter};
use std::path::Path;

/// JSON-based persistence for the database.
///
/// Format:
/// {
///   "tables": {
///     "table_name": {
///       "columns": [{"name": "id", "type": "INT", "primary_key": false}, ...],
///       "rows": [[1, "hello", true, null], ...]
///     }
///   }
/// }
///
/// Hand-rolled JSON serializer/deserializer — zero dependencies.

pub fn save_to_file(db: &Database, path: &str) -> io::Result<()> {
    let file = fs::File::create(path)?;
    let mut w = BufWriter::new(file);

    write!(w, "{{\n  \"tables\": {{\n")?;
    let table_count = db.tables.len();
    for (i, (name, table)) in db.tables.iter().enumerate() {
        write!(w, "    \"{}\": {{\n", escape_json(name))?;

        // Columns
        write!(w, "      \"columns\": [")?;
        for (j, col) in table.schema.columns.iter().enumerate() {
            if j > 0 { write!(w, ", ")?; }
            write!(w, "{{\"name\": \"{}\", \"type\": \"{}\", \"primary_key\": {}}}",
                escape_json(&col.name),
                datatype_str(&col.data_type),
                col.primary_key,
            )?;
        }
        write!(w, "],\n")?;

        // Rows
        write!(w, "      \"rows\": [\n")?;
        for (j, row) in table.rows.iter().enumerate() {
            write!(w, "        [")?;
            for (k, val) in row.iter().enumerate() {
                if k > 0 { write!(w, ", ")?; }
                write_json_value(&mut w, val)?;
            }
            write!(w, "]")?;
            if j + 1 < table.rows.len() { write!(w, ",")?; }
            write!(w, "\n")?;
        }
        write!(w, "      ]\n")?;

        write!(w, "    }}")?;
        if i + 1 < table_count { write!(w, ",")?; }
        write!(w, "\n")?;
    }
    write!(w, "  }}\n}}\n")?;
    w.flush()?;
    Ok(())
}

pub fn load_from_file(path: &str) -> io::Result<Database> {
    if !Path::new(path).exists() {
        return Ok(Database::new());
    }
    let content = fs::read_to_string(path)?;
    parse_database_json(&content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_json_value(w: &mut impl IoWrite, val: &Value) -> io::Result<()> {
    match val {
        Value::Integer(n) => write!(w, "{}", n),
        Value::Float(n) => write!(w, "{}", n),
        Value::Text(s) => write!(w, "\"{}\"", escape_json(s)),
        Value::Boolean(b) => write!(w, "{}", b),
        Value::Null => write!(w, "null"),
    }
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn datatype_str(dt: &DataType) -> &'static str {
    match dt {
        DataType::Int => "INT",
        DataType::Float => "FLOAT",
        DataType::Text => "TEXT",
        DataType::Boolean => "BOOLEAN",
    }
}

// ── Minimal JSON parser ─────────────────────────────────────────

struct JsonParser {
    chars: Vec<char>,
    pos: usize,
}

#[derive(Debug, Clone)]
enum JsonValue {
    Obj(Vec<(String, JsonValue)>),
    Arr(Vec<JsonValue>),
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

impl JsonParser {
    fn new(input: &str) -> Self {
        Self { chars: input.chars().collect(), pos: 0 }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied();
        self.pos += 1;
        ch
    }

    fn expect_char(&mut self, ch: char) -> Result<(), String> {
        self.skip_ws();
        match self.advance() {
            Some(c) if c == ch => Ok(()),
            Some(c) => Err(format!("Expected '{}', got '{}'", ch, c)),
            None => Err(format!("Expected '{}', got EOF", ch)),
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_ws();
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => Ok(JsonValue::Str(self.parse_string()?)),
            Some('t') | Some('f') => self.parse_bool(),
            Some('n') => self.parse_null(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!("Unexpected char: '{}'", c)),
            None => Err("Unexpected EOF".to_string()),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect_char('{')?;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.advance();
            return Ok(JsonValue::Obj(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.expect_char(':')?;
            let val = self.parse_value()?;
            pairs.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(',') => { self.advance(); }
                Some('}') => { self.advance(); break; }
                _ => return Err("Expected ',' or '}' in object".to_string()),
            }
        }
        Ok(JsonValue::Obj(pairs))
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect_char('[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.advance();
            return Ok(JsonValue::Arr(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => { self.advance(); }
                Some(']') => { self.advance(); break; }
                _ => return Err("Expected ',' or ']' in array".to_string()),
            }
        }
        Ok(JsonValue::Arr(items))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.skip_ws();
        self.expect_char('"')?;
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('\\') => {
                    match self.advance() {
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('n') => s.push('\n'),
                        Some('r') => s.push('\r'),
                        Some('t') => s.push('\t'),
                        Some(c) => { s.push('\\'); s.push(c); }
                        None => return Err("Unexpected EOF in string escape".to_string()),
                    }
                }
                Some('"') => return Ok(s),
                Some(c) => s.push(c),
                None => return Err("Unterminated string".to_string()),
            }
        }
    }

    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let start = self.pos;
        if self.peek() == Some('-') { self.advance(); }
        while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            self.advance();
        }
        if self.peek() == Some('.') {
            self.advance();
            while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                self.advance();
            }
        }
        // Handle scientific notation
        if self.peek() == Some('e') || self.peek() == Some('E') {
            self.advance();
            if self.peek() == Some('+') || self.peek() == Some('-') { self.advance(); }
            while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                self.advance();
            }
        }
        let num_str: String = self.chars[start..self.pos].iter().collect();
        num_str.parse::<f64>()
            .map(JsonValue::Num)
            .map_err(|e| format!("Invalid number: {}", e))
    }

    fn parse_bool(&mut self) -> Result<JsonValue, String> {
        if self.chars[self.pos..].starts_with(&['t', 'r', 'u', 'e']) {
            self.pos += 4;
            Ok(JsonValue::Bool(true))
        } else if self.chars[self.pos..].starts_with(&['f', 'a', 'l', 's', 'e']) {
            self.pos += 5;
            Ok(JsonValue::Bool(false))
        } else {
            Err("Invalid boolean".to_string())
        }
    }

    fn parse_null(&mut self) -> Result<JsonValue, String> {
        if self.chars[self.pos..].starts_with(&['n', 'u', 'l', 'l']) {
            self.pos += 4;
            Ok(JsonValue::Null)
        } else {
            Err("Invalid null".to_string())
        }
    }
}

fn parse_database_json(content: &str) -> Result<Database, String> {
    let mut parser = JsonParser::new(content);
    let root = parser.parse_value()?;

    let tables_val = match &root {
        JsonValue::Obj(pairs) => {
            pairs.iter().find(|(k, _)| k == "tables")
                .map(|(_, v)| v)
                .ok_or("Missing 'tables' key")?
        }
        _ => return Err("Expected root object".to_string()),
    };

    let table_pairs = match tables_val {
        JsonValue::Obj(pairs) => pairs,
        _ => return Err("'tables' must be an object".to_string()),
    };

    let mut tables = HashMap::new();

    for (table_name, table_val) in table_pairs {
        let table_obj = match table_val {
            JsonValue::Obj(pairs) => pairs,
            _ => return Err(format!("Table '{}' must be an object", table_name)),
        };

        // Parse columns
        let columns_val = table_obj.iter().find(|(k, _)| k == "columns")
            .map(|(_, v)| v)
            .ok_or(format!("Missing 'columns' in table '{}'", table_name))?;

        let columns_arr = match columns_val {
            JsonValue::Arr(arr) => arr,
            _ => return Err("'columns' must be an array".to_string()),
        };

        let mut columns = Vec::new();
        for col_val in columns_arr {
            let col_obj = match col_val {
                JsonValue::Obj(pairs) => pairs,
                _ => return Err("Column must be an object".to_string()),
            };
            let name = col_obj.iter().find(|(k, _)| k == "name")
                .and_then(|(_, v)| if let JsonValue::Str(s) = v { Some(s.clone()) } else { None })
                .ok_or("Missing column 'name'")?;
            let dtype_str = col_obj.iter().find(|(k, _)| k == "type")
                .and_then(|(_, v)| if let JsonValue::Str(s) = v { Some(s.clone()) } else { None })
                .ok_or("Missing column 'type'")?;
            let primary_key = col_obj.iter().find(|(k, _)| k == "primary_key")
                .and_then(|(_, v)| if let JsonValue::Bool(b) = v { Some(*b) } else { None })
                .unwrap_or(false);

            let data_type = match dtype_str.as_str() {
                "INT" => DataType::Int,
                "FLOAT" => DataType::Float,
                "TEXT" => DataType::Text,
                "BOOLEAN" => DataType::Boolean,
                _ => return Err(format!("Unknown data type: {}", dtype_str)),
            };

            columns.push(ColumnDef { name, data_type, primary_key });
        }

        // Parse rows
        let rows_val = table_obj.iter().find(|(k, _)| k == "rows")
            .map(|(_, v)| v)
            .ok_or(format!("Missing 'rows' in table '{}'", table_name))?;

        let rows_arr = match rows_val {
            JsonValue::Arr(arr) => arr,
            _ => return Err("'rows' must be an array".to_string()),
        };

        let mut rows = Vec::new();
        for row_val in rows_arr {
            let row_arr = match row_val {
                JsonValue::Arr(arr) => arr,
                _ => return Err("Row must be an array".to_string()),
            };

            let mut row = Vec::new();
            for (i, cell) in row_arr.iter().enumerate() {
                let val = match cell {
                    JsonValue::Num(n) => {
                        if i < columns.len() && columns[i].data_type == DataType::Int {
                            Value::Integer(*n as i64)
                        } else {
                            // Check if it's actually an integer
                            if n.fract() == 0.0 && i < columns.len() && columns[i].data_type == DataType::Float {
                                Value::Float(*n)
                            } else if n.fract() == 0.0 {
                                Value::Integer(*n as i64)
                            } else {
                                Value::Float(*n)
                            }
                        }
                    }
                    JsonValue::Str(s) => Value::Text(s.clone()),
                    JsonValue::Bool(b) => Value::Boolean(*b),
                    JsonValue::Null => Value::Null,
                    _ => return Err("Invalid cell value in row".to_string()),
                };
                row.push(val);
            }
            rows.push(row);
        }

        tables.insert(table_name.clone(), Table {
            schema: TableSchema { name: table_name.clone(), columns },
            rows,
        });
    }

    Ok(Database { tables })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_load_roundtrip() {
        let mut db = Database::new();

        // Set up test data
        let schema = TableSchema {
            name: "test".to_string(),
            columns: vec![
                ColumnDef { name: "id".to_string(), data_type: DataType::Int, primary_key: true },
                ColumnDef { name: "name".to_string(), data_type: DataType::Text, primary_key: false },
            ],
        };
        let table = Table {
            schema,
            rows: vec![
                vec![Value::Integer(1), Value::Text("hello".to_string())],
                vec![Value::Integer(2), Value::Null],
            ],
        };
        db.tables.insert("test".to_string(), table);

        let path = "test_roundtrip.json";
        save_to_file(&db, path).unwrap();
        let loaded = load_from_file(path).unwrap();

        assert!(loaded.tables.contains_key("test"));
        let t = &loaded.tables["test"];
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.schema.columns.len(), 2);

        // Cleanup
        std::fs::remove_file(path).ok();
    }
}
