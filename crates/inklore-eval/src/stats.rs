//! Small aggregation helpers for the report: median, min, max and sums that understand "not reported".

/// Median, minimum and maximum of a set of values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    /// The median (mean of the two middle values for an even count).
    pub median: f64,
    /// The smallest value.
    pub min: f64,
    /// The largest value.
    pub max: f64,
}

/// Median of `values`; `None` when empty. Even counts average the two middle values.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted.get(mid).copied()
    } else {
        let (low, high) = (sorted.get(mid - 1)?, sorted.get(mid)?);
        Some((low + high) / 2.0)
    }
}

/// Summarise the values that are present; `None` entries are skipped, and if none are present the
/// result is `None` (shown as `–` in the report).
pub fn summarize(values: impl IntoIterator<Item = Option<f64>>) -> Option<Summary> {
    let present: Vec<f64> = values.into_iter().flatten().collect();
    let median = median(&present)?;
    let min = present.iter().copied().min_by(f64::total_cmp)?;
    let max = present.iter().copied().max_by(f64::total_cmp)?;
    Some(Summary { median, min, max })
}

/// Sum of the values that are present; `None` when none are present.
pub fn sum_present(values: impl IntoIterator<Item = Option<f64>>) -> Option<f64> {
    values
        .into_iter()
        .flatten()
        .fold(None, |acc, v| Some(acc.unwrap_or(0.0) + v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_odd_and_even_counts() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[7.0]), Some(7.0));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn summarize_skips_none() {
        let s = summarize([Some(10.0), None, Some(30.0), Some(20.0)]).unwrap();
        assert_eq!(
            s,
            Summary {
                median: 20.0,
                min: 10.0,
                max: 30.0
            }
        );
        assert_eq!(summarize([None, None]), None);
        assert_eq!(summarize(std::iter::empty()), None);
    }

    #[test]
    fn sum_present_distinguishes_zero_from_missing() {
        assert_eq!(sum_present([Some(1.0), None, Some(2.5)]), Some(3.5));
        assert_eq!(sum_present([Some(0.0)]), Some(0.0));
        assert_eq!(sum_present([None, None]), None);
    }
}
