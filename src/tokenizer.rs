/// SQL Token types produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Select,
    From,
    Where,
    Insert,
    Into,
    Values,
    Create,
    Table,
    Update,
    Set,
    Delete,
    Drop,
    Join,
    Inner,
    Left,
    Right,
    On,
    Order,
    By,
    Group,
    Having,
    Limit,
    And,
    Or,
    Not,
    Null,
    As,
    Asc,
    Desc,
    Int,
    Integer,
    Text,
    Varchar,
    Float,
    Boolean,
    Bool,
    True,
    False,
    Primary,
    Key,
    Is,
    In,
    Between,
    Like,
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Distinct,
    Offset,

    // Identifiers and literals
    Identifier(String),
    StringLiteral(String),
    IntegerLiteral(i64),
    FloatLiteral(f64),

    // Operators
    Equals,        // =
    NotEquals,     // != or <>
    LessThan,      // <
    GreaterThan,   // >
    LessEqual,     // <=
    GreaterEqual,  // >=
    Plus,          // +
    Minus,         // -
    Asterisk,      // *
    Slash,         // /
    Percent,       // %

    // Punctuation
    Comma,
    Semicolon,
    LeftParen,
    RightParen,
    Dot,

    // End
    Eof,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Select => write!(f, "SELECT"),
            Token::From => write!(f, "FROM"),
            Token::Where => write!(f, "WHERE"),
            Token::Identifier(s) => write!(f, "'{}'", s),
            Token::StringLiteral(s) => write!(f, "\"{}\"", s),
            Token::IntegerLiteral(n) => write!(f, "{}", n),
            Token::FloatLiteral(n) => write!(f, "{}", n),
            Token::Eof => write!(f, "EOF"),
            other => write!(f, "{:?}", other),
        }
    }
}

pub struct Tokenizer {
    input: Vec<char>,
    pos: usize,
}

