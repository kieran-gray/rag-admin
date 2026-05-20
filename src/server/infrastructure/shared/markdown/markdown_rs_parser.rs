use markdown::{to_mdast, ParseOptions};

use crate::server::application::markdown::Document;
use crate::server::application::ports::MarkdownParser;
use crate::server::application::AppError;

use super::mdast_mapper::MdastMapper;

pub struct MarkdownRsParser;

impl MarkdownParser for MarkdownRsParser {
    fn parse(&self, source: &str) -> Result<Document, AppError> {
        let root = to_mdast(source, &ParseOptions::gfm())
            .map_err(|err| AppError::Validation(format!("markdown parse failed: {err}")))?;
        MdastMapper::map_document(source, root)
    }
}
