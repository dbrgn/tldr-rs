//! Shared types used in tealdeer.

use std::fmt;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum OsType {
    Linux,
    OsX,
    SunOs,
    Windows,
    Other,
}

impl fmt::Display for OsType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Linux => write!(f, "Linux"),
            Self::OsX => write!(f, "macOS / BSD"),
            Self::SunOs => write!(f, "SunOS"),
            Self::Windows => write!(f, "Windows"),
            Self::Other => write!(f, "Unknown OS"),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorOptions {
    Always,
    Auto,
    Never,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LineType {
    Empty,
    Title(String),
    Description(String),
    ExampleText(String),
    ExampleCode(String),
    Other(String),
}

impl<'a> From<&'a str> for LineType {
    /// Convert a string slice to a `LineType`. Newlines and trailing whitespace are trimmed.
    fn from(line: &'a str) -> Self {
        let trimmed: &str = line.trim_end();
        let mut chars = trimmed.chars();
        match chars.next() {
            None => Self::Empty,
            Some('#') => Self::Title(
                trimmed
                    .trim_start_matches(|chr: char| chr == '#' || chr.is_whitespace())
                    .into(),
            ),
            Some('>') => Self::Description(
                trimmed
                    .trim_start_matches(|chr: char| chr == '>' || chr.is_whitespace())
                    .into(),
            ),
            Some(' ') => Self::ExampleCode(trimmed.trim_start_matches(char::is_whitespace).into()),
            Some(_) => Self::ExampleText(trimmed.into()),
        }
    }
}

impl LineType {
    /// Support for old format.
    /// TODO: Remove once old format has been phased out!
    pub fn from_v1(line: &str) -> Self {
        let trimmed = line.trim();
        let mut chars = trimmed.chars();
        match chars.next() {
            None => Self::Empty,
            Some('#') => Self::Title(
                trimmed
                    .trim_start_matches(|chr: char| chr == '#' || chr.is_whitespace())
                    .into(),
            ),
            Some('>') => Self::Description(
                trimmed
                    .trim_start_matches(|chr: char| chr == '>' || chr.is_whitespace())
                    .into(),
            ),
            Some('-') => Self::ExampleText(
                trimmed
                    .trim_start_matches(|chr: char| chr == '-' || chr.is_whitespace())
                    .into(),
            ),
            Some('`') if chars.last() == Some('`') => Self::ExampleCode(
                trimmed
                    .trim_matches(|chr: char| chr == '`' || chr.is_whitespace())
                    .into(),
            ),
            Some(_) => Self::Other(trimmed.into()),
        }
    }
}

/// The reason why a certain path (e.g. config path or cache dir) was chosen.
#[derive(Debug, PartialEq)]
pub enum PathSource {
    /// OS convention (e.g. XDG on Linux)
    OsConvention,
    /// Env variable (TEALDEER_*)
    EnvVar,

    #[allow(dead_code)] // Waiting for Pull Request #141
    /// Config file variable
    ConfigVar,
    /// CLI argument override
    Override,
}

impl fmt::Display for PathSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::OsConvention => "OS convention",
                Self::EnvVar => "env variable",
                Self::ConfigVar => "config file variable",
                Self::Override => "override by flag",
            }
        )
    }
}

#[cfg(test)]
mod test {
    use super::LineType;

    #[test]
    fn test_linetype_from_str() {
        assert_eq!(LineType::from(""), LineType::Empty);
        assert_eq!(LineType::from(" \n \r"), LineType::Empty);
        assert_eq!(
            LineType::from("# Hello there"),
            LineType::Title("Hello there".into())
        );
        assert_eq!(
            LineType::from("> tis a description \n"),
            LineType::Description("tis a description".into())
        );
        assert_eq!(
            LineType::from("some command "),
            LineType::ExampleText("some command".into())
        );
        assert_eq!(
            LineType::from("    $ cargo run "),
            LineType::ExampleCode("$ cargo run".into())
        );
    }
}