impl Tokenizer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                tokens.push(Token::Eof);
                break;
            }
            let token = self.next_token()?;
            tokens.push(token);
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        self.pos += 1;
        ch
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        // Skip single-line comments
        if self.pos + 1 < self.input.len() && self.input[self.pos] == '-' && self.input[self.pos + 1] == '-' {
            while self.pos < self.input.len() && self.input[self.pos] != '\n' {
                self.pos += 1;
            }
            self.skip_whitespace();
        }
    }

    fn next_token(&mut self) -> Result<Token, String> {
        let ch = self.peek().unwrap();

        match ch {
            '\'' => self.read_string(),
            '0'..='9' => self.read_number(),
            'a'..='z' | 'A'..='Z' | '_' => self.read_identifier_or_keyword(),
            '=' => { self.advance(); Ok(Token::Equals) }
            '!' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::NotEquals)
                } else {
                    Err("Expected '=' after '!'".to_string())
                }
            }
            '<' => {
                self.advance();
                match self.peek() {
                    Some('=') => { self.advance(); Ok(Token::LessEqual) }
                    Some('>') => { self.advance(); Ok(Token::NotEquals) }
                    _ => Ok(Token::LessThan),
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::GreaterEqual)
                } else {
                    Ok(Token::GreaterThan)
                }
            }
            '+' => { self.advance(); Ok(Token::Plus) }
            '-' => { self.advance(); Ok(Token::Minus) }
            '*' => { self.advance(); Ok(Token::Asterisk) }
            '/' => { self.advance(); Ok(Token::Slash) }
            '%' => { self.advance(); Ok(Token::Percent) }
            ',' => { self.advance(); Ok(Token::Comma) }
            ';' => { self.advance(); Ok(Token::Semicolon) }
            '(' => { self.advance(); Ok(Token::LeftParen) }
            ')' => { self.advance(); Ok(Token::RightParen) }
            '.' => { self.advance(); Ok(Token::Dot) }
            _ => Err(format!("Unexpected character: '{}'", ch)),
        }
    }

    fn read_string(&mut self) -> Result<Token, String> {
        self.advance(); // skip opening quote
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('\'') => {
                    // Handle escaped quotes ('')
                    if self.peek() == Some('\'') {
                        self.advance();
                        s.push('\'');
                    } else {
                        return Ok(Token::StringLiteral(s));
                    }
                }
                Some(ch) => s.push(ch),
                None => return Err("Unterminated string literal".to_string()),
            }
        }
    }

    fn read_number(&mut self) -> Result<Token, String> {
        let start = self.pos;
        let mut has_dot = false;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.advance();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                self.advance();
            } else {
                break;
            }
        }
        let num_str: String = self.input[start..self.pos].iter().collect();
        if has_dot {
            num_str.parse::<f64>()
                .map(Token::FloatLiteral)
                .map_err(|e| format!("Invalid float: {}", e))
        } else {
            num_str.parse::<i64>()
                .map(Token::IntegerLiteral)
                .map_err(|e| format!("Invalid integer: {}", e))
        }
    }

    fn read_identifier_or_keyword(&mut self) -> Result<Token, String> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let word: String = self.input[start..self.pos].iter().collect();
        let token = match word.to_uppercase().as_str() {
            "SELECT" => Token::Select,
            "FROM" => Token::From,
            "WHERE" => Token::Where,
            "INSERT" => Token::Insert,
            "INTO" => Token::Into,
            "VALUES" => Token::Values,
            "CREATE" => Token::Create,
            "TABLE" => Token::Table,
            "UPDATE" => Token::Update,
            "SET" => Token::Set,
            "DELETE" => Token::Delete,
            "DROP" => Token::Drop,
            "JOIN" => Token::Join,
            "INNER" => Token::Inner,
            "LEFT" => Token::Left,
            "RIGHT" => Token::Right,
            "ON" => Token::On,
            "ORDER" => Token::Order,
            "BY" => Token::By,
            "GROUP" => Token::Group,
            "HAVING" => Token::Having,
            "LIMIT" => Token::Limit,
            "AND" => Token::And,
            "OR" => Token::Or,
            "NOT" => Token::Not,
            "NULL" => Token::Null,
            "AS" => Token::As,
            "ASC" => Token::Asc,
            "DESC" => Token::Desc,
            "INT" => Token::Int,
            "INTEGER" => Token::Integer,
            "TEXT" => Token::Text,
            "VARCHAR" => Token::Varchar,
            "FLOAT" => Token::Float,
            "BOOLEAN" => Token::Boolean,
            "BOOL" => Token::Bool,
            "TRUE" => Token::True,
            "FALSE" => Token::False,
            "PRIMARY" => Token::Primary,
            "KEY" => Token::Key,
            "IS" => Token::Is,
            "IN" => Token::In,
            "BETWEEN" => Token::Between,
            "LIKE" => Token::Like,
            "COUNT" => Token::Count,
            "SUM" => Token::Sum,
            "AVG" => Token::Avg,
            "MIN" => Token::Min,
            "MAX" => Token::Max,
            "DISTINCT" => Token::Distinct,
            "OFFSET" => Token::Offset,
            _ => Token::Identifier(word),
        };
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_star() {
        let mut t = Tokenizer::new("SELECT * FROM users;");
        let tokens = t.tokenize().unwrap();
        assert_eq!(tokens, vec![
            Token::Select, Token::Asterisk, Token::From,
            Token::Identifier("users".to_string()), Token::Semicolon, Token::Eof,
        ]);
    }

    #[test]
    fn test_string_literal() {
        let mut t = Tokenizer::new("'hello world'");
        let tokens = t.tokenize().unwrap();
        assert_eq!(tokens[0], Token::StringLiteral("hello world".to_string()));
    }

    #[test]
    fn test_numbers() {
        let mut t = Tokenizer::new("42 3.14");
        let tokens = t.tokenize().unwrap();
        assert_eq!(tokens[0], Token::IntegerLiteral(42));
        assert_eq!(tokens[1], Token::FloatLiteral(3.14));
    }

    #[test]
    fn test_operators() {
        let mut t = Tokenizer::new("= != < > <= >= <>");
        let tokens = t.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Equals);
        assert_eq!(tokens[1], Token::NotEquals);
        assert_eq!(tokens[2], Token::LessThan);
        assert_eq!(tokens[3], Token::GreaterThan);
        assert_eq!(tokens[4], Token::LessEqual);
        assert_eq!(tokens[5], Token::GreaterEqual);
        assert_eq!(tokens[6], Token::NotEquals);
    }
}
