use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BridgeBackendConfig {
    // Server
    pub host: String,
    pub port: u32,

    // Postgress
    pub pghost: String,
    pub pgport: u32,
    pub pguser: String,
    pub pgpassword: String,
    pub pgdatabase: String,

    // Redis
    pub redis_url: String,

    // Bridge
    pub user_takes_after: String,
    pub verifier_pks: String,
    pub bridge_amount_btc: u32,
    pub confirmation_threshold: u32,

    // Bitcoin
    pub bitcoin_rest_endpoint: String,
    pub bitcoin_rest_auth_header: String,

    // Citrea
    pub citrea_rest_endpoint: String,
    pub bitcoin_lightclient_contract_address: String,
    pub bridge_contract_address: String,
    pub declare_withdraw_filler_private_key: String,

    // Faucet
    pub faucet_private_key: String,
    pub faucet_amount: String,
    pub faucet_amount_limit: String,
    pub faucet_count_limit: String,

    // Operator & Verifier & Aggregator
    pub operator_urls: String,
    pub verifier_urls: String,
    pub aggregator_url: String,

    // Non bridge backend.
    pub docker_image: Option<String>,
    pub env: Vec<(&'static str, &'static str)>,
    pub idx: usize,
}

impl Default for BridgeBackendConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8080,

            pghost: "localhost".to_string(),
            pgport: 5432,
            pguser: "citrea".to_string(),
            pgpassword: "".to_string(),
            pgdatabase: "citrea_bridge".to_string(),

            redis_url: "redis://localhost:6379".to_string(),

            user_takes_after: "c800".to_string(),
            verifier_pks: "7c4803421956db53eed29ee45bddbe60d16e66560f918a94270ea5272b2b4e90".to_string(),
            bridge_amount_btc: 1,
            confirmation_threshold: 6,

            bitcoin_rest_endpoint: "".to_string(),
            bitcoin_rest_auth_header: "".to_string(),

            citrea_rest_endpoint: "".to_string(),
            bitcoin_lightclient_contract_address: "0x3100000000000000000000000000000000000001".to_string(),
            bridge_contract_address: "".to_string(),
            declare_withdraw_filler_private_key: "".to_string(),

            faucet_private_key: "".to_string(),
            faucet_amount: "".to_string(),
            faucet_amount_limit: "".to_string(),
            faucet_count_limit: "".to_string(),

            operator_urls: "http://localhost:17007,http://localhost:17008,http://localhost:17009".to_string(),
            verifier_urls: "http://localhost:17000,http://localhost:17001,http://localhost:17002,http://localhost:17003,http://localhost:17004,http://localhost:17005,http://localhost:17006".to_string(),
            aggregator_url: "http://localhost:17010".to_string(),

            docker_image: None,
            env: Vec::new(),
            idx: 0,
        }
    }
}

impl BridgeBackendConfig {
    /// Sets current configuration to environment variables and returns it.
    /// Bridge backend checks environment variables for it's parameters.
    pub fn get_env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        env.insert("NODE_ENV".to_string(), "development".to_string());
        env.insert("NODE_DEBUG".to_string(), "jsonrpc,db".to_string());

        env.insert("HOST".to_string(), self.host.clone());
        env.insert("PORT".to_string(), self.port.to_string());
        env.insert(
            "CORS_ORIGIN_PATTERN".to_string(),
            "^http://localhost ".to_string(),
        );

        env.insert("PGHOST".to_string(), self.pghost.clone());
        env.insert("PGPORT".to_string(), self.pgport.to_string());
        env.insert("PGUSER".to_string(), self.pguser.clone());
        env.insert("PGPASSWORD".to_string(), self.pgpassword.clone());
        env.insert("PGDATABASE".to_string(), self.pgdatabase.clone());
        env.insert("PGSSLMODE".to_string(), "prefer".to_string());

        env.insert("REDIS_URL".to_string(), self.redis_url.clone());

        env.insert("NETWORK".to_string(), "regtest".to_string());
        env.insert(
            "USER_TAKES_AFTER".to_string(),
            self.user_takes_after.clone(),
        );
        env.insert("VERIFIER_PKS".to_string(), self.verifier_pks.clone());
        env.insert(
            "BRIDGE_AMOUNT_BTC".to_string(),
            self.bridge_amount_btc.to_string(),
        );
        env.insert(
            "CONFIRMATION_THRESHOLD".to_string(),
            self.confirmation_threshold.to_string(),
        );

        env.insert(
            "BITCOIN_REST_ENDPOINT".to_string(),
            self.bitcoin_rest_endpoint.clone(),
        );
        env.insert(
            "BITCOIN_REST_AUTH_HEADER".to_string(),
            self.bitcoin_rest_auth_header.clone(),
        );

        env.insert(
            "CITREA_REST_ENDPOINT".to_string(),
            self.citrea_rest_endpoint.clone(),
        );
        env.insert(
            "BITCOIN_LIGHTCLIENT_CONTRACT_ADDRESS".to_string(),
            self.bitcoin_lightclient_contract_address.clone(),
        );
        env.insert(
            "BRIDGE_CONTRACT_ADDRESS".to_string(),
            self.bridge_contract_address.clone(),
        );
        env.insert(
            "DECLARE_WITHDRAW_FILLER_PRIVATE_KEY".to_string(),
            self.declare_withdraw_filler_private_key.clone(),
        );

        env.insert(
            "FAUCET_PRIVATE_KEY".to_string(),
            self.faucet_private_key.clone(),
        );
        env.insert("FAUCET_AMOUNT".to_string(), self.faucet_amount.clone());
        env.insert(
            "FAUCET_AMOUNT_LIMIT".to_string(),
            self.faucet_amount_limit.clone(),
        );
        env.insert(
            "FAUCET_COUNT_LIMIT".to_string(),
            self.faucet_count_limit.clone(),
        );

        env.insert("OPERATOR_URLS".to_string(), self.operator_urls.clone());
        env.insert("VERIFIER_URLS".to_string(), self.verifier_urls.clone());
        env.insert("AGGREGATOR_URL".to_string(), self.aggregator_url.clone());

        env.insert("TELEGRAM_BOT_API_KEY".to_string(), "".to_string());
        env.insert("TELEGRAM_CHAT_ID".to_string(), "".to_string());

        env.insert(
            "CLOUDFLARE_SITE_KEY".to_string(),
            "1x00000000000000000000AA".to_string(),
        );
        env.insert(
            "CLOUDFLARE_SECRET_KEY".to_string(),
            "1x0000000000000000000000000000000AA".to_string(),
        );

        env
    }

    #[allow(unused)]
    pub fn convert_hashmap_to_vec(input: &HashMap<String, String>) -> Vec<(&str, &str)> {
        let mut result: Vec<(&str, &str)> = Vec::new();

        for val in input {
            result.push((val.0.as_str(), val.1.as_str()));
        }

        result
    }
}
