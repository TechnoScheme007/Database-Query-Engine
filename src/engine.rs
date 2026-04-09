use crate::ast::*;
use std::collections::HashMap;
use std::fmt;

// ── Value type ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Text(String),
    Boolean(bool),
    Null,
}

impl Value {
    pub fn data_type_name(&self) -> &str {
        match self {
            Value::Integer(_) => "INT",
            Value::Float(_) => "FLOAT",
            Value::Text(_) => "TEXT",
            Value::Boolean(_) => "BOOLEAN",
            Value::Null => "NULL",
        }
    }

    fn to_f64(&self) -> Option<f64> {
        match self {
            Value::Integer(n) => Some(*n as f64),
            Value::Float(n) => Some(*n),
            _ => None,
        }
    }

    fn is_truthy(&self) -> bool {
        match self {
            Value::Boolean(b) => *b,
            Value::Integer(n) => *n != 0,
            Value::Null => false,
            _ => true,
        }
    }

    fn cmp_values(&self, other: &Value) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Integer(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Integer(b)) => a.partial_cmp(&(*b as f64)),
            (Value::Text(a), Value::Text(b)) => a.partial_cmp(b),
            (Value::Boolean(a), Value::Boolean(b)) => a.partial_cmp(b),
            (Value::Null, Value::Null) => Some(std::cmp::Ordering::Equal),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Text(s) => write!(f, "{}", s),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Null => write!(f, "NULL"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Integer(a), Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Integer(b)) => *a == (*b as f64),
            (Value::Text(a), Value::Text(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Null, Value::Null) => true,
            _ => false,
        }
    }
}

// ── Table storage ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

impl TableSchema {
    pub fn column_index(&self, col_name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name.eq_ignore_ascii_case(col_name))
    }
}

#[derive(Debug, Clone)]
pub struct Table {
    pub schema: TableSchema,
    pub rows: Vec<Vec<Value>>,
}

// ── Query result ────────────────────────────────────────────────

#[derive(Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub rows_affected: usize,
    pub message: Option<String>,
}

impl fmt::Display for QueryResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(msg) = &self.message {
            return write!(f, "{}", msg);
        }
        if self.columns.is_empty() {
            return write!(f, "({} rows affected)", self.rows_affected);
        }

        let mut widths: Vec<usize> = self.columns.iter().map(|c| c.len()).collect();
        for row in &self.rows {
            for (i, val) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(format!("{}", val).len());
                }
            }
        }

        let header: Vec<String> = self.columns.iter().enumerate()
            .map(|(i, c)| format!("{:width$}", c, width = widths[i]))
            .collect();
        writeln!(f, " {} ", header.join(" | "))?;

        let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
        writeln!(f, "-{}-", sep.join("-+-"))?;

        for row in &self.rows {
            let cells: Vec<String> = row.iter().enumerate()
                .map(|(i, v)| {
                    let w = if i < widths.len() { widths[i] } else { 0 };
                    format!("{:width$}", format!("{}", v), width = w)
                })
                .collect();
            writeln!(f, " {} ", cells.join(" | "))?;
        }

        write!(f, "({} rows)", self.rows.len())
    }
}

// ══════════════════════════════════════════════════════════════════
//  Free functions for expression evaluation (no &self needed)
// ══════════════════════════════════════════════════════════════════

pub fn eval_expr_static(expr: &Expr) -> Result<Value, String> {
    match expr {
        Expr::IntegerLiteral(n) => Ok(Value::Integer(*n)),
        Expr::FloatLiteral(n) => Ok(Value::Float(*n)),
        Expr::StringLiteral(s) => Ok(Value::Text(s.clone())),
        Expr::BooleanLiteral(b) => Ok(Value::Boolean(*b)),
        Expr::Null => Ok(Value::Null),
        Expr::UnaryOp(UnaryOp::Neg, inner) => {
            match eval_expr_static(inner)? {
                Value::Integer(n) => Ok(Value::Integer(-n)),
                Value::Float(n) => Ok(Value::Float(-n)),
                _ => Err("Cannot negate non-numeric value".to_string()),
            }
        }
        _ => Err("Expression not supported in this context".to_string()),
    }
}

