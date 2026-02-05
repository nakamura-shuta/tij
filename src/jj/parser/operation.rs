//! Operation log parser (jj op log)

use super::super::JjError;
use super::super::template::FIELD_SEPARATOR;
use crate::model::Operation;

use super::Parser;

impl Parser {
    /// Parse `jj op log` output into a list of Operations
    ///
    /// Expected format (tab-separated):
    /// `<id>\t<user>\t<timestamp>\t<description>`
    pub fn parse_op_log(output: &str) -> Result<Vec<Operation>, JjError> {
        let mut operations = Vec::new();

        for (index, line) in output.lines().enumerate() {
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split(FIELD_SEPARATOR).collect();
            if parts.len() < 4 {
                continue; // Skip malformed lines
            }

            operations.push(Operation {
                id: parts[0].to_string(),
                user: parts[1].to_string(),
                timestamp: parts[2].to_string(),
                description: parts[3].to_string(),
                is_current: index == 0,
            });
        }

        Ok(operations)
    }
}
