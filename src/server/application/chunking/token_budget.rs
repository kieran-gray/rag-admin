use crate::server::application::ports::Tokenizer;
use crate::server::application::AppError;

pub struct TokenBudget<'a> {
    tokenizer: &'a dyn Tokenizer,
}

impl<'a> TokenBudget<'a> {
    pub fn new(tokenizer: &'a dyn Tokenizer) -> Self {
        Self { tokenizer }
    }

    pub fn count_str(&self, text: &str) -> Result<usize, AppError> {
        self.tokenizer.count(text).map(|count| count as usize)
    }

    pub fn count_chars(&self, chars: &[char]) -> Result<usize, AppError> {
        let text: String = chars.iter().collect();
        self.count_str(&text)
    }

    pub fn max_prefix_chars(&self, chars: &[char], max_tokens: usize) -> Result<usize, AppError> {
        if chars.is_empty() || max_tokens == 0 {
            return Ok(0);
        }
        if self.count_chars(chars)? <= max_tokens {
            return Ok(chars.len());
        }

        let mut low = 0usize;
        let mut high = chars.len();
        while low < high {
            let mid = (low + high + 1).div_ceil(2);
            if self.count_chars(&chars[..mid])? <= max_tokens {
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        Ok(low.max(1))
    }

    pub fn suffix_start_for_overlap(
        &self,
        chars: &[char],
        end: usize,
        overlap_tokens: usize,
    ) -> Result<usize, AppError> {
        if end == 0 || overlap_tokens == 0 {
            return Ok(end);
        }

        let mut low = 0usize;
        let mut high = end;
        let mut best = end;
        while low <= high {
            let mid = (low + high) / 2;
            if self.count_chars(&chars[mid..end])? <= overlap_tokens {
                best = mid;
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            } else {
                low = mid + 1;
            }
        }

        Ok(best)
    }
}
