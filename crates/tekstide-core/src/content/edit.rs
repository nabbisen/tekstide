use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextDocumentEditError {
    ContainsNul,
}

impl fmt::Display for TextDocumentEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContainsNul => write!(formatter, "replacement text contains NUL"),
        }
    }
}

impl std::error::Error for TextDocumentEditError {}
