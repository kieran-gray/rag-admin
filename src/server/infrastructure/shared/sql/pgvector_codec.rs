use crate::server::application::AppError;

pub fn format_vector_literal(values: &[f32]) -> String {
    let mut out = String::with_capacity(values.len() * 8 + 2);
    out.push('[');
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let v = if v.is_finite() { *v } else { 0.0 };
        out.push_str(&v.to_string());
    }
    out.push(']');
    out
}

pub fn parse_vector_literal(s: &str) -> Result<Vec<f32>, AppError> {
    let trimmed = s.trim();
    let body = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| AppError::Internal(format!("malformed vector literal: {s:?}")))?;
    if body.is_empty() {
        return Ok(Vec::new());
    }
    body.split(',')
        .map(|part| {
            part.trim().parse::<f32>().map_err(|e| {
                AppError::Internal(format!("malformed vector literal element {part:?}: {e}"))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_vector_literal() {
        assert_eq!(format_vector_literal(&[]), "[]");
        assert_eq!(format_vector_literal(&[1.0, 2.5, -0.5]), "[1,2.5,-0.5]");
    }

    #[test]
    fn clamps_non_finite_values() {
        let s = format_vector_literal(&[1.0, f32::NAN, f32::INFINITY]);
        assert_eq!(s, "[1,0,0]");
    }

    #[test]
    fn parses_vector_literal() {
        assert_eq!(parse_vector_literal("[]").unwrap(), Vec::<f32>::new());
        assert_eq!(
            parse_vector_literal("[1,2.5,-0.5]").unwrap(),
            vec![1.0, 2.5, -0.5]
        );
    }

    #[test]
    fn parses_with_whitespace() {
        assert_eq!(
            parse_vector_literal(" [ 1.0 , 2.0 , 3.0 ] ").unwrap(),
            vec![1.0, 2.0, 3.0]
        );
    }

    #[test]
    fn rejects_missing_brackets() {
        assert!(parse_vector_literal("1,2,3").is_err());
    }

    #[test]
    fn roundtrips() {
        let v = vec![0.123_f32, -4.5, 6.789, 0.0];
        let parsed = parse_vector_literal(&format_vector_literal(&v)).unwrap();
        assert_eq!(parsed, v);
    }
}
