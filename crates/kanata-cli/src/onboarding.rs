use std::io::{self, Write as _};

use anyhow::Result;
use kanata_types::KanataConfig;

use crate::config;

/// Run the first-time onboarding wizard (before TUI initialization).
///
/// Guides the user through provider selection, API key entry, and model choice.
/// Returns the configured `KanataConfig` on success.
pub fn run_onboarding() -> Result<KanataConfig> {
    println!();
    println!("Welcome to Kanata — next-generation AI Code Agent!");
    println!("Let's get you set up.\n");

    // Step 1: Choose provider
    let provider = choose_provider()?;

    // Step 2: Enter API key
    let api_key = prompt_api_key(&provider)?;

    // Step 3: Choose default model
    let model = choose_model(&provider)?;

    // Build config
    let mut cfg = KanataConfig { default_model: model, ..KanataConfig::default() };
    cfg.api_keys.insert(provider, api_key);

    // Save config
    config::save_config(&cfg)?;
    println!("\nConfiguration saved. You're all set!\n");

    Ok(cfg)
}

fn choose_provider() -> Result<String> {
    println!("Select your AI provider:");
    println!("  1) Anthropic (Claude)");
    println!("  2) OpenAI");
    println!("  3) DeepSeek");
    print!("\nChoice [1]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();

    let provider = match choice {
        "" | "1" => "anthropic",
        "2" => "openai",
        "3" => "deepseek",
        _ => {
            println!("Invalid choice, defaulting to Anthropic.");
            "anthropic"
        }
    };

    Ok(provider.to_string())
}

fn prompt_api_key(provider: &str) -> Result<String> {
    print!("Enter your {provider} API key: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let key = input.trim().to_string();

    if key.is_empty() {
        println!("Warning: empty API key. You can set it later in ~/.config/kanata/config.yaml");
    }

    Ok(key)
}

fn choose_model(provider: &str) -> Result<String> {
    let (models, default) = match provider {
        "anthropic" => {
            (vec!["claude-sonnet-4", "claude-opus-4", "claude-haiku-3"], "claude-sonnet-4")
        }
        "openai" => (vec!["gpt-4o", "gpt-4-turbo", "o1-mini"], "gpt-4o"),
        "deepseek" => (vec!["deepseek-chat", "deepseek-coder"], "deepseek-chat"),
        _ => (vec!["claude-sonnet-4"], "claude-sonnet-4"),
    };

    println!("\nSelect default model:");
    for (i, model) in models.iter().enumerate() {
        let marker = if *model == default { " (default)" } else { "" };
        println!("  {}) {model}{marker}", i + 1);
    }
    print!("\nChoice [1]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();

    let idx: usize =
        if choice.is_empty() { 0 } else { choice.parse::<usize>().unwrap_or(1).saturating_sub(1) };

    let model = models.get(idx).copied().unwrap_or(default);
    Ok(model.to_string())
}

#[cfg(test)]
mod tests {
    // Onboarding tests are limited since they require stdin interaction.
    // Integration tests can use mock stdin if needed.

    #[test]
    fn test_onboarding_module_exists() {
        // Smoke test — module compiles
    }
}
