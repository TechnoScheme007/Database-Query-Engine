use crate::ast::*;
use crate::tokenizer::Token;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Statement, String> {
        let stmt = match self.peek() {
            Token::Create => self.parse_create_table()?,
            Token::Insert => self.parse_insert()?,
            Token::Select => self.parse_select_stmt()?,
            Token::Update => self.parse_update()?,
            Token::Delete => self.parse_delete()?,
            Token::Drop => self.parse_drop_table()?,
            other => return Err(format!("Unexpected token at start of statement: {}", other)),
        };
        // Consume optional semicolon
        if self.peek() == Token::Semicolon {
            self.advance();
        }
        Ok(stmt)
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn peek(&self) -> Token {
        self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.peek();
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<Token, String> {
        let tok = self.advance();
        if std::mem::discriminant(&tok) == std::mem::discriminant(expected) {
            Ok(tok)
        } else {
            Err(format!("Expected {:?}, got {}", expected, tok))
        }
    }

    fn expect_identifier(&mut self) -> Result<String, String> {
        match self.advance() {
            Token::Identifier(s) => Ok(s),
            other => Err(format!("Expected identifier, got {}", other)),
        }
    }

    fn peek_is_identifier(&self) -> bool {
        matches!(self.peek(), Token::Identifier(_))
    }

    // ── CREATE TABLE ────────────────────────────────────────────

    fn parse_create_table(&mut self) -> Result<Statement, String> {
        self.expect(&Token::Create)?;
        self.expect(&Token::Table)?;
        let table_name = self.expect_identifier()?;
        self.expect(&Token::LeftParen)?;

        let mut columns = Vec::new();
        loop {
            let col_name = self.expect_identifier()?;
            let data_type = self.parse_data_type()?;
            let mut primary_key = false;
            if self.peek() == Token::Primary {
                self.advance();
                self.expect(&Token::Key)?;
                primary_key = true;
            }
            columns.push(ColumnDef { name: col_name, data_type, primary_key });
            if self.peek() == Token::Comma {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(&Token::RightParen)?;
        Ok(Statement::CreateTable(CreateTableStmt { table_name, columns }))
    }

    fn parse_data_type(&mut self) -> Result<DataType, String> {
        let tok = self.advance();
        match tok {
            Token::Int | Token::Integer => Ok(DataType::Int),
            Token::Float => Ok(DataType::Float),
            Token::Text => Ok(DataType::Text),
            Token::Varchar => {
                // Optionally consume (N)
                if self.peek() == Token::LeftParen {
                    self.advance();
                    self.advance(); // the number
                    self.expect(&Token::RightParen)?;
                }
                Ok(DataType::Text)
            }
            Token::Boolean | Token::Bool => Ok(DataType::Boolean),
            _ => Err(format!("Expected data type, got {:?}", tok)),
        }
    }

    // ── DROP TABLE ──────────────────────────────────────────────

    fn parse_drop_table(&mut self) -> Result<Statement, String> {
        self.expect(&Token::Drop)?;
        self.expect(&Token::Table)?;
        let name = self.expect_identifier()?;
        Ok(Statement::DropTable(name))
    }

    // ── INSERT ──────────────────────────────────────────────────

    fn parse_insert(&mut self) -> Result<Statement, String> {
        self.expect(&Token::Insert)?;
        self.expect(&Token::Into)?;
        let table_name = self.expect_identifier()?;

        // Optional column list
        let columns = if self.peek() == Token::LeftParen {
            self.advance();
            let cols = self.parse_identifier_list()?;
            self.expect(&Token::RightParen)?;
            Some(cols)
        } else {
            None
        };

        self.expect(&Token::Values)?;

        let mut value_rows = Vec::new();
        loop {
            self.expect(&Token::LeftParen)?;
            let vals = self.parse_expr_list()?;
            self.expect(&Token::RightParen)?;
            value_rows.push(vals);
            if self.peek() == Token::Comma {
                self.advance();
            } else {
                break;
            }
        }

        Ok(Statement::Insert(InsertStmt { table_name, columns, values: value_rows }))
    }

    fn parse_identifier_list(&mut self) -> Result<Vec<String>, String> {
        let mut list = vec![self.expect_identifier()?];
        while self.peek() == Token::Comma {
            self.advance();
            list.push(self.expect_identifier()?);
        }
        Ok(list)
    }

    fn parse_expr_list(&mut self) -> Result<Vec<Expr>, String> {
        let mut list = vec![self.parse_expr()?];
        while self.peek() == Token::Comma {
            self.advance();
            list.push(self.parse_expr()?);
        }
        Ok(list)
    }

    // ── SELECT ──────────────────────────────────────────────────

    fn parse_select_stmt(&mut self) -> Result<Statement, String> {
        self.expect(&Token::Select)?;

        let distinct = if self.peek() == Token::Distinct {
            self.advance();
            true
        } else {
            false
        };

        let columns = self.parse_select_columns()?;

        let from = if self.peek() == Token::From {
            self.advance();
            Some(self.parse_table_ref()?)
        } else {
            None
        };

        let joins = self.parse_joins()?;

        let where_clause = if self.peek() == Token::Where {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        let group_by = if self.peek() == Token::Group {
            self.advance();
            self.expect(&Token::By)?;
            self.parse_expr_list()?
        } else {
            Vec::new()
        };

        let having = if self.peek() == Token::Having {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        let order_by = if self.peek() == Token::Order {
            self.advance();
            self.expect(&Token::By)?;
            self.parse_order_by_list()?
        } else {
            Vec::new()
        };

        let limit = if self.peek() == Token::Limit {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        let offset = if self.peek() == Token::Offset {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(Statement::Select(SelectStmt {
            columns,
            from,
            joins,
            where_clause,
            group_by,
            having,
            order_by,
            limit,
            offset,
            distinct,
        }))
    }

    fn parse_select_columns(&mut self) -> Result<Vec<SelectColumn>, String> {
        let mut cols = vec![self.parse_select_column()?];
        while self.peek() == Token::Comma {
            self.advance();
            cols.push(self.parse_select_column()?);
        }
        Ok(cols)
    }

    fn parse_select_column(&mut self) -> Result<SelectColumn, String> {
        if self.peek() == Token::Asterisk {
            self.advance();
            return Ok(SelectColumn::AllColumns);
        }

        // Check for table.* pattern
        if self.peek_is_identifier() {
            let next_pos = self.pos + 1;
            if self.tokens.get(next_pos) == Some(&Token::Dot)
                && self.tokens.get(next_pos + 1) == Some(&Token::Asterisk)
            {
                if let Token::Identifier(table) = self.advance() {
                    self.advance(); // dot
                    self.advance(); // asterisk
                    return Ok(SelectColumn::TableAll(table));
                }
            }
        }

        let expr = self.parse_expr()?;
        let alias = if self.peek() == Token::As {
            self.advance();
            Some(self.expect_identifier()?)
        } else if self.peek_is_identifier() && !self.peek_is_keyword() {
            Some(self.expect_identifier()?)
        } else {
            None
        };
        Ok(SelectColumn::Expr(expr, alias))
    }

    fn peek_is_keyword(&self) -> bool {
        matches!(
            self.peek(),
            Token::From | Token::Where | Token::Group | Token::Having
            | Token::Order | Token::Limit | Token::Inner | Token::Left
            | Token::Right | Token::Join | Token::On | Token::And
            | Token::Or | Token::Offset
        )
    }

    fn parse_table_ref(&mut self) -> Result<TableRef, String> {
        let table_name = self.expect_identifier()?;
        let alias = if self.peek() == Token::As {
            self.advance();
            Some(self.expect_identifier()?)
        } else if self.peek_is_identifier() && !self.peek_is_keyword() {
            Some(self.expect_identifier()?)
        } else {
            None
        };
        Ok(TableRef { table_name, alias })
    }

    fn parse_joins(&mut self) -> Result<Vec<JoinClause>, String> {
        let mut joins = Vec::new();
        loop {
            let join_type = match self.peek() {
                Token::Inner => {
                    self.advance();
                    self.expect(&Token::Join)?;
                    JoinType::Inner
                }
                Token::Left => {
                    self.advance();
                    self.expect(&Token::Join)?;
                    JoinType::Left
                }
                Token::Right => {
                    self.advance();
                    self.expect(&Token::Join)?;
                    JoinType::Right
                }
                Token::Join => {
                    self.advance();
                    JoinType::Inner // bare JOIN = INNER JOIN
                }
                _ => break,
            };
            let table = self.parse_table_ref()?;
            self.expect(&Token::On)?;
            let on_condition = self.parse_expr()?;
            joins.push(JoinClause { join_type, table, on_condition });
        }
        Ok(joins)
    }

    fn parse_order_by_list(&mut self) -> Result<Vec<OrderByItem>, String> {
        let mut items = vec![self.parse_order_by_item()?];
        while self.peek() == Token::Comma {
            self.advance();
            items.push(self.parse_order_by_item()?);
        }
        Ok(items)
    }

    fn parse_order_by_item(&mut self) -> Result<OrderByItem, String> {
        let expr = self.parse_expr()?;
        let ascending = match self.peek() {
            Token::Asc => { self.advance(); true }
            Token::Desc => { self.advance(); false }
            _ => true,
        };
        Ok(OrderByItem { expr, ascending })
    }

    // ── UPDATE ──────────────────────────────────────────────────

    fn parse_update(&mut self) -> Result<Statement, String> {
        self.expect(&Token::Update)?;
        let table_name = self.expect_identifier()?;
        self.expect(&Token::Set)?;

        let mut assignments = Vec::new();
        loop {
            let col = self.expect_identifier()?;
            self.expect(&Token::Equals)?;
            let val = self.parse_expr()?;
            assignments.push((col, val));
            if self.peek() == Token::Comma {
                self.advance();
            } else {
                break;
            }
        }

        let where_clause = if self.peek() == Token::Where {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(Statement::Update(UpdateStmt { table_name, assignments, where_clause }))
    }

    // ── DELETE ──────────────────────────────────────────────────

    fn parse_delete(&mut self) -> Result<Statement, String> {
        self.expect(&Token::Delete)?;
        self.expect(&Token::From)?;
        let table_name = self.expect_identifier()?;

        let where_clause = if self.peek() == Token::Where {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(Statement::Delete(DeleteStmt { table_name, where_clause }))
    }

    // ── Expression Parser (precedence climbing) ─────────────────

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and_expr()?;
        while self.peek() == Token::Or {
            self.advance();
            let right = self.parse_and_expr()?;
            left = Expr::BinaryOp(Box::new(left), BinaryOp::Or, Box::new(right));
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_not_expr()?;
        while self.peek() == Token::And {
            self.advance();
            let right = self.parse_not_expr()?;
            left = Expr::BinaryOp(Box::new(left), BinaryOp::And, Box::new(right));
        }
        Ok(left)
    }

    fn parse_not_expr(&mut self) -> Result<Expr, String> {
        if self.peek() == Token::Not {
            self.advance();
            let expr = self.parse_not_expr()?;
            return Ok(Expr::UnaryOp(UnaryOp::Not, Box::new(expr)));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let left = self.parse_addition()?;

        // IS [NOT] NULL
        if self.peek() == Token::Is {
            self.advance();
            let is_not = if self.peek() == Token::Not {
                self.advance();
                true
            } else {
                false
            };
            self.expect(&Token::Null)?;
            return Ok(Expr::IsNull(Box::new(left), is_not));
        }

        // [NOT] IN (...)
        let (negate, check_in) = if self.peek() == Token::Not {
            let saved = self.pos;
            self.advance();
            if self.peek() == Token::In {
                (true, true)
            } else if self.peek() == Token::Between {
                self.pos = saved;
                (false, false) // will be handled below after re-check
            } else if self.peek() == Token::Like {
                self.pos = saved;
                (false, false)
            } else {
                self.pos = saved;
                (false, false)
            }
        } else {
            (false, false)
        };

        if check_in || self.peek() == Token::In {
            let is_not = if check_in {
                self.advance(); // consume IN
                negate
            } else {
                self.advance(); // consume IN
                false
            };
            self.expect(&Token::LeftParen)?;
            let list = self.parse_expr_list()?;
            self.expect(&Token::RightParen)?;
            return Ok(Expr::InList(Box::new(left), list, is_not));
        }

        // [NOT] BETWEEN a AND b
        if self.peek() == Token::Not {
            let saved = self.pos;
            self.advance();
            if self.peek() == Token::Between {
                self.advance();
                let low = self.parse_addition()?;
                self.expect(&Token::And)?;
                let high = self.parse_addition()?;
                return Ok(Expr::BetweenExpr(Box::new(left), Box::new(low), Box::new(high), true));
            }
            if self.peek() == Token::Like {
                self.advance();
                let pattern = self.parse_addition()?;
                return Ok(Expr::LikeExpr(Box::new(left), Box::new(pattern), true));
            }
            self.pos = saved;
        }

        if self.peek() == Token::Between {
            self.advance();
            let low = self.parse_addition()?;
            self.expect(&Token::And)?;
            let high = self.parse_addition()?;
            return Ok(Expr::BetweenExpr(Box::new(left), Box::new(low), Box::new(high), false));
        }

        // LIKE
        if self.peek() == Token::Like {
            self.advance();
            let pattern = self.parse_addition()?;
            return Ok(Expr::LikeExpr(Box::new(left), Box::new(pattern), false));
        }

        // Standard comparisons
        let op = match self.peek() {
            Token::Equals => BinaryOp::Eq,
            Token::NotEquals => BinaryOp::NotEq,
            Token::LessThan => BinaryOp::Lt,
            Token::GreaterThan => BinaryOp::Gt,
            Token::LessEqual => BinaryOp::LtEq,
            Token::GreaterEqual => BinaryOp::GtEq,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_addition()?;
        Ok(Expr::BinaryOp(Box::new(left), op, Box::new(right)))
    }

    fn parse_addition(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplication()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOp::Plus,
                Token::Minus => BinaryOp::Minus,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = Expr::BinaryOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_multiplication(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Asterisk => BinaryOp::Multiply,
                Token::Slash => BinaryOp::Divide,
                Token::Percent => BinaryOp::Modulo,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinaryOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.peek() == Token::Minus {
            self.advance();
            let expr = self.parse_primary()?;
            return Ok(Expr::UnaryOp(UnaryOp::Neg, Box::new(expr)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Token::IntegerLiteral(_) => {
                if let Token::IntegerLiteral(n) = self.advance() {
                    Ok(Expr::IntegerLiteral(n))
                } else {
                    unreachable!()
                }
            }
            Token::FloatLiteral(_) => {
                if let Token::FloatLiteral(n) = self.advance() {
                    Ok(Expr::FloatLiteral(n))
                } else {
                    unreachable!()
                }
            }
            Token::StringLiteral(_) => {
                if let Token::StringLiteral(s) = self.advance() {
                    Ok(Expr::StringLiteral(s))
                } else {
                    unreachable!()
                }
            }
            Token::True => { self.advance(); Ok(Expr::BooleanLiteral(true)) }
            Token::False => { self.advance(); Ok(Expr::BooleanLiteral(false)) }
            Token::Null => { self.advance(); Ok(Expr::Null) }
            Token::LeftParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RightParen)?;
                Ok(expr)
            }
            // Aggregate functions
            Token::Count | Token::Sum | Token::Avg | Token::Min | Token::Max => {
                let name = format!("{:?}", self.advance()).to_uppercase();
                let name = match name.as_str() {
                    _ => name,
                };
                self.expect(&Token::LeftParen)?;
                let distinct = if self.peek() == Token::Distinct {
                    self.advance();
                    true
                } else {
                    false
                };
                // COUNT(*) special case
                let args = if self.peek() == Token::Asterisk {
                    self.advance();
                    vec![Expr::ColumnRef(ColumnRefExpr { table: None, column: "*".to_string() })]
                } else {
                    self.parse_expr_list()?
                };
                self.expect(&Token::RightParen)?;
                Ok(Expr::Function(FunctionCall { name, args, distinct }))
            }
            Token::Identifier(_) => {
                if let Token::Identifier(name) = self.advance() {
                    // Check for function call
                    if self.peek() == Token::LeftParen {
                        self.advance();
                        let args = if self.peek() == Token::RightParen {
                            Vec::new()
                        } else {
                            self.parse_expr_list()?
                        };
                        self.expect(&Token::RightParen)?;
                        return Ok(Expr::Function(FunctionCall {
                            name: name.to_uppercase(),
                            args,
                            distinct: false,
                        }));
                    }
                    // Check for table.column
                    if self.peek() == Token::Dot {
                        self.advance();
                        let col = self.expect_identifier()?;
                        return Ok(Expr::ColumnRef(ColumnRefExpr {
                            table: Some(name),
                            column: col,
                        }));
                    }
                    Ok(Expr::ColumnRef(ColumnRefExpr { table: None, column: name }))
                } else {
                    unreachable!()
                }
            }
            tok => Err(format!("Unexpected token in expression: {}", tok)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::Tokenizer;

    fn parse_sql(sql: &str) -> Statement {
        let mut tokenizer = Tokenizer::new(sql);
        let tokens = tokenizer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse().unwrap()
    }

    #[test]
    fn test_create_table() {
        let stmt = parse_sql("CREATE TABLE users (id INT PRIMARY KEY, name TEXT, age INT)");
        match stmt {
            Statement::CreateTable(ct) => {
                assert_eq!(ct.table_name, "users");
                assert_eq!(ct.columns.len(), 3);
                assert!(ct.columns[0].primary_key);
            }
            _ => panic!("Expected CreateTable"),
        }
    }

    #[test]
    fn test_insert() {
        let stmt = parse_sql("INSERT INTO users (name, age) VALUES ('Alice', 30)");
        match stmt {
            Statement::Insert(ins) => {
                assert_eq!(ins.table_name, "users");
                assert_eq!(ins.columns.as_ref().unwrap().len(), 2);
                assert_eq!(ins.values.len(), 1);
            }
            _ => panic!("Expected Insert"),
        }
    }

    #[test]
    fn test_select_with_where() {
        let stmt = parse_sql("SELECT name, age FROM users WHERE age > 25 ORDER BY name ASC LIMIT 10");
        match stmt {
            Statement::Select(sel) => {
                assert_eq!(sel.columns.len(), 2);
                assert!(sel.where_clause.is_some());
                assert_eq!(sel.order_by.len(), 1);
                assert!(sel.limit.is_some());
            }
            _ => panic!("Expected Select"),
        }
    }

    #[test]
    fn test_join() {
        let stmt = parse_sql(
            "SELECT u.name, o.total FROM users u INNER JOIN orders o ON u.id = o.user_id"
        );
        match stmt {
            Statement::Select(sel) => {
                assert_eq!(sel.joins.len(), 1);
                assert_eq!(sel.joins[0].join_type, JoinType::Inner);
            }
            _ => panic!("Expected Select"),
        }
    }

    #[test]
    fn test_group_by_having() {
        let stmt = parse_sql(
            "SELECT department, COUNT(*) AS cnt FROM employees GROUP BY department HAVING COUNT(*) > 5"
        );
        match stmt {
            Statement::Select(sel) => {
                assert_eq!(sel.group_by.len(), 1);
                assert!(sel.having.is_some());
            }
            _ => panic!("Expected Select"),
        }
    }
}
