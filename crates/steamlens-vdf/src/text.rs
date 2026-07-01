use thiserror::Error;

#[derive(Debug, Error)]
pub enum TextVdfError {
    #[error("unexpected end of input")]
    Truncated,

    #[error("unexpected token at line {line}: {found:?}")]
    UnexpectedToken { line: usize, found: String },

    #[error("invalid escape sequence at line {line}: {sequence:?}")]
    InvalidEscape { line: usize, sequence: String },

    #[error("invalid UTF-8 in input")]
    InvalidUtf8,

    #[error("maximum nesting depth exceeded at line {line}")]
    MaxDepthExceeded { line: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextValue {
    String(std::string::String),
    Block(Vec<(std::string::String, TextValue)>),
}

impl TextValue {
    pub fn get(&self, key: &str) -> Option<&TextValue> {
        match self {
            TextValue::Block(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            TextValue::String(_) => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            TextValue::String(s) => Some(s.as_str()),
            TextValue::Block(_) => None,
        }
    }

    pub fn as_block(&self) -> Option<&[(std::string::String, TextValue)]> {
        match self {
            TextValue::Block(pairs) => Some(pairs.as_slice()),
            TextValue::String(_) => None,
        }
    }

    pub fn path(&self, segments: &[&str]) -> Option<&TextValue> {
        let mut current = self;
        for &seg in segments {
            current = current.get(seg)?;
        }
        Some(current)
    }
}

const MAX_NESTING_DEPTH: usize = 128;

pub fn parse(input: &str) -> Result<TextValue, TextVdfError> {
    let mut parser = Parser::new(input);
    let pairs = parser.read_pairs(false, 1)?;
    Ok(TextValue::Block(pairs))
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser {
            input,
            pos: 0,
            line: 1,
        }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance(&mut self, count: usize) {
        let slice = &self.input[self.pos..self.pos + count];
        for ch in slice.chars() {
            if ch == '\n' {
                self.line += 1;
            }
        }
        self.pos += count;
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            let rem = self.remaining();
            if rem.is_empty() {
                break;
            }
            let ch = rem.chars().next().unwrap();
            if ch.is_ascii_whitespace() {
                self.advance(1);
            } else if rem.starts_with("//") {
                let end = rem.find('\n').map(|i| i + 1).unwrap_or(rem.len());
                self.advance(end);
            } else {
                break;
            }
        }
    }

    fn read_quoted_string(&mut self) -> Result<std::string::String, TextVdfError> {
        debug_assert_eq!(self.peek(), Some('"'));
        self.advance(1);

        let mut out = std::string::String::new();
        loop {
            match self.peek() {
                None => return Err(TextVdfError::Truncated),
                Some('"') => {
                    self.advance(1);
                    return Ok(out);
                }
                Some('\\') => {
                    let line = self.line;
                    self.advance(1);
                    match self.peek() {
                        None => return Err(TextVdfError::Truncated),
                        Some('n') => {
                            self.advance(1);
                            out.push('\n');
                        }
                        Some('t') => {
                            self.advance(1);
                            out.push('\t');
                        }
                        Some('\\') => {
                            self.advance(1);
                            out.push('\\');
                        }
                        Some('"') => {
                            self.advance(1);
                            out.push('"');
                        }
                        Some(other) => {
                            return Err(TextVdfError::InvalidEscape {
                                line,
                                sequence: format!("\\{other}"),
                            });
                        }
                    }
                }
                Some(ch) => {
                    let len = ch.len_utf8();
                    out.push(ch);
                    self.advance(len);
                }
            }
        }
    }

    fn read_pairs(
        &mut self,
        inside_block: bool,
        depth: usize,
    ) -> Result<Vec<(std::string::String, TextValue)>, TextVdfError> {
        if depth > MAX_NESTING_DEPTH {
            return Err(TextVdfError::MaxDepthExceeded { line: self.line });
        }

        let mut pairs = Vec::new();

        loop {
            self.skip_whitespace_and_comments();

            match self.peek() {
                None => {
                    if inside_block {
                        return Err(TextVdfError::Truncated);
                    }
                    return Ok(pairs);
                }
                Some('}') => {
                    if inside_block {
                        self.advance(1);
                        return Ok(pairs);
                    } else {
                        return Err(TextVdfError::UnexpectedToken {
                            line: self.line,
                            found: "}".to_owned(),
                        });
                    }
                }
                Some('"') => {
                    let key = self.read_quoted_string()?;
                    self.skip_whitespace_and_comments();

                    match self.peek() {
                        None => return Err(TextVdfError::Truncated),
                        Some('{') => {
                            self.advance(1);
                            let children = self.read_pairs(true, depth + 1)?;
                            pairs.push((key, TextValue::Block(children)));
                        }
                        Some('"') => {
                            let value = self.read_quoted_string()?;
                            pairs.push((key, TextValue::String(value)));
                        }
                        Some(other) => {
                            return Err(TextVdfError::UnexpectedToken {
                                line: self.line,
                                found: other.to_string(),
                            });
                        }
                    }
                }
                Some(other) => {
                    return Err(TextVdfError::UnexpectedToken {
                        line: self.line,
                        found: other.to_string(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_kv_at_root() {
        let input = r#""key" "value""#;
        let result = parse(input).unwrap();
        let pairs = result.as_block().unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "key");
        assert_eq!(pairs[0].1.as_str(), Some("value"));
    }

    #[test]
    fn parse_appmanifest_shape() {
        let input = r#"
"AppState"
{
    "appid"         "105600"
    "name"          "Terraria"
    "StateFlags"    "4"
    "LastPlayed"    "1700000000"
}
"#;
        let result = parse(input).unwrap();
        let top = result.get("AppState").unwrap();
        assert_eq!(top.get("appid").unwrap().as_str(), Some("105600"));
        assert_eq!(top.get("name").unwrap().as_str(), Some("Terraria"));
        assert_eq!(top.get("LastPlayed").unwrap().as_str(), Some("1700000000"));
    }

    #[test]
    fn parse_libraryfolders_shape() {
        let input = r#"
"libraryfolders"
{
    "0"
    {
        "path"  "/home/user/.local/share/Steam"
        "apps"
        {
            "105600"    "828396557"
        }
    }
}
"#;
        let result = parse(input).unwrap();
        let lf = result.get("libraryfolders").unwrap();
        let lib0 = lf.get("0").unwrap();
        assert_eq!(
            lib0.get("path").unwrap().as_str(),
            Some("/home/user/.local/share/Steam")
        );
        let apps = lib0.get("apps").unwrap();
        assert_eq!(apps.get("105600").unwrap().as_str(), Some("828396557"));
    }

    #[test]
    fn parse_handles_escaped_quote_in_value() {
        let input = r#""msg" "say \"hello\"""#;
        let result = parse(input).unwrap();
        let pairs = result.as_block().unwrap();
        assert_eq!(pairs[0].1.as_str(), Some(r#"say "hello""#));
    }

    #[test]
    fn parse_handles_nested_blocks_three_deep() {
        let input = r#"
"root"
{
    "level1"
    {
        "level2"
        {
            "leaf"  "value"
        }
    }
}
"#;
        let result = parse(input).unwrap();
        let leaf = result
            .get("root")
            .and_then(|v| v.get("level1"))
            .and_then(|v| v.get("level2"))
            .and_then(|v| v.get("leaf"))
            .and_then(|v| v.as_str());
        assert_eq!(leaf, Some("value"));
    }

    #[test]
    fn parse_truncated_returns_error() {
        let input = r#""key""#;
        let result = parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn parse_unmatched_close_brace_returns_error() {
        let input = r#"}"#;
        let result = parse(input);
        assert!(matches!(result, Err(TextVdfError::UnexpectedToken { .. })));
    }

    #[test]
    fn parse_comment_skipped() {
        let input = r#"
// this is a comment
"key" "value"
"#;
        let result = parse(input).unwrap();
        let pairs = result.as_block().unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "key");
    }

    #[test]
    fn parse_escape_newline_and_tab() {
        let input = r#""k" "line1\nline2\tend""#;
        let result = parse(input).unwrap();
        let pairs = result.as_block().unwrap();
        assert_eq!(pairs[0].1.as_str(), Some("line1\nline2\tend"));
    }

    #[test]
    fn parse_unknown_escape_returns_error() {
        let input = r#""k" "bad\xescape""#;
        let result = parse(input);
        assert!(matches!(result, Err(TextVdfError::InvalidEscape { .. })));
    }

    #[test]
    fn path_traversal_happy() {
        let input = r#"
"root"
{
    "a"
    {
        "b"
        {
            "c" "found"
        }
    }
}
"#;
        let doc = parse(input).unwrap();
        let val = doc.path(&["root", "a", "b", "c"]).unwrap();
        assert_eq!(val.as_str(), Some("found"));
    }

    #[test]
    fn path_missing_key_returns_none() {
        let input = r#""root" { "a" { "b" "x" } }"#;
        let doc = parse(input).unwrap();
        assert!(doc.path(&["root", "a", "missing"]).is_none());
        assert!(doc.path(&["nope"]).is_none());
    }

    #[test]
    fn path_empty_segments_returns_self() {
        let input = r#""k" "v""#;
        let doc = parse(input).unwrap();
        assert!(std::ptr::eq(doc.path(&[]).unwrap(), &doc));
    }

    #[test]
    fn parse_exceeds_max_nesting_depth_returns_typed_error() {
        let depth = MAX_NESTING_DEPTH + 5;
        let mut input = String::new();
        for i in 0..depth {
            input.push_str(&format!("\"level{i}\" {{"));
        }
        let result = parse(&input);
        assert!(matches!(result, Err(TextVdfError::MaxDepthExceeded { .. })));
    }
}