pub fn eval_expr(expr: &Expr, row: &[Value], col_map: &ColumnMap) -> Result<Value, String> {
    match expr {
        Expr::IntegerLiteral(n) => Ok(Value::Integer(*n)),
        Expr::FloatLiteral(n) => Ok(Value::Float(*n)),
        Expr::StringLiteral(s) => Ok(Value::Text(s.clone())),
        Expr::BooleanLiteral(b) => Ok(Value::Boolean(*b)),
        Expr::Null => Ok(Value::Null),

        Expr::ColumnRef(cr) => {
            let idx = col_map.resolve(&cr.table, &cr.column)
                .ok_or_else(|| {
                    let col_name = if let Some(ref t) = cr.table {
                        format!("{}.{}", t, cr.column)
                    } else {
                        cr.column.clone()
                    };
                    format!("Column '{}' not found", col_name)
                })?;
            Ok(row.get(idx).cloned().unwrap_or(Value::Null))
        }

        Expr::BinaryOp(left, op, right) => {
            let lv = eval_expr(left, row, col_map)?;
            let rv = eval_expr(right, row, col_map)?;
            apply_binary_op(&lv, op, &rv)
        }

        Expr::UnaryOp(op, inner) => {
            let val = eval_expr(inner, row, col_map)?;
            match op {
                UnaryOp::Not => Ok(Value::Boolean(!val.is_truthy())),
                UnaryOp::Neg => match val {
                    Value::Integer(n) => Ok(Value::Integer(-n)),
                    Value::Float(n) => Ok(Value::Float(-n)),
                    _ => Err("Cannot negate non-numeric value".to_string()),
                },
            }
        }

        Expr::IsNull(inner, is_not) => {
            let val = eval_expr(inner, row, col_map)?;
            let is_null = matches!(val, Value::Null);
            Ok(Value::Boolean(if *is_not { !is_null } else { is_null }))
        }

        Expr::InList(expr, list, is_not) => {
            let val = eval_expr(expr, row, col_map)?;
            let found = list.iter().any(|item| {
                eval_expr(item, row, col_map)
                    .map(|v| v == val)
                    .unwrap_or(false)
            });
            Ok(Value::Boolean(if *is_not { !found } else { found }))
        }

        Expr::BetweenExpr(expr, low, high, is_not) => {
            let val = eval_expr(expr, row, col_map)?;
            let lo = eval_expr(low, row, col_map)?;
            let hi = eval_expr(high, row, col_map)?;
            let in_range = val.cmp_values(&lo).map(|o| o != std::cmp::Ordering::Less).unwrap_or(false)
                && val.cmp_values(&hi).map(|o| o != std::cmp::Ordering::Greater).unwrap_or(false);
            Ok(Value::Boolean(if *is_not { !in_range } else { in_range }))
        }

        Expr::LikeExpr(expr, pattern, is_not) => {
            let val = eval_expr(expr, row, col_map)?;
            let pat = eval_expr(pattern, row, col_map)?;
            let matches = match (&val, &pat) {
                (Value::Text(s), Value::Text(p)) => like_match(s, p),
                _ => false,
            };
            Ok(Value::Boolean(if *is_not { !matches } else { matches }))
        }

        Expr::Function(fc) => {
            match fc.name.as_str() {
                "UPPER" => {
                    let val = eval_expr(&fc.args[0], row, col_map)?;
                    match val {
                        Value::Text(s) => Ok(Value::Text(s.to_uppercase())),
                        _ => Ok(val),
                    }
                }
                "LOWER" => {
                    let val = eval_expr(&fc.args[0], row, col_map)?;
                    match val {
                        Value::Text(s) => Ok(Value::Text(s.to_lowercase())),
                        _ => Ok(val),
                    }
                }
                "LENGTH" => {
                    let val = eval_expr(&fc.args[0], row, col_map)?;
                    match val {
                        Value::Text(s) => Ok(Value::Integer(s.len() as i64)),
                        Value::Null => Ok(Value::Null),
                        _ => Ok(Value::Integer(format!("{}", val).len() as i64)),
                    }
                }
                "ABS" => {
                    let val = eval_expr(&fc.args[0], row, col_map)?;
                    match val {
                        Value::Integer(n) => Ok(Value::Integer(n.abs())),
                        Value::Float(n) => Ok(Value::Float(n.abs())),
                        _ => Err("ABS requires numeric value".to_string()),
                    }
                }
                "COALESCE" => {
                    for arg in &fc.args {
                        let val = eval_expr(arg, row, col_map)?;
                        if !matches!(val, Value::Null) {
                            return Ok(val);
                        }
                    }
                    Ok(Value::Null)
                }
                _ => Err(format!("Unknown function: {}", fc.name)),
            }
        }
    }
}

fn apply_binary_op(left: &Value, op: &BinaryOp, right: &Value) -> Result<Value, String> {
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        match op {
            BinaryOp::And => {
                if matches!(left, Value::Boolean(false)) || matches!(right, Value::Boolean(false)) {
                    return Ok(Value::Boolean(false));
                }
                return Ok(Value::Null);
            }
            BinaryOp::Or => {
                if matches!(left, Value::Boolean(true)) || matches!(right, Value::Boolean(true)) {
                    return Ok(Value::Boolean(true));
                }
                return Ok(Value::Null);
            }
            BinaryOp::Eq | BinaryOp::NotEq => return Ok(Value::Null),
            _ => return Ok(Value::Null),
        }
    }

    match op {
        BinaryOp::Eq => Ok(Value::Boolean(*left == *right)),
        BinaryOp::NotEq => Ok(Value::Boolean(*left != *right)),
        BinaryOp::Lt => Ok(Value::Boolean(left.cmp_values(right) == Some(std::cmp::Ordering::Less))),
        BinaryOp::Gt => Ok(Value::Boolean(left.cmp_values(right) == Some(std::cmp::Ordering::Greater))),
        BinaryOp::LtEq => Ok(Value::Boolean(left.cmp_values(right).map(|o| o != std::cmp::Ordering::Greater).unwrap_or(false))),
        BinaryOp::GtEq => Ok(Value::Boolean(left.cmp_values(right).map(|o| o != std::cmp::Ordering::Less).unwrap_or(false))),
        BinaryOp::And => Ok(Value::Boolean(left.is_truthy() && right.is_truthy())),
        BinaryOp::Or => Ok(Value::Boolean(left.is_truthy() || right.is_truthy())),
        BinaryOp::Plus => numeric_op(left, right, |a, b| a + b, |a, b| a + b),
        BinaryOp::Minus => numeric_op(left, right, |a, b| a - b, |a, b| a - b),
        BinaryOp::Multiply => numeric_op(left, right, |a, b| a * b, |a, b| a * b),
        BinaryOp::Divide => {
            match right {
                Value::Integer(0) => Err("Division by zero".to_string()),
                Value::Float(n) if *n == 0.0 => Err("Division by zero".to_string()),
                _ => numeric_op(left, right, |a, b| a / b, |a, b| a / b),
            }
        }
        BinaryOp::Modulo => numeric_op(left, right, |a, b| a % b, |a, b| a % b),
    }
}

