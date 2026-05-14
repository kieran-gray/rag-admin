pub fn parse_u32_values(
    input: &str,
    min: u32,
    max: u32,
    default_step: u32,
) -> Result<Vec<u32>, String> {
    let mut values = Vec::new();
    for token in input.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if let Some((range, step)) = token.split_once(':') {
            let step = step
                .parse::<u32>()
                .map_err(|_| format!("invalid step in '{token}'"))?;
            push_range_values(&mut values, range, min, max, step)?;
        } else if token.contains('-') {
            push_range_values(&mut values, token, min, max, default_step)?;
        } else {
            values.push(parse_bounded(token, min, max)?);
        }
    }
    values.sort_unstable();
    values.dedup();
    if values.is_empty() {
        Err("no values supplied".into())
    } else {
        Ok(values)
    }
}

fn push_range_values(
    values: &mut Vec<u32>,
    range: &str,
    min: u32,
    max: u32,
    step: u32,
) -> Result<(), String> {
    if step == 0 {
        return Err("range step must be greater than 0".into());
    }
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| format!("invalid range '{range}'"))?;
    let start = parse_bounded(start.trim(), min, max)?;
    let end = parse_bounded(end.trim(), min, max)?;
    if start > end {
        return Err(format!("range start is greater than end in '{range}'"));
    }
    let mut value = start;
    while value <= end {
        values.push(value);
        match value.checked_add(step) {
            Some(next) => value = next,
            None => break,
        }
    }
    Ok(())
}

fn parse_bounded(token: &str, min: u32, max: u32) -> Result<u32, String> {
    let value = token
        .parse::<u32>()
        .map_err(|_| format!("invalid number '{token}'"))?;
    if value < min || value > max {
        Err(format!("{value} is outside {min}..={max}"))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_values_round_trip() {
        assert_eq!(
            parse_u32_values("2,3,5,8", 1, 100, 1).unwrap(),
            vec![2, 3, 5, 8]
        );
    }

    #[test]
    fn range_with_explicit_step() {
        assert_eq!(
            parse_u32_values("400-700:100", 0, 1000, 100).unwrap(),
            vec![400, 500, 600, 700]
        );
    }

    #[test]
    fn mixed_tokens_are_sorted_and_deduped() {
        assert_eq!(
            parse_u32_values("3, 1, 3, 2-5", 0, 100, 1).unwrap(),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn out_of_bounds_rejected() {
        assert!(parse_u32_values("1500", 0, 1000, 1).is_err());
    }

    #[test]
    fn invalid_step_zero_rejected() {
        assert!(parse_u32_values("1-10:0", 0, 100, 1).is_err());
    }

    #[test]
    fn empty_input_rejected() {
        assert!(parse_u32_values("", 0, 100, 1).is_err());
    }
}
