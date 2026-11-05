//! Minimal, read-only Valve KeyValues parser.
//!
//! This deliberately does not try to serialize or patch VDF files. VibranceGUI
//! only reads Steam metadata and launch options in v0.1.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct Node {
    pub key: String,
    pub value: Option<String>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn child(&self, key: &str) -> Option<&Node> {
        self.children
            .iter()
            .find(|n| n.key.eq_ignore_ascii_case(key))
    }

    pub fn path(&self, keys: &[&str]) -> Option<&Node> {
        let mut node = self;
        for key in keys {
            node = node.child(key)?;
        }
        Some(node)
    }

    pub fn value_or_empty(&self) -> &str {
        self.value.as_deref().unwrap_or("")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VdfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error at byte {offset}: {message}")]
    Parse { offset: usize, message: String },
}

pub fn parse_file(path: impl AsRef<Path>) -> Result<Node, VdfError> {
    let text = fs::read_to_string(path)?;
    parse_str(&text)
}

pub fn parse_str(text: &str) -> Result<Node, VdfError> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    let mut nodes = p.parse_block(false)?;
    if nodes.len() == 1 && nodes[0].value.is_none() {
        Ok(nodes.remove(0))
    } else {
        Ok(Node {
            key: "root".to_string(),
            value: None,
            children: nodes,
        })
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn parse_block(&mut self, in_brace: bool) -> Result<Vec<Node>, VdfError> {
        let mut nodes = Vec::new();
        loop {
            self.skip_ws_comments();
            if self.eof() {
                if in_brace {
                    return Err(self.err("unexpected end of file (missing '}')"));
                }
                break;
            }
            if self.peek() == Some(b'}') {
                if !in_brace {
                    return Err(self.err("unexpected '}'"));
                }
                self.pos += 1;
                break;
            }

            let key = self.read_value()?;
            self.skip_ws_comments();

            if self.peek() == Some(b'{') {
                self.pos += 1;
                let children = self.parse_block(true)?;
                nodes.push(Node {
                    key,
                    value: None,
                    children,
                });
            } else {
                let value = self.read_value()?;
                nodes.push(Node {
                    key,
                    value: Some(value),
                    children: Vec::new(),
                });
            }
        }
        Ok(nodes)
    }

    fn read_value(&mut self) -> Result<String, VdfError> {
        self.skip_ws_comments();
        match self.peek() {
            Some(b'"') => self.read_quoted(),
            Some(b'{') | Some(b'}') => Err(self.err("expected a key/value, found a brace")),
            Some(_) => {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_ascii_whitespace() || c == b'{' || c == b'}' {
                        break;
                    }
                    self.pos += 1;
                }
                if start == self.pos {
                    return Err(self.err("empty bare token"));
                }
                Ok(String::from_utf8_lossy(&self.bytes[start..self.pos]).into_owned())
            }
            None => Err(self.err("unexpected end of file")),
        }
    }

    fn read_quoted(&mut self) -> Result<String, VdfError> {
        self.pos += 1; // opening quote
        let mut out = Vec::new();
        while let Some(c) = self.peek() {
            self.pos += 1;
            match c {
                b'"' => return Ok(String::from_utf8_lossy(&out).into_owned()),
                b'\\' => {
                    let Some(escaped) = self.peek() else {
                        return Err(self.err("unterminated escape sequence"));
                    };
                    self.pos += 1;
                    match escaped {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'\\' => out.push(b'\\'),
                        b'"' => out.push(b'"'),
                        other => {
                            // Valve accepts unknown escapes literally.
                            out.push(other);
                        }
                    }
                }
                other => out.push(other),
            }
        }
        Err(self.err("unterminated quoted string"))
    }

    fn skip_ws_comments(&mut self) {
        loop {
            while let Some(c) = self.peek() {
                if c.is_ascii_whitespace() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if self.peek() == Some(b'/') && self.peek_n(1) == Some(b'/') {
                self.pos += 2;
                while let Some(c) = self.peek() {
                    self.pos += 1;
                    if c == b'\n' {
                        break;
                    }
                }
                continue;
            }
            break;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_n(&self, n: usize) -> Option<u8> {
        self.bytes.get(self.pos + n).copied()
    }

    fn eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn err(&self, msg: &str) -> VdfError {
        VdfError::Parse {
            offset: self.pos,
            message: msg.to_string(),
        }
    }
}

// `thiserror` is pulled in through a tiny dependency; keep the parser testable.
// If the dependency is removed later, this can be replaced with std::error::Error.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_app_manifest() {
        let text = r#"
            "AppState"
            {
                "appid" "12345"
                "name" "Example Game"
                "installdir" "Example"
                "StateFlags" "4"
            }
        "#;
        let root = parse_str(text).unwrap();
        assert_eq!(root.child("appid").unwrap().value_or_empty(), "12345");
        assert_eq!(root.child("name").unwrap().value_or_empty(), "Example Game");
    }

    #[test]
    fn parses_library_folders() {
        let text = r#"
            "libraryfolders"
            {
                "0"
                {
                    "path" "/home/u/.local/share/Steam"
                }
                "1"
                {
                    "path" "/mnt/games/SteamLibrary"
                }
            }
        "#;
        let root = parse_str(text).unwrap();
        let paths: Vec<_> = root
            .children
            .iter()
            .filter_map(|n| n.child("path"))
            .map(|n| n.value_or_empty().to_string())
            .collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[1], "/mnt/games/SteamLibrary");
    }

    #[test]
    fn parses_localconfig_shape() {
        let text = r#"
            "UserLocalConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "apps"
                            {
                                "620"
                                {
                                    "LaunchOptions" "ENABLE_VKSUMI=1 %command%"
                                }
                            }
                        }
                    }
                }
            }
        "#;
        let root = parse_str(text).unwrap();
        let apps = root.path(&["Software", "Valve", "Steam", "apps"]).unwrap();
        let app = apps.child("620").unwrap();
        assert_eq!(
            app.child("LaunchOptions").unwrap().value_or_empty(),
            "ENABLE_VKSUMI=1 %command%"
        );
    }

    #[test]
    fn handles_escapes_and_comments() {
        let text = r#"
            // comment
            "root"
            {
                "key" "a \"quoted\" value\\path"
            }
        "#;
        let root = parse_str(text).unwrap();
        assert_eq!(
            root.child("key").unwrap().value_or_empty(),
            "a \"quoted\" value\\path"
        );
    }

    #[test]
    fn rejects_unclosed_block() {
        assert!(parse_str(r#""root" { "key" "value""#).is_err());
    }
}