fn numeric_op(
    left: &Value,
    right: &Value,
    int_op: impl Fn(i64, i64) -> i64,
    float_op: impl Fn(f64, f64) -> f64,
) -> Result<Value, String> {
    match (left, right) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(int_op(*a, *b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(float_op(*a, *b))),
        (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(float_op(*a as f64, *b))),
        (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(float_op(*a, *b as f64))),
        (Value::Text(a), Value::Text(b)) => Ok(Value::Text(format!("{}{}", a, b))),
        _ => Err(format!("Cannot perform arithmetic on {} and {}", left.data_type_name(), right.data_type_name())),
    }
}

fn expr_has_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Function(fc) => matches!(fc.name.as_str(), "COUNT" | "SUM" | "AVG" | "MIN" | "MAX"),
        Expr::BinaryOp(l, _, r) => expr_has_aggregate(l) || expr_has_aggregate(r),
        Expr::UnaryOp(_, e) => expr_has_aggregate(e),
        _ => false,
    }
}

fn has_aggregates_in_select(columns: &[SelectColumn]) -> bool {
    columns.iter().any(|col| match col {
        SelectColumn::Expr(expr, _) => expr_has_aggregate(expr),
        _ => false,
    })
}

fn eval_aggregate(expr: &Expr, rows: &[Vec<Value>], col_map: &ColumnMap) -> Result<Value, String> {
    match expr {
        Expr::Function(fc) => {
            match fc.name.as_str() {
                "COUNT" => {
                    if fc.args.len() == 1 {
                        if let Expr::ColumnRef(cr) = &fc.args[0] {
                            if cr.column == "*" {
                                return Ok(Value::Integer(rows.len() as i64));
                            }
                        }
                    }
                    let count = if fc.distinct {
                        let mut seen = Vec::new();
                        rows.iter().filter(|row| {
                            let val = eval_expr(&fc.args[0], row, col_map).unwrap_or(Value::Null);
                            if matches!(val, Value::Null) { return false; }
                            let key = format!("{}", val);
                            if seen.contains(&key) { false }
                            else { seen.push(key); true }
                        }).count()
                    } else {
                        rows.iter().filter(|row| {
                            let val = eval_expr(&fc.args[0], row, col_map).unwrap_or(Value::Null);
                            !matches!(val, Value::Null)
                        }).count()
                    };
                    Ok(Value::Integer(count as i64))
                }
                "SUM" => {
                    let mut sum = 0.0_f64;
                    let mut has_float = false;
                    let mut any = false;
                    for row in rows {
                        let val = eval_expr(&fc.args[0], row, col_map)?;
                        match val {
                            Value::Integer(n) => { sum += n as f64; any = true; }
                            Value::Float(n) => { sum += n; has_float = true; any = true; }
                            Value::Null => {}
                            _ => return Err("SUM requires numeric values".to_string()),
                        }
                    }
                    if !any { return Ok(Value::Null); }
                    if has_float { Ok(Value::Float(sum)) } else { Ok(Value::Integer(sum as i64)) }
                }
                "AVG" => {
                    let mut sum = 0.0_f64;
                    let mut count = 0;
                    for row in rows {
                        let val = eval_expr(&fc.args[0], row, col_map)?;
                        if let Some(n) = val.to_f64() {
                            sum += n;
                            count += 1;
                        }
                    }
                    if count == 0 { Ok(Value::Null) } else { Ok(Value::Float(sum / count as f64)) }
                }
                "MIN" => {
                    let mut min: Option<Value> = None;
                    for row in rows {
                        let val = eval_expr(&fc.args[0], row, col_map)?;
                        if matches!(val, Value::Null) { continue; }
                        min = Some(match min {
                            None => val,
                            Some(ref m) => {
                                if val.cmp_values(m) == Some(std::cmp::Ordering::Less) { val } else { m.clone() }
                            }
                        });
                    }
                    Ok(min.unwrap_or(Value::Null))
                }
                "MAX" => {
                    let mut max: Option<Value> = None;
                    for row in rows {
                        let val = eval_expr(&fc.args[0], row, col_map)?;
                        if matches!(val, Value::Null) { continue; }
                        max = Some(match max {
                            None => val,
                            Some(ref m) => {
                                if val.cmp_values(m) == Some(std::cmp::Ordering::Greater) { val } else { m.clone() }
                            }
                        });
                    }
                    Ok(max.unwrap_or(Value::Null))
                }
                other => Err(format!("Unknown aggregate function: {}", other)),
            }
        }
        Expr::ColumnRef(_) => {
            if let Some(first) = rows.first() {
                eval_expr(expr, first, col_map)
            } else {
                Ok(Value::Null)
            }
        }
        Expr::BinaryOp(l, op, r) => {
            let lv = eval_aggregate(l, rows, col_map)?;
            let rv = eval_aggregate(r, rows, col_map)?;
            apply_binary_op(&lv, op, &rv)
        }
        _ => {
            if let Some(first) = rows.first() {
                eval_expr(expr, first, col_map)
            } else {
                eval_expr_static(expr)
            }
        }
    }
}

