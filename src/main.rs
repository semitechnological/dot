use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use dot::auth::ProviderCredential;
use tracing_subscriber::EnvFilter;
#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install().map_err(|e| anyhow!("{e}"))?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();
    let cli = dot::cli::Cli::parse();
    match cli.command {
        Some(dot::cli::Commands::Login) => {
            dot::config::Config::ensure_dirs()?;
            dot::auth::login_flow().await?;
        }
        Some(dot::cli::Commands::Config) => {
            let config_path = dot::config::Config::config_path();
            let data_dir = dot::config::Config::data_dir();
            let creds_path = dot::config::Config::config_dir().join("credentials.json");
            println!("config   {}", config_path.display());
            println!("data     {}", data_dir.display());
            println!("creds    {}", creds_path.display());
            if config_path.exists() {
                let cfg = dot::config::Config::load()?;
                println!("provider {}", cfg.default_provider);
                println!("model    {}", cfg.default_model);
            }
        }
        None => {
            dot::config::Config::ensure_dirs()?;
            let config = dot::config::Config::load()?;
            let creds = dot::auth::Credentials::load()?;
            let db = dot::db::Db::open().context("opening database")?;
            let provider = build_provider(&config, &creds)?;
            dot::tui::run(config, provider, db).await?;
        }
    }
    Ok(())
}
fn build_provider(
    config: &dot::config::Config,
    creds: &dot::auth::Credentials,
) -> Result<Box<dyn dot::provider::Provider>> {
    let model = config.default_model.clone();

    match config.default_provider.as_str() {
        "anthropic" => match creds.get("anthropic") {
            Some(ProviderCredential::ApiKey { key }) => Ok(Box::new(
                dot::provider::anthropic::AnthropicProvider::new_with_api_key(key.clone(), model),
            )),
            Some(ProviderCredential::OAuth {
                access_token,
                refresh_token,
                expires_at,
                ..
            }) => Ok(Box::new(
                dot::provider::anthropic::AnthropicProvider::new_with_oauth(
                    access_token.clone(),
                    refresh_token.clone().unwrap_or_default(),
                    expires_at.unwrap_or(0),
                    model,
                ),
            )),
            None => bail!("No Anthropic credentials — run `dot login` first."),
        },
        "openai" => match creds.get("openai") {
            Some(cred) => {
                let api_key = cred
                    .api_key()
                    .ok_or_else(|| anyhow!("Invalid OpenAI credentials"))?
                    .to_string();
                let oai_config =
                    async_openai::config::OpenAIConfig::new().with_api_key(api_key);
                Ok(Box::new(
                    dot::provider::openai::OpenAIProvider::new_with_config(oai_config, model),
                ))
            }
            None => bail!("No OpenAI credentials — run `dot login` first."),
        },

        other => bail!("Unknown provider '{other}'. Supported: anthropic, openai"),
    }
}
