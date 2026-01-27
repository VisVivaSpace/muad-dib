//! Text PCK (Planetary Constants Kernel) parser with full round-trip support.
//!
//! Text PCK files are ASCII files containing kernel pool variables in the form:
//! ```text
//! \begindata
//! BODY399_POLE_RA  = (    0.      -0.641         0. )
//! BODY399_RADII    = (  6378.14   6378.14   6356.75 )
//! \begintext
//! ```
//!
//! Unlike SPK/CK/BPCK, text PCK files are NOT DAF binary files.
//!
//! This module provides `PCKSource` which preserves the alternating text/data
//! block structure of the file for perfect round-trip reconstruction.

use crate::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

/// A value in a text kernel variable.
///
/// Text kernels (PCK, LSK, SCLK, FK) can contain different value types:
/// - Numeric: floating-point values like `123.456` or `1.234D-5`
/// - Epoch: date strings prefixed with `@`, like `@1972-JAN-1`
/// - Text: quoted strings like `'FRAME_NAME'`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KernelValue {
    /// Numeric value (floating-point)
    Numeric(f64),
    /// Epoch string (stores the full string including `@` prefix)
    Epoch(String),
    /// Text string (quotes stripped)
    Text(String),
}

impl KernelValue {
    /// Returns the numeric value if this is a Numeric variant, None otherwise.
    pub fn as_numeric(&self) -> Option<f64> {
        match self {
            KernelValue::Numeric(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns the string value if this is an Epoch or Text variant, None for Numeric.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            KernelValue::Epoch(s) | KernelValue::Text(s) => Some(s),
            KernelValue::Numeric(_) => None,
        }
    }

    /// Returns true if this is a Numeric variant.
    pub fn is_numeric(&self) -> bool {
        matches!(self, KernelValue::Numeric(_))
    }
}

/// A single kernel pool variable from a text kernel file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PCKVariable {
    /// Variable name (e.g., "BODY399_RADII", "DELTET/DELTA_AT")
    pub name: String,
    /// Variable values (can be mixed types for LSK/SCLK/FK files)
    pub values: Vec<KernelValue>,
}

impl PCKVariable {
    /// Returns all values as f64 if all values are numeric.
    /// Returns None if any value is non-numeric.
    pub fn values_as_f64(&self) -> Option<Vec<f64>> {
        self.values.iter().map(|v| v.as_numeric()).collect()
    }

    /// Returns only the numeric values, filtering out epochs and text.
    pub fn numeric_values(&self) -> Vec<f64> {
        self.values.iter().filter_map(|v| v.as_numeric()).collect()
    }

    /// Returns only the text/epoch values as string references.
    pub fn text_values(&self) -> Vec<&str> {
        self.values.iter().filter_map(|v| v.as_string()).collect()
    }

    /// Returns true if all values are numeric.
    pub fn is_all_numeric(&self) -> bool {
        self.values.iter().all(|v| v.is_numeric())
    }
}

/// A block in a PCK file - either text or data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PCKBlock {
    /// Text block (comments between \begintext and \begindata)
    Text(String),
    /// Data block (variables between \begindata and \begintext)
    Data(Vec<PCKVariable>),
}

/// Complete PCK source with full structural preservation.
///
/// Stores alternating text/data blocks in file order for round-trip support.
/// Unlike DAFSource, this is specifically for text PCK files which are not DAF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PCKSource {
    /// Source filename
    pub filename: String,
    /// Alternating text/data blocks in file order
    pub blocks: Vec<PCKBlock>,
}

impl PCKSource {
    /// Parse a text PCK file from a file handle.
    pub fn from_file(file: File, filename: &str) -> Result<Self> {
        let reader = std::io::BufReader::new(file);
        Self::parse(reader, filename)
    }

    /// Parse a text PCK file from a path.
    pub fn from_path(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let filename = path.display().to_string();
        Self::from_file(file, &filename)
    }

