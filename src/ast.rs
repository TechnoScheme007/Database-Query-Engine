/// All SQL statement types supported by the engine.
#[derive(Debug, Clone)]
pub enum Statement {
    CreateTable(CreateTableStmt),
    Insert(InsertStmt),
    Select(SelectStmt),
    Update(UpdateStmt),
    Delete(DeleteStmt),
    DropTable(String),
}

#[derive(Debug, Clone)]
pub struct CreateTableStmt {
    pub table_name: String,
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub primary_key: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Int,
    Float,
    Text,
    Boolean,
}

#[derive(Debug, Clone)]
pub struct InsertStmt {
    pub table_name: String,
    pub columns: Option<Vec<String>>,
    pub values: Vec<Vec<Expr>>,
}

#[derive(Debug, Clone)]
pub struct SelectStmt {
    pub columns: Vec<SelectColumn>,
    pub from: Option<TableRef>,
    pub joins: Vec<JoinClause>,
    pub where_clause: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub having: Option<Expr>,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<Expr>,
    pub offset: Option<Expr>,
    pub distinct: bool,
}

#[derive(Debug, Clone)]
pub enum SelectColumn {
    Expr(Expr, Option<String>), // expression with optional alias
    AllColumns,                 // *
    TableAll(String),           // table.*
}

#[derive(Debug, Clone)]
pub struct TableRef {
    pub table_name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: TableRef,
    pub on_condition: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct OrderByItem {
    pub expr: Expr,
    pub ascending: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateStmt {
    pub table_name: String,
    pub assignments: Vec<(String, Expr)>,
    pub where_clause: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct DeleteStmt {
    pub table_name: String,
    pub where_clause: Option<Expr>,
}

/// Expression types used in WHERE, SELECT lists, etc.
#[derive(Debug, Clone)]
pub enum Expr {
    // Literals
    IntegerLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    BooleanLiteral(bool),
    Null,

    // References
    ColumnRef(ColumnRefExpr),

    // Operations
    BinaryOp(Box<Expr>, BinaryOp, Box<Expr>),
    UnaryOp(UnaryOp, Box<Expr>),

    // Aggregate functions
    Function(FunctionCall),

    // IS NULL / IS NOT NULL
    IsNull(Box<Expr>, bool), // (expr, is_not)

    // IN (list)
    InList(Box<Expr>, Vec<Expr>, bool), // (expr, list, is_not)

    // BETWEEN a AND b
    BetweenExpr(Box<Expr>, Box<Expr>, Box<Expr>, bool), // (expr, low, high, is_not)

    // LIKE pattern
    LikeExpr(Box<Expr>, Box<Expr>, bool), // (expr, pattern, is_not)
}

#[derive(Debug, Clone)]
pub struct ColumnRefExpr {
    pub table: Option<String>,
    pub column: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Expr>,
    pub distinct: bool,
}
