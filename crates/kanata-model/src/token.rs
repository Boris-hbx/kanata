//! Token pricing and cost calculation.

/// Returns `(input_price_per_1m, output_price_per_1m)` in USD for a given model.
fn pricing(model: &str) -> (f64, f64) {
    match model {
        m if m.contains("claude-opus-4") => (15.0, 75.0),
        m if m.contains("claude-sonnet-4") => (3.0, 15.0),
        m if m.contains("claude-haiku-3") || m.contains("claude-3-5-haiku") => (0.80, 4.0),
        m if m.contains("gpt-4o") => (2.50, 10.0),
        m if m.contains("deepseek") => (0.27, 1.10),
        m if m.contains("gemini-2.0-flash") => (0.10, 0.40),
        m if m.contains("gemini-2.5-pro") => (1.25, 10.0),
        m if m.contains("gemini-2.5-flash") => (0.15, 0.60),
        m if m.contains("gemini") => (0.15, 0.60), // default gemini
        m if m.contains("grok-3") => (3.0, 15.0),
        m if m.contains("grok-2") => (2.0, 10.0),
        m if m.contains("grok") => (5.0, 15.0), // default grok
        _ => (3.0, 15.0), // default to sonnet pricing
    }
}

/// Calculate cost in USD given model name and token counts.
pub fn cost_usd(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    let (inp, out) = pricing(model);
    (f64::from(input_tokens) * inp + f64::from(output_tokens) * out) / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sonnet_pricing() {
        let cost = cost_usd("claude-sonnet-4-20250514", 1_000_000, 1_000_000);
        assert!((cost - 18.0).abs() < 0.001);
    }

    #[test]
    fn test_deepseek_pricing() {
        let cost = cost_usd("deepseek-chat", 1_000_000, 1_000_000);
        assert!((cost - 1.37).abs() < 0.001);
    }
}