    /// Parse text PCK content from a buffered reader.
    ///
    /// Parser logic:
    /// 1. Start with text accumulator
    /// 2. Accumulate lines until \begindata
    /// 3. On \begindata: push Text(accumulated) if non-empty, start data accumulator
    /// 4. Parse variables until \begintext
    /// 5. On \begintext: push Data(variables), start text accumulator
    /// 6. Repeat until EOF
    /// 7. Push final text block if non-empty
    fn parse<R: BufRead>(reader: R, filename: &str) -> Result<Self> {
        let mut blocks = Vec::new();
        let mut text_accumulator = String::new();
        let mut in_data_block = false;
        let mut current_variables: Vec<PCKVariable> = Vec::new();
        let mut current_assignment: Option<String> = None;

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            let lower = trimmed.to_lowercase();

            // Check for block delimiters
            if lower.contains("\\begindata") {
                // We're entering a data block
                // Push any accumulated text as a Text block
                if !text_accumulator.is_empty() || blocks.is_empty() {
                    blocks.push(PCKBlock::Text(text_accumulator.clone()));
                    text_accumulator.clear();
                }
                in_data_block = true;
                current_variables.clear();
                current_assignment = None;
                continue;
            }

            if lower.contains("\\begintext") {
                // We're leaving a data block
                // Process any pending assignment
                if let Some(assignment) = current_assignment.take() {
                    if let Some(var) = Self::parse_assignment(&assignment)? {
                        current_variables.push(var);
                    }
                }
                // Push the data block
                if !current_variables.is_empty() {
                    blocks.push(PCKBlock::Data(current_variables.clone()));
                    current_variables.clear();
                }
                in_data_block = false;
                continue;
            }

            if in_data_block {
                // Skip empty lines in data block
                if trimmed.is_empty() {
                    continue;
                }

                // Check if this line starts a new assignment (contains '=')
                if trimmed.contains('=') && !trimmed.starts_with('(') {
                    // Process previous assignment if any
                    if let Some(assignment) = current_assignment.take() {
                        if let Some(var) = Self::parse_assignment(&assignment)? {
                            current_variables.push(var);
                        }
                    }
                    current_assignment = Some(trimmed.to_string());
                } else if let Some(ref mut assignment) = current_assignment {
                    // Continuation of previous assignment
                    assignment.push(' ');
                    assignment.push_str(trimmed);
                }
            } else {
                // In text block - accumulate lines preserving newlines
                if !text_accumulator.is_empty() {
                    text_accumulator.push('\n');
                }
                text_accumulator.push_str(&line);
            }
        }

        // Process any final pending assignment
        if let Some(assignment) = current_assignment {
            if let Some(var) = Self::parse_assignment(&assignment)? {
                current_variables.push(var);
            }
        }

        // Push final data block if we were in one at EOF
        if in_data_block && !current_variables.is_empty() {
            blocks.push(PCKBlock::Data(current_variables));
        }

        // Push final text block if non-empty
        if !text_accumulator.is_empty() {
            blocks.push(PCKBlock::Text(text_accumulator));
        }

