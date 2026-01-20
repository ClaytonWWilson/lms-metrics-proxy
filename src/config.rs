use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub lm_studio_url: String,
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        // Load .env file if it exists (for development)
        dotenvy::dotenv().ok();

        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid PORT value: {}", e))?;

        let lm_studio_url =
            env::var("LM_STUDIO_URL").unwrap_or_else(|_| "http://localhost:1234".to_string());

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:./lms_metrics_proxy.db".to_string());

        Ok(Config {
            port,
            lm_studio_url,
            database_url,
        })
    }
}
