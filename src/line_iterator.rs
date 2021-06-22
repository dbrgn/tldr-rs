//! Code to split a `BufRead` instance into an iterator of `LineType`s.

use std::io::BufRead;

use crate::types::LineType;
use log::warn;

#[derive(Debug, PartialEq, Eq)]
pub enum TldrFormat {
    /// Not yet clear
    Undecided,
    /// The original format
    V1,
    /// The new format (see https://github.com/tldr-pages/tldr/pull/958)
    V2,
}

/// A `LineIterator` is initialized with a `BufReader` instance that contains the
/// entire Tldr page. It then implements `Iterator<Item = LineType>`.
#[derive(Debug)]
pub struct LineIterator<R: BufRead> {
    /// An instance of `R: BufRead`.
    reader: R,
    /// Whether the first line has already been processed or not.
    first_line: bool,
    /// Buffer for the current line. Used internally.
    current_line: String,
    /// The tldr page format.
    format: TldrFormat,
}

impl<R> LineIterator<R>
where
    R: BufRead,
{
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            first_line: true,
            current_line: String::new(),
            format: TldrFormat::Undecided,
        }
    }
}

impl<R: BufRead> Iterator for LineIterator<R> {
    type Item = LineType;

    fn next(&mut self) -> Option<LineType> {
        self.current_line.clear();
        let bytes_read = self.reader.read_line(&mut self.current_line);
        match bytes_read {
            Ok(0) => None,
            Err(e) => {
                warn!("Could not read line from reader: {:?}", e);
                None
            }
            Ok(_) => {
                // Handle new titles
                if self.first_line {
                    if self.current_line.starts_with('#') {
                        // It's the old format.
                        self.format = TldrFormat::V1;
                    } else {
                        // It's the new format! Drop next line.
                        // (Hmm, is there a way to do this without an allocation?)
                        let mut devnull = String::new();
                        if let Err(e) = self.reader.read_line(&mut devnull) {
                            warn!("Could not read line from reader: {:?}", e);
                            return None;
                        }
                        self.first_line = false;
                        self.format = TldrFormat::V2;
                        return Some(LineType::Title(self.current_line.trim_end().to_string()));
                    }
                }
                self.first_line = false;

                // Convert line to a `LineType` instance
                match self.format {
                    TldrFormat::V1 => Some(LineType::from_v1(&self.current_line[..])),
                    TldrFormat::V2 => Some(LineType::from(&self.current_line[..])),
                    TldrFormat::Undecided => panic!("Could not determine page format version"),
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::LineIterator;
    use crate::types::LineType;

    #[test]
    fn test_first_line_old_format() {
        let input = "# The Title\n\n";
        let mut lines = LineIterator::new(input.as_bytes());
        let title = lines.next().unwrap();
        assert_eq!(title, LineType::Title("The Title".to_string()));
        let empty = lines.next().unwrap();
        assert_eq!(empty, LineType::Empty);
    }

    #[test]
    fn test_first_line_new_format() {
        let input = "The Title\n=========\n\n";
        let mut lines = LineIterator::new(input.as_bytes());
        let title = lines.next().unwrap();
        assert_eq!(title, LineType::Title("The Title".to_string()));
        let empty = lines.next().unwrap();
        assert_eq!(empty, LineType::Empty);
    }
}