        Ok(PCKSource {
            filename: filename.to_string(),
            blocks,
        })
    }

    /// Parse a single assignment line like "NAME = ( values )" or "NAME = value"
    fn parse_assignment(line: &str) -> Result<Option<PCKVariable>> {
        // Split on '='
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Ok(None);
        }

        let name = parts[0].trim().to_uppercase();
        let value_part = parts[1].trim();

        // Parse values - can be "( val1 val2 ... )" or just "value"
        let values = Self::parse_values(value_part)?;

        if values.is_empty() {
            return Ok(None);
        }

        Ok(Some(PCKVariable { name, values }))
    }

    /// Parse values from a string like "( val1 val2 ... )" or "value"
    ///
    /// Handles:
    /// - Numeric values: `123.456`, `1.234D-5` (FORTRAN notation)
    /// - Epoch strings: `@1972-JAN-1`
    /// - Quoted text: `'FRAME_NAME'`
    fn parse_values(s: &str) -> Result<Vec<KernelValue>> {
        let mut values = Vec::new();
        let trimmed = s.trim();

        // Strip parentheses if present
        let content = if let Some(stripped) = trimmed.strip_prefix('(') {
            if let Some(inner) = stripped.strip_suffix(')') {
                inner
            } else if let Some(end) = stripped.rfind(')') {
                // Multi-line case: closing paren somewhere in the middle
                &stripped[..end]
            } else {
                // No closing paren yet
                stripped
            }
        } else {
            trimmed
        };

        // Parse tokens - need special handling for quoted strings
        let mut chars = content.chars().peekable();
        let mut current_token = String::new();

        while let Some(c) = chars.next() {
            match c {
                // Start of quoted string
                '\'' => {
                    // Collect until closing quote
                    let mut quoted = String::new();
                    while let Some(&next) = chars.peek() {
                        chars.next();
                        if next == '\'' {
                            break;
                        }
                        quoted.push(next);
                    }
                    if !quoted.is_empty() {
                        values.push(KernelValue::Text(quoted));
                    }
                }
                // Whitespace or comma - end of token
                c if c.is_whitespace() || c == ',' => {
                    if !current_token.is_empty() {
                        if let Some(val) = Self::parse_single_value(&current_token) {
                            values.push(val);
                        }
                        current_token.clear();
                    }
                }
                // Regular character
                _ => {
                    current_token.push(c);
                }
            }
        }

        // Handle final token
        if !current_token.is_empty() {
            if let Some(val) = Self::parse_single_value(&current_token) {
                values.push(val);
            }
        }

        Ok(values)
    }

    /// Parse a single non-quoted value token.
    fn parse_single_value(token: &str) -> Option<KernelValue> {
        let token = token.trim();
        if token.is_empty() {
            return None;
        }

        // Check for epoch string (@DATE format)
        if token.starts_with('@') {
            return Some(KernelValue::Epoch(token.to_string()));
        }

        // Try to parse as numeric
        // Convert FORTRAN-style scientific notation (D -> E)
        let normalized = token
            .replace('D', "E")
            .replace('d', "e")
            .replace("+E", "E")
            .replace("+e", "e");

        // Handle leading + sign
        let normalized = normalized.trim_start_matches('+');

        match normalized.parse::<f64>() {
            Ok(v) => Some(KernelValue::Numeric(v)),
            Err(_) => None, // Skip unparseable tokens
        }
    }

    /// Get all variables from all data blocks (flattened).
    pub fn variables(&self) -> Vec<&PCKVariable> {
        self.blocks
            .iter()
            .filter_map(|block| {
                if let PCKBlock::Data(vars) = block {
                    Some(vars.iter())
                } else {
                    None
                }
            })
            .flatten()
            .collect()
    }

    /// Get all variables for a specific body ID.
    pub fn variables_for_body(&self, body_id: i32) -> Vec<&PCKVariable> {
        let prefix = format!("BODY{}_", body_id);
        self.variables()
            .into_iter()
            .filter(|v| v.name.starts_with(&prefix))
            .collect()
    }

    /// Get all unique body IDs from the variables.
    /// Extracts the numeric part from `BODY<id>_<name>` patterns.
    pub fn body_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .variables()
            .iter()
            .filter_map(|v| Self::extract_body_id(&v.name))
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Extract body ID from a variable name like "BODY399_RADII" -> 399
    pub fn extract_body_id(name: &str) -> Option<i32> {
        if !name.starts_with("BODY") {
            return None;
        }

        let rest = &name[4..]; // Skip "BODY"
        let id_part: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();

        id_part.parse::<i32>().ok()
    }

    /// Group variables by body ID.
    pub fn variables_by_body(&self) -> HashMap<i32, Vec<&PCKVariable>> {
        let mut map: HashMap<i32, Vec<&PCKVariable>> = HashMap::new();

        for var in self.variables() {
            if let Some(body_id) = Self::extract_body_id(&var.name) {
                map.entry(body_id).or_default().push(var);
            }
        }

        map
    }
}