fn expr_display_name(expr: &Expr) -> String {
    match expr {
        Expr::ColumnRef(cr) => {
            if let Some(ref t) = cr.table {
                format!("{}.{}", t, cr.column)
            } else {
                cr.column.clone()
            }
        }
        Expr::Function(fc) => {
            let args: Vec<String> = fc.args.iter().map(|a| expr_display_name(a)).collect();
            if fc.distinct {
                format!("{}(DISTINCT {})", fc.name, args.join(", "))
            } else {
                format!("{}({})", fc.name, args.join(", "))
            }
        }
        Expr::IntegerLiteral(n) => format!("{}", n),
        Expr::FloatLiteral(n) => format!("{}", n),
        Expr::StringLiteral(s) => format!("'{}'", s),
        Expr::BinaryOp(l, op, r) => {
            let op_str = match op {
                BinaryOp::Plus => "+",
                BinaryOp::Minus => "-",
                BinaryOp::Multiply => "*",
                BinaryOp::Divide => "/",
                _ => "?",
            };
            format!("{} {} {}", expr_display_name(l), op_str, expr_display_name(r))
        }
        _ => "?".to_string(),
    }
}

fn sort_rows_by(
    rows: &mut [Vec<Value>],
    order_by: &[OrderByItem],
    col_map: &ColumnMap,
) {
    rows.sort_by(|a, b| {
        for item in order_by {
            let va = eval_expr(&item.expr, a, col_map).unwrap_or(Value::Null);
            let vb = eval_expr(&item.expr, b, col_map).unwrap_or(Value::Null);
            let ord = va.cmp_values(&vb).unwrap_or(std::cmp::Ordering::Equal);
            let ord = if item.ascending { ord } else { ord.reverse() };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
}

// ── LIKE pattern matching ───────────────────────────────────────

fn like_match(s: &str, pattern: &str) -> bool {
    let s_chars: Vec<char> = s.chars().collect();
    let p_chars: Vec<char> = pattern.chars().collect();
    like_match_recursive(&s_chars, 0, &p_chars, 0)
}

fn like_match_recursive(s: &[char], si: usize, p: &[char], pi: usize) -> bool {
    if pi == p.len() {
        return si == s.len();
    }
    match p[pi] {
        '%' => {
            for i in si..=s.len() {
                if like_match_recursive(s, i, p, pi + 1) {
                    return true;
                }
            }
            false
        }
        '_' => {
            si < s.len() && like_match_recursive(s, si + 1, p, pi + 1)
        }
        ch => {
            si < s.len()
                && (s[si] == ch || s[si].to_lowercase().next() == ch.to_lowercase().next())
                && like_match_recursive(s, si + 1, p, pi + 1)
        }
    }
}

// ── Column map ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ColumnMap {
    qualified: Vec<(String, String, usize)>,
    unqualified: Vec<(String, usize)>,
    ordered: Vec<(String, usize)>,
}

impl ColumnMap {
    pub fn new() -> Self {
        Self { qualified: Vec::new(), unqualified: Vec::new(), ordered: Vec::new() }
    }

    pub fn from_table(schema: &TableSchema) -> Self {
        let mut cm = Self::new();
        for (i, col) in schema.columns.iter().enumerate() {
            cm.add(schema.name.clone(), col.name.clone(), i);
            cm.add_unqualified(col.name.clone(), i);
        }
        cm
    }

    pub fn add(&mut self, table: String, column: String, index: usize) {
        self.qualified.push((table.to_lowercase(), column.to_lowercase(), index));
        self.ordered.push((column.clone(), index));
    }

    pub fn add_unqualified(&mut self, column: String, index: usize) {
        self.unqualified.retain(|(c, _)| c.to_lowercase() != column.to_lowercase());
        self.unqualified.push((column.to_lowercase(), index));
    }

    pub fn resolve(&self, table: &Option<String>, column: &str) -> Option<usize> {
        let col_lower = column.to_lowercase();
        if let Some(ref t) = table {
            let t_lower = t.to_lowercase();
            self.qualified.iter()
                .find(|(tbl, col, _)| *tbl == t_lower && *col == col_lower)
                .map(|(_, _, idx)| *idx)
        } else {
            self.unqualified.iter()
                .find(|(c, _)| *c == col_lower)
                .map(|(_, idx)| *idx)
        }
    }

    pub fn ordered_columns(&self) -> &[(String, usize)] {
        &self.ordered
    }

    pub fn table_columns(&self, table: &str) -> Vec<(&String, &usize)> {
        let t_lower = table.to_lowercase();
        self.qualified.iter()
            .filter(|(t, _, _)| *t == t_lower)
            .map(|(_, c, i)| (c, i))
            .collect()
    }
}

fn build_result_col_map(cols: &[String]) -> ColumnMap {
    let mut cm = ColumnMap::new();
    for (i, name) in cols.iter().enumerate() {
        cm.add_unqualified(name.clone(), i);
    }
    cm
}

// ══════════════════════════════════════════════════════════════════
//  Database engine
// ══════════════════════════════════════════════════════════════════

pub struct Database {
    pub tables: HashMap<String, Table>,
}

impl Database {
    pub fn new() -> Self {
        Self { tables: HashMap::new() }
    }

    pub fn execute(&mut self, stmt: Statement) -> Result<QueryResult, String> {
        match stmt {
            Statement::CreateTable(ct) => self.exec_create_table(ct),
            Statement::DropTable(name) => self.exec_drop_table(&name),
            Statement::Insert(ins) => self.exec_insert(ins),
            Statement::Select(sel) => self.exec_select(sel),
            Statement::Update(upd) => self.exec_update(upd),
            Statement::Delete(del) => self.exec_delete(del),
        }
    }

    fn exec_create_table(&mut self, stmt: CreateTableStmt) -> Result<QueryResult, String> {
        let name = stmt.table_name.to_lowercase();
        if self.tables.contains_key(&name) {
            return Err(format!("Table '{}' already exists", name));
        }
        self.tables.insert(name.clone(), Table {
            schema: TableSchema { name: name.clone(), columns: stmt.columns },
            rows: Vec::new(),
        });
        Ok(QueryResult { columns: vec![], rows: vec![], rows_affected: 0,
            message: Some(format!("Table '{}' created", name)) })
    }

    fn exec_drop_table(&mut self, name: &str) -> Result<QueryResult, String> {
        let name = name.to_lowercase();
        if self.tables.remove(&name).is_none() {
            return Err(format!("Table '{}' does not exist", name));
        }
        Ok(QueryResult { columns: vec![], rows: vec![], rows_affected: 0,
            message: Some(format!("Table '{}' dropped", name)) })
    }

    fn exec_insert(&mut self, stmt: InsertStmt) -> Result<QueryResult, String> {
        let table_name = stmt.table_name.to_lowercase();
        let table = self.tables.get_mut(&table_name)
            .ok_or_else(|| format!("Table '{}' does not exist", table_name))?;

        let column_order: Vec<usize> = if let Some(ref cols) = stmt.columns {
            cols.iter().map(|c| {
                table.schema.column_index(c)
                    .ok_or_else(|| format!("Column '{}' not found in table '{}'", c, table_name))
            }).collect::<Result<Vec<_>, _>>()?
        } else {
            (0..table.schema.columns.len()).collect()
        };

        let num_cols = table.schema.columns.len();
        let mut count = 0;
        for value_row in &stmt.values {
            if value_row.len() != column_order.len() {
                return Err(format!("Expected {} values, got {}", column_order.len(), value_row.len()));
            }
            let mut row = vec![Value::Null; num_cols];
            for (i, expr) in value_row.iter().enumerate() {
                let val = eval_expr_static(expr)?;
                row[column_order[i]] = val;
            }
            table.rows.push(row);
            count += 1;
        }

        Ok(QueryResult { columns: vec![], rows: vec![], rows_affected: count,
            message: Some(format!("{} row(s) inserted", count)) })
    }

    fn exec_select(&self, stmt: SelectStmt) -> Result<QueryResult, String> {
        let (mut rows, col_map) = if let Some(ref table_ref) = stmt.from {
            self.load_table_rows(table_ref)?
        } else {
            (vec![vec![]], ColumnMap::new())
        };

        let mut col_map = col_map;
        for join in &stmt.joins {
            let result = self.apply_join(&rows, &col_map, join)?;
            rows = result.0;
            col_map = result.1;
        }

        // WHERE
        if let Some(ref where_expr) = stmt.where_clause {
            rows = rows.into_iter()
                .filter(|row| eval_expr(where_expr, row, &col_map).map(|v| v.is_truthy()).unwrap_or(false))
                .collect();
        }

        // GROUP BY
        if !stmt.group_by.is_empty() {
            return self.exec_grouped_select(&stmt, &rows, &col_map);
        }

        // Aggregate without GROUP BY
        if has_aggregates_in_select(&stmt.columns) {
            return Self::exec_aggregate_select(&stmt, &rows, &col_map);
        }

        // Project
        let (result_cols, mut result_rows) = Self::project_columns(&stmt.columns, &rows, &col_map)?;
        let result_col_map = build_result_col_map(&result_cols);

        // DISTINCT
        if stmt.distinct {
            let mut seen: Vec<Vec<String>> = Vec::new();
            result_rows.retain(|row| {
                let key: Vec<String> = row.iter().map(|v| format!("{}", v)).collect();
                if seen.contains(&key) { false } else { seen.push(key); true }
            });
        }

        // ORDER BY
        if !stmt.order_by.is_empty() {
            sort_rows_by(&mut result_rows, &stmt.order_by, &result_col_map);
        }

        // OFFSET
        if let Some(ref offset_expr) = stmt.offset {
            if let Value::Integer(n) = eval_expr_static(offset_expr)? {
                let n = n.max(0) as usize;
                if n < result_rows.len() { result_rows = result_rows[n..].to_vec(); }
                else { result_rows.clear(); }
            }
        }

        // LIMIT
        if let Some(ref limit_expr) = stmt.limit {
            if let Value::Integer(n) = eval_expr_static(limit_expr)? {
                result_rows.truncate(n.max(0) as usize);
            }
        }

        Ok(QueryResult { columns: result_cols, rows: result_rows, rows_affected: 0, message: None })
    }

    fn exec_aggregate_select(
        stmt: &SelectStmt, rows: &[Vec<Value>], col_map: &ColumnMap,
    ) -> Result<QueryResult, String> {
        let mut result_cols = Vec::new();
        let mut result_row = Vec::new();
        for col in &stmt.columns {
            match col {
                SelectColumn::Expr(expr, alias) => {
                    let val = eval_aggregate(expr, rows, col_map)?;
                    result_cols.push(alias.clone().unwrap_or_else(|| expr_display_name(expr)));
                    result_row.push(val);
                }
                _ => return Err("Cannot mix * with aggregates without GROUP BY".to_string()),
            }
        }
        Ok(QueryResult { columns: result_cols, rows: vec![result_row], rows_affected: 0, message: None })
    }

    fn exec_grouped_select(
        &self, stmt: &SelectStmt, rows: &[Vec<Value>], col_map: &ColumnMap,
    ) -> Result<QueryResult, String> {
        let mut groups: Vec<(Vec<String>, Vec<Vec<Value>>)> = Vec::new();
        for row in rows {
            let key: Vec<String> = stmt.group_by.iter()
                .map(|e| eval_expr(e, row, col_map).map(|v| format!("{}", v)).unwrap_or_default())
                .collect();
            if let Some(group) = groups.iter_mut().find(|(k, _)| *k == key) {
                group.1.push(row.clone());
            } else {
                groups.push((key, vec![row.clone()]));
            }
        }

        let mut result_cols = Vec::new();
        let mut result_rows = Vec::new();
        let mut cols_built = false;

        for (_key, group_rows) in &groups {
            let mut result_row = Vec::new();
            for col in &stmt.columns {
                match col {
                    SelectColumn::Expr(expr, alias) => {
                        let val = eval_aggregate(expr, group_rows, col_map)?;
                        if !cols_built {
                            result_cols.push(alias.clone().unwrap_or_else(|| expr_display_name(expr)));
                        }
                        result_row.push(val);
                    }
                    SelectColumn::AllColumns => return Err("Cannot use * with GROUP BY".to_string()),
                    SelectColumn::TableAll(_) => return Err("Cannot use table.* with GROUP BY".to_string()),
                }
            }
            cols_built = true;

            if let Some(ref having_expr) = stmt.having {
                let val = eval_aggregate(having_expr, group_rows, col_map)?;
                if !val.is_truthy() { continue; }
            }
            result_rows.push(result_row);
        }

        let result_col_map = build_result_col_map(&result_cols);

        if !stmt.order_by.is_empty() {
            sort_rows_by(&mut result_rows, &stmt.order_by, &result_col_map);
        }

        if let Some(ref offset_expr) = stmt.offset {
            if let Value::Integer(n) = eval_expr_static(offset_expr)? {
                let n = n.max(0) as usize;
                if n < result_rows.len() { result_rows = result_rows[n..].to_vec(); }
                else { result_rows.clear(); }
            }
        }
        if let Some(ref limit_expr) = stmt.limit {
            if let Value::Integer(n) = eval_expr_static(limit_expr)? {
                result_rows.truncate(n.max(0) as usize);
            }
        }

        Ok(QueryResult { columns: result_cols, rows: result_rows, rows_affected: 0, message: None })
    }

    fn exec_update(&mut self, stmt: UpdateStmt) -> Result<QueryResult, String> {
        let table_name = stmt.table_name.to_lowercase();
        let table = self.tables.get_mut(&table_name)
            .ok_or_else(|| format!("Table '{}' does not exist", table_name))?;

        let col_map = ColumnMap::from_table(&table.schema);

        // Collect indices of matching rows first
        let matching: Vec<usize> = table.rows.iter().enumerate()
            .filter(|(_, row)| {
                if let Some(ref where_expr) = stmt.where_clause {
                    eval_expr(where_expr, row, &col_map).map(|v| v.is_truthy()).unwrap_or(false)
                } else {
                    true
                }
            })
            .map(|(i, _)| i)
            .collect();

        let count = matching.len();
        for idx in matching {
            for (col_name, expr) in &stmt.assignments {
                let col_idx = table.schema.column_index(col_name)
                    .ok_or_else(|| format!("Column '{}' not found", col_name))?;
                let val = eval_expr(expr, &table.rows[idx], &col_map)?;
                table.rows[idx][col_idx] = val;
            }
        }

        Ok(QueryResult { columns: vec![], rows: vec![], rows_affected: count,
            message: Some(format!("{} row(s) updated", count)) })
    }

    fn exec_delete(&mut self, stmt: DeleteStmt) -> Result<QueryResult, String> {
        let table_name = stmt.table_name.to_lowercase();
        let table = self.tables.get_mut(&table_name)
            .ok_or_else(|| format!("Table '{}' does not exist", table_name))?;

        let col_map = ColumnMap::from_table(&table.schema);
        let before = table.rows.len();

        let where_clause = stmt.where_clause;
        table.rows.retain(|row| {
            if let Some(ref where_expr) = where_clause {
                !eval_expr(where_expr, row, &col_map).map(|v| v.is_truthy()).unwrap_or(false)
            } else {
                false
            }
        });

        let count = before - table.rows.len();
        Ok(QueryResult { columns: vec![], rows: vec![], rows_affected: count,
            message: Some(format!("{} row(s) deleted", count)) })
    }

    fn load_table_rows(&self, table_ref: &TableRef) -> Result<(Vec<Vec<Value>>, ColumnMap), String> {
        let table_name = table_ref.table_name.to_lowercase();
        let table = self.tables.get(&table_name)
            .ok_or_else(|| format!("Table '{}' does not exist", table_name))?;

        let alias = table_ref.alias.as_deref().unwrap_or(&table_ref.table_name);
        let mut col_map = ColumnMap::new();
        for (i, col) in table.schema.columns.iter().enumerate() {
            col_map.add(alias.to_string(), col.name.clone(), i);
            col_map.add_unqualified(col.name.clone(), i);
        }
        Ok((table.rows.clone(), col_map))
    }

    fn apply_join(
        &self, left_rows: &[Vec<Value>], left_map: &ColumnMap, join: &JoinClause,
    ) -> Result<(Vec<Vec<Value>>, ColumnMap), String> {
        let table_name = join.table.table_name.to_lowercase();
        let right_table = self.tables.get(&table_name)
            .ok_or_else(|| format!("Table '{}' does not exist", table_name))?;

        let alias = join.table.alias.as_deref().unwrap_or(&join.table.table_name);
        let left_width = if left_rows.is_empty() { 0 } else { left_rows[0].len() };

        let mut combined_map = left_map.clone();
        for (i, col) in right_table.schema.columns.iter().enumerate() {
            combined_map.add(alias.to_string(), col.name.clone(), left_width + i);
            combined_map.add_unqualified(col.name.clone(), left_width + i);
        }

        let right_width = right_table.schema.columns.len();
        let mut result = Vec::new();

        match join.join_type {
            JoinType::Inner => {
                for left_row in left_rows {
                    for right_row in &right_table.rows {
                        let mut combined = left_row.clone();
                        combined.extend(right_row.clone());
                        if eval_expr(&join.on_condition, &combined, &combined_map).map(|v| v.is_truthy()).unwrap_or(false) {
                            result.push(combined);
                        }
                    }
                }
            }
            JoinType::Left => {
                for left_row in left_rows {
                    let mut matched = false;
                    for right_row in &right_table.rows {
                        let mut combined = left_row.clone();
                        combined.extend(right_row.clone());
                        if eval_expr(&join.on_condition, &combined, &combined_map).map(|v| v.is_truthy()).unwrap_or(false) {
                            result.push(combined);
                            matched = true;
                        }
                    }
                    if !matched {
                        let mut combined = left_row.clone();
                        combined.extend(vec![Value::Null; right_width]);
                        result.push(combined);
                    }
                }
            }
            JoinType::Right => {
                for right_row in &right_table.rows {
                    let mut matched = false;
                    for left_row in left_rows {
                        let mut combined = left_row.clone();
                        combined.extend(right_row.clone());
                        if eval_expr(&join.on_condition, &combined, &combined_map).map(|v| v.is_truthy()).unwrap_or(false) {
                            result.push(combined);
                            matched = true;
                        }
                    }
                    if !matched {
                        let mut combined = vec![Value::Null; left_width];
                        combined.extend(right_row.clone());
                        result.push(combined);
                    }
                }
            }
        }

        Ok((result, combined_map))
    }

    fn project_columns(
        select_cols: &[SelectColumn], rows: &[Vec<Value>], col_map: &ColumnMap,
    ) -> Result<(Vec<String>, Vec<Vec<Value>>), String> {
        // Build projection plan: list of (column_name, ProjectionKind)
        enum Proj {
            Index(usize),
            Expr(Expr),
        }

        let mut result_cols = Vec::new();
        let mut projs: Vec<Proj> = Vec::new();

        for col in select_cols {
            match col {
                SelectColumn::AllColumns => {
                    for (name, idx) in col_map.ordered_columns() {
                        result_cols.push(name.clone());
                        projs.push(Proj::Index(*idx));
                    }
                }
                SelectColumn::TableAll(table) => {
                    for (name, idx) in col_map.table_columns(table) {
                        result_cols.push(name.clone());
                        projs.push(Proj::Index(*idx));
                    }
                }
                SelectColumn::Expr(expr, alias) => {
                    result_cols.push(alias.clone().unwrap_or_else(|| expr_display_name(expr)));
                    projs.push(Proj::Expr(expr.clone()));
                }
            }
        }

        let mut result_rows = Vec::new();
        for row in rows {
            let mut result_row = Vec::new();
            for proj in &projs {
                let val = match proj {
                    Proj::Index(idx) => row.get(*idx).cloned().unwrap_or(Value::Null),
                    Proj::Expr(expr) => eval_expr(expr, row, col_map)?,
                };
                result_row.push(val);
            }
            result_rows.push(result_row);
        }

        Ok((result_cols, result_rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::Tokenizer;
    use crate::parser::Parser;

    fn run_sql(db: &mut Database, sql: &str) -> Result<QueryResult, String> {
        let mut tokenizer = Tokenizer::new(sql);
        let tokens = tokenizer.tokenize().map_err(|e| e.to_string())?;
        let mut parser = Parser::new(tokens);
        let stmt = parser.parse()?;
        db.execute(stmt)
    }

    #[test]
    fn test_create_insert_select() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE users (id INT, name TEXT, age INT)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (1, 'Alice', 30)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (2, 'Bob', 25)").unwrap();

        let result = run_sql(&mut db, "SELECT * FROM users").unwrap();
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.columns.len(), 3);
    }

    #[test]
    fn test_where_clause() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE users (id INT, name TEXT, age INT)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (1, 'Alice', 30)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (2, 'Bob', 25)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (3, 'Charlie', 35)").unwrap();

        let result = run_sql(&mut db, "SELECT name FROM users WHERE age > 28").unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_order_by_limit() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE nums (val INT)").unwrap();
        run_sql(&mut db, "INSERT INTO nums VALUES (3)").unwrap();
        run_sql(&mut db, "INSERT INTO nums VALUES (1)").unwrap();
        run_sql(&mut db, "INSERT INTO nums VALUES (2)").unwrap();

        let result = run_sql(&mut db, "SELECT val FROM nums ORDER BY val ASC LIMIT 2").unwrap();
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0][0], Value::Integer(1));
        assert_eq!(result.rows[1][0], Value::Integer(2));
    }

    #[test]
    fn test_group_by_having() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE emp (dept TEXT, salary INT)").unwrap();
        run_sql(&mut db, "INSERT INTO emp VALUES ('eng', 100)").unwrap();
        run_sql(&mut db, "INSERT INTO emp VALUES ('eng', 120)").unwrap();
        run_sql(&mut db, "INSERT INTO emp VALUES ('sales', 90)").unwrap();
        run_sql(&mut db, "INSERT INTO emp VALUES ('sales', 80)").unwrap();
        run_sql(&mut db, "INSERT INTO emp VALUES ('hr', 70)").unwrap();

        let result = run_sql(&mut db,
            "SELECT dept, SUM(salary) AS total FROM emp GROUP BY dept HAVING SUM(salary) > 100"
        ).unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_inner_join() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE users (id INT, name TEXT)").unwrap();
        run_sql(&mut db, "CREATE TABLE orders (id INT, user_id INT, amount INT)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (1, 'Alice')").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (2, 'Bob')").unwrap();
        run_sql(&mut db, "INSERT INTO orders VALUES (1, 1, 100)").unwrap();
        run_sql(&mut db, "INSERT INTO orders VALUES (2, 1, 200)").unwrap();
        run_sql(&mut db, "INSERT INTO orders VALUES (3, 2, 150)").unwrap();

        let result = run_sql(&mut db,
            "SELECT u.name, o.amount FROM users u INNER JOIN orders o ON u.id = o.user_id"
        ).unwrap();
        assert_eq!(result.rows.len(), 3);
    }

    #[test]
    fn test_left_join() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE users (id INT, name TEXT)").unwrap();
        run_sql(&mut db, "CREATE TABLE orders (user_id INT, amount INT)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (1, 'Alice')").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (2, 'Bob')").unwrap();
        run_sql(&mut db, "INSERT INTO orders VALUES (1, 100)").unwrap();

        let result = run_sql(&mut db,
            "SELECT u.name, o.amount FROM users u LEFT JOIN orders o ON u.id = o.user_id"
        ).unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_update() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE users (id INT, name TEXT, age INT)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (1, 'Alice', 30)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (2, 'Bob', 25)").unwrap();

        let result = run_sql(&mut db, "UPDATE users SET age = 31 WHERE name = 'Alice'").unwrap();
        assert_eq!(result.rows_affected, 1);

        let result = run_sql(&mut db, "SELECT age FROM users WHERE name = 'Alice'").unwrap();
        assert_eq!(result.rows[0][0], Value::Integer(31));
    }

    #[test]
    fn test_delete() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE users (id INT, name TEXT)").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (1, 'Alice')").unwrap();
        run_sql(&mut db, "INSERT INTO users VALUES (2, 'Bob')").unwrap();

        let result = run_sql(&mut db, "DELETE FROM users WHERE name = 'Bob'").unwrap();
        assert_eq!(result.rows_affected, 1);

        let result = run_sql(&mut db, "SELECT * FROM users").unwrap();
        assert_eq!(result.rows.len(), 1);
    }

    #[test]
    fn test_like() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE t (name TEXT)").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES ('Alice')").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES ('Bob')").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES ('Alicia')").unwrap();

        let result = run_sql(&mut db, "SELECT name FROM t WHERE name LIKE 'Ali%'").unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_aggregate_functions() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE nums (val INT)").unwrap();
        run_sql(&mut db, "INSERT INTO nums VALUES (10)").unwrap();
        run_sql(&mut db, "INSERT INTO nums VALUES (20)").unwrap();
        run_sql(&mut db, "INSERT INTO nums VALUES (30)").unwrap();

        let result = run_sql(&mut db,
            "SELECT COUNT(*) AS cnt, SUM(val) AS total, AVG(val) AS average, MIN(val) AS lo, MAX(val) AS hi FROM nums"
        ).unwrap();
        assert_eq!(result.rows[0][0], Value::Integer(3));
        assert_eq!(result.rows[0][1], Value::Integer(60));
        assert_eq!(result.rows[0][3], Value::Integer(10));
        assert_eq!(result.rows[0][4], Value::Integer(30));
    }

    #[test]
    fn test_multi_row_insert() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE t (id INT, name TEXT)").unwrap();
        let result = run_sql(&mut db, "INSERT INTO t VALUES (1, 'a'), (2, 'b'), (3, 'c')").unwrap();
        assert_eq!(result.rows_affected, 3);

        let result = run_sql(&mut db, "SELECT * FROM t").unwrap();
        assert_eq!(result.rows.len(), 3);
    }

    #[test]
    fn test_between() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE nums (val INT)").unwrap();
        run_sql(&mut db, "INSERT INTO nums VALUES (1)").unwrap();
        run_sql(&mut db, "INSERT INTO nums VALUES (5)").unwrap();
        run_sql(&mut db, "INSERT INTO nums VALUES (10)").unwrap();

        let result = run_sql(&mut db, "SELECT val FROM nums WHERE val BETWEEN 2 AND 8").unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], Value::Integer(5));
    }

    #[test]
    fn test_in_list() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE t (name TEXT)").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES ('a')").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES ('b')").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES ('c')").unwrap();

        let result = run_sql(&mut db, "SELECT name FROM t WHERE name IN ('a', 'c')").unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_is_null() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE t (id INT, name TEXT)").unwrap();
        run_sql(&mut db, "INSERT INTO t (id) VALUES (1)").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES (2, 'Bob')").unwrap();

        let result = run_sql(&mut db, "SELECT id FROM t WHERE name IS NULL").unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], Value::Integer(1));
    }

    #[test]
    fn test_distinct() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE t (val INT)").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES (1)").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES (1)").unwrap();
        run_sql(&mut db, "INSERT INTO t VALUES (2)").unwrap();

        let result = run_sql(&mut db, "SELECT DISTINCT val FROM t").unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_right_join() {
        let mut db = Database::new();
        run_sql(&mut db, "CREATE TABLE a (id INT, name TEXT)").unwrap();
        run_sql(&mut db, "CREATE TABLE b (a_id INT, val TEXT)").unwrap();
        run_sql(&mut db, "INSERT INTO a VALUES (1, 'x')").unwrap();
        run_sql(&mut db, "INSERT INTO b VALUES (1, 'p')").unwrap();
        run_sql(&mut db, "INSERT INTO b VALUES (2, 'q')").unwrap();

        let result = run_sql(&mut db,
            "SELECT a.name, b.val FROM a RIGHT JOIN b ON a.id = b.a_id"
        ).unwrap();
        assert_eq!(result.rows.len(), 2);
    }
}
