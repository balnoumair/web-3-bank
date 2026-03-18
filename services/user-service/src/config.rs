use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub grpc_addr: String,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")?,
            grpc_addr: env::var("GRPC_ADDR").unwrap_or_else(|_| "0.0.0.0:50051".to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize config tests that mutate global env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_config_missing_database_url() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("DATABASE_URL");
        assert!(Config::from_env().is_err());
    }

    #[test]
    fn test_config_default_grpc_addr() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var("DATABASE_URL", "postgres://localhost/test");
        env::remove_var("GRPC_ADDR");
        let cfg = Config::from_env().unwrap();
        assert_eq!(cfg.grpc_addr, "0.0.0.0:50051");
        env::remove_var("DATABASE_URL");
    }
}