// Keep TextPCKFile as an alias for backward compatibility during transition
pub type TextPCKFile = PCKSource;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_simple_variable() {
        let content = r#"
\begindata
BODY10_POLE_RA = ( 286.13 0. 0. )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        // Should have 2 blocks: initial text + data
        assert_eq!(pck.blocks.len(), 2);

        let vars = pck.variables();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "BODY10_POLE_RA");
        assert_eq!(vars[0].values.len(), 3);
        let floats = vars[0].values_as_f64().unwrap();
        assert!((floats[0] - 286.13).abs() < 1e-10);
        assert!((floats[1] - 0.0).abs() < 1e-10);
        assert!((floats[2] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_array_with_commas() {
        let content = r#"
\begindata
BODY199_POLE_RA = ( 281.01, -0.033, 0. )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        let vars = pck.variables();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].values.len(), 3);
        let floats = vars[0].values_as_f64().unwrap();
        assert!((floats[1] - (-0.033)).abs() < 1e-10);
    }

    #[test]
    fn test_parse_scientific_notation() {
        let content = r#"
\begindata
BODY3_NUT_PREC_ANGLES = ( 125.045D0 -0.0529921D0 1.4D-12 )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        let vars = pck.variables();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].values.len(), 3);
        let floats = vars[0].values_as_f64().unwrap();
        assert!((floats[0] - 125.045).abs() < 1e-10);
        assert!((floats[1] - (-0.0529921)).abs() < 1e-10);
        assert!((floats[2] - 1.4e-12).abs() < 1e-20);
    }

    #[test]
    fn test_parse_multiline_values() {
        let content = r#"
\begindata
BODY3_NUT_PREC_ANGLES = ( 125.045 -1935.5364525000
                          250.089 -3871.0729050000 )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        let vars = pck.variables();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].values.len(), 4);
    }

    #[test]
    fn test_parse_scalar_value() {
        let content = r#"
\begindata
BODY399_MAG_NORTH_POLE_LON = ( -69.761 )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        let vars = pck.variables();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].values.len(), 1);
        let floats = vars[0].values_as_f64().unwrap();
        assert!((floats[0] - (-69.761)).abs() < 1e-10);
    }

    #[test]
    fn test_parse_multiple_variables() {
        let content = r#"
\begindata
BODY10_POLE_RA = ( 286.13 0. 0. )
BODY10_POLE_DEC = ( 63.87 0. 0. )
BODY10_PM = ( 84.10 14.18440 0. )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        let vars = pck.variables();
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_parse_multiple_data_blocks() {
        let content = r#"Some comment text
\begindata
BODY10_POLE_RA = ( 286.13 0. 0. )
\begintext
More comments
\begindata
BODY199_POLE_RA = ( 281.01 -0.033 0. )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        // Should have: Text, Data, Text, Data
        assert_eq!(pck.blocks.len(), 4);
        assert!(matches!(pck.blocks[0], PCKBlock::Text(_)));
        assert!(matches!(pck.blocks[1], PCKBlock::Data(_)));
        assert!(matches!(pck.blocks[2], PCKBlock::Text(_)));
        assert!(matches!(pck.blocks[3], PCKBlock::Data(_)));

        let vars = pck.variables();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "BODY10_POLE_RA");
        assert_eq!(vars[1].name, "BODY199_POLE_RA");
    }

    #[test]
    fn test_extract_body_id() {
        assert_eq!(PCKSource::extract_body_id("BODY399_RADII"), Some(399));
        assert_eq!(PCKSource::extract_body_id("BODY10_POLE_RA"), Some(10));
        assert_eq!(PCKSource::extract_body_id("BODY3_NUT_PREC_ANGLES"), Some(3));
        assert_eq!(PCKSource::extract_body_id("SOME_OTHER_VAR"), None);
    }

    #[test]
    fn test_body_ids() {
        let content = r#"
\begindata
BODY10_POLE_RA = ( 286.13 0. 0. )
BODY10_POLE_DEC = ( 63.87 0. 0. )
BODY399_RADII = ( 6378.14 6378.14 6356.75 )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        let ids = pck.body_ids();
        assert_eq!(ids, vec![10, 399]);
    }

    #[test]
    fn test_variables_by_body() {
        let content = r#"
\begindata
BODY10_POLE_RA = ( 286.13 0. 0. )
BODY10_POLE_DEC = ( 63.87 0. 0. )
BODY399_RADII = ( 6378.14 6378.14 6356.75 )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        let by_body = pck.variables_by_body();
        assert_eq!(by_body.get(&10).map(|v| v.len()), Some(2));
        assert_eq!(by_body.get(&399).map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_case_insensitive_delimiters() {
        let content = r#"
\BEGINDATA
BODY10_POLE_RA = ( 286.13 0. 0. )
\BEGINTEXT
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        let vars = pck.variables();
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_text_block_preservation() {
        let content = r#"KPL/PCK

Planetary Constants Kernel
Contains orientation data for planets

\begindata
BODY399_RADII = ( 6378.14 6378.14 6356.75 )
\begintext

This is a comment between blocks.
With multiple lines.

\begindata
BODY10_PM = ( 84.10 14.18440 0. )
\begintext
"#;
        let cursor = Cursor::new(content);
        let pck = PCKSource::parse(cursor, "test.tpc").unwrap();

        // Should have: Text, Data, Text, Data
        assert_eq!(pck.blocks.len(), 4);

        // Check first text block
        if let PCKBlock::Text(text) = &pck.blocks[0] {
            assert!(text.contains("KPL/PCK"));
            assert!(text.contains("Planetary Constants Kernel"));
        } else {
            panic!("Expected Text block");
        }

        // Check middle text block
        if let PCKBlock::Text(text) = &pck.blocks[2] {
            assert!(text.contains("This is a comment between blocks"));
            assert!(text.contains("With multiple lines"));
        } else {
            panic!("Expected Text block");
        }
    }

    #[test]
    fn test_parse_lsk_with_epochs() {
        // LSK files have alternating numeric and epoch values
        let content = r#"KPL/LSK
\begindata
DELTET/DELTA_T_A = 32.184
DELTET/DELTA_AT = ( 10,   @1972-JAN-1
                    11,   @1972-JUL-1
                    12,   @1973-JAN-1 )
\begintext
"#;
        let cursor = Cursor::new(content);
        let lsk = PCKSource::parse(cursor, "test.tls").unwrap();

        let vars = lsk.variables();
        assert_eq!(vars.len(), 2);

        // DELTA_T_A should be all numeric
        let delta_ta = &vars[0];
        assert_eq!(delta_ta.name, "DELTET/DELTA_T_A");
        assert!(delta_ta.is_all_numeric());
        assert_eq!(delta_ta.values_as_f64(), Some(vec![32.184]));

        // DELTA_AT should have mixed values
        let delta_at = &vars[1];
        assert_eq!(delta_at.name, "DELTET/DELTA_AT");
        assert!(!delta_at.is_all_numeric());
        assert_eq!(delta_at.values.len(), 6); // 3 numeric + 3 epoch

        // Check numeric values
        let numerics = delta_at.numeric_values();
        assert_eq!(numerics.len(), 3);
        assert!((numerics[0] - 10.0).abs() < 1e-10);
        assert!((numerics[1] - 11.0).abs() < 1e-10);
        assert!((numerics[2] - 12.0).abs() < 1e-10);

        // Check epoch values
        let epochs = delta_at.text_values();
        assert_eq!(epochs.len(), 3);
        assert_eq!(epochs[0], "@1972-JAN-1");
        assert_eq!(epochs[1], "@1972-JUL-1");
        assert_eq!(epochs[2], "@1973-JAN-1");
    }

    #[test]
    fn test_parse_quoted_strings() {
        // FK files have quoted string values
        let content = r#"KPL/FK
\begindata
FRAME_MY_FRAME = 'CUSTOM_FRAME'
TEST_VALUES = ( 1.0, 'STRING_VAL', 2.0 )
\begintext
"#;
        let cursor = Cursor::new(content);
        let fk = PCKSource::parse(cursor, "test.tf").unwrap();

        let vars = fk.variables();
        assert_eq!(vars.len(), 2);

        // Check quoted string variable
        let frame_var = &vars[0];
        assert_eq!(frame_var.name, "FRAME_MY_FRAME");
        assert_eq!(frame_var.values.len(), 1);
        assert_eq!(frame_var.text_values(), vec!["CUSTOM_FRAME"]);

        // Check mixed numeric and string
        let test_var = &vars[1];
        assert_eq!(test_var.name, "TEST_VALUES");
        assert_eq!(test_var.values.len(), 3);
        assert!(!test_var.is_all_numeric());
        assert_eq!(test_var.numeric_values(), vec![1.0, 2.0]);
        assert_eq!(test_var.text_values(), vec!["STRING_VAL"]);
    }

    #[test]
    fn test_kernel_value_methods() {
        let num = KernelValue::Numeric(42.0);
        let epoch = KernelValue::Epoch("@2000-JAN-1".to_string());
        let text = KernelValue::Text("FRAME".to_string());

        assert!(num.is_numeric());
        assert!(!epoch.is_numeric());
        assert!(!text.is_numeric());

        assert_eq!(num.as_numeric(), Some(42.0));
        assert_eq!(epoch.as_numeric(), None);

        assert_eq!(num.as_string(), None);
        assert_eq!(epoch.as_string(), Some("@2000-JAN-1"));
        assert_eq!(text.as_string(), Some("FRAME"));
    }
}
