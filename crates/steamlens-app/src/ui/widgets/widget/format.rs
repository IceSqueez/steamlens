pub fn format_thousands(n: u32) -> String {
    format_thousands_u64(n as u64)
}

fn format_thousands_u64(n: u64) -> String {
    let digits = n.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

pub fn format_remaining(remaining: u64) -> String {
    format!("{} achievements remaining", format_thousands_u64(remaining))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_thousands_basic() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(42), "42");
        assert_eq!(format_thousands(1_234), "1,234");
        assert_eq!(format_thousands(1_234_567), "1,234,567");
    }
}
