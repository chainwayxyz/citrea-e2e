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
        }
    }
}

impl BridgeBackendConfig {
    /// Sets current configuration to environment variables and returns it.
    /// Bridge backend checks environment variables for it's parameters.
    fn set_env(&self) -> String {
        let mut env = String::new();

        env += &format!("NODE_ENV=development ");
        env += &format!("NODE_DEBUG=jsonrpc,db ");

        env += &format!("HOST={} ", self.host);
        env += &format!("PORT={} ", self.port);
        env += &format!("CORS_ORIGIN_PATTERN=^http://localhost ");

        env += &format!("PGHOST={} ", self.pghost);
        env += &format!("PGPORT={} ", self.pgport);
        env += &format!("PGUSER={} ", self.pguser);
        env += &format!("PGPASSWORD={} ", self.pgpassword);
        env += &format!("PGDATABASE={} ", self.pgdatabase);
        env += &format!("PGSSLMODE=prefer ");

        env += &format!("REDIS_URL={} ", self.redis_url);

        env += &format!("NETWORK=regtest ");
        env += &format!("USER_TAKES_AFTER={} ", self.user_takes_after);
        env += &format!("VERIFIER_PKS={} ", self.verifier_pks);
        env += &format!("BRIDGE_AMOUNT_BTC={} ", self.bridge_amount_btc);
        env += &format!("CONFIRMATION_THRESHOLD={} ", self.confirmation_threshold);

        env += &format!("BITCOIN_REST_ENDPOINT={} ", self.bitcoin_rest_endpoint);
        env += &format!(
            "BITCOIN_REST_AUTH_HEADER={} ",
            self.bitcoin_rest_auth_header
        );

        env += &format!("CITREA_REST_ENDPOINT={} ", self.citrea_rest_endpoint);
        env += &format!(
            "BITCOIN_LIGHTCLIENT_CONTRACT_ADDRESS={} ",
            self.bitcoin_lightclient_contract_address
        );
        env += &format!("BRIDGE_CONTRACT_ADDRESS= {}", self.bridge_contract_address);
        env += &format!(
            "DECLARE_WITHDRAW_FILLER_PRIVATE_KEY= {}",
            self.declare_withdraw_filler_private_key
        );

        env += &format!("FAUCET_PRIVATE_KEY= {}", self.faucet_private_key);
        env += &format!("FAUCET_AMOUNT= {}", self.faucet_amount);
        env += &format!("FAUCET_AMOUNT_LIMIT= {}", self.faucet_amount_limit);
        env += &format!("FAUCET_COUNT_LIMIT= {}", self.faucet_count_limit);

        env += &format!("OPERATOR_URLS={} ", self.operator_urls);
        env += &format!("VERIFIER_URLS={} ", self.verifier_urls);
        env += &format!("AGGREGATOR_URL={} ", self.aggregator_url);

        env += &format!("TELEGRAM_BOT_API_KEY= ");
        env += &format!("TELEGRAM_CHAT_ID= ");

        env += &format!("CLOUDFLARE_SITE_KEY=1x00000000000000000000AA ");
        env += &format!("CLOUDFLARE_SECRET_KEY=1x0000000000000000000000000000000AA ");

        env
    }
}
