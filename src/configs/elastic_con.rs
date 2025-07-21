use std::sync::OnceLock;
use serde::{Deserialize, Serialize};
use crate::configs;

#[derive(Deserialize, Serialize, Debug)]
pub struct ElasticConConfig {
    pub url: String,
    pub path_cert_validation: String,
    pub login: String,
    pub pass: String,
    pub es_replicas: Option<u32>,
    pub chain: String,
    pub get_actions: Option<i64>,
    pub get_blocks: Option<u32>,
    pub get_created_accounts: Option<u32>,
    pub get_deltas: Option<u32>,
    pub get_key_accounts: Option<u32>,
    pub get_links: Option<u32>,
    pub get_proposals: Option<u32>,
    pub get_transfers: Option<u32>,
    pub get_trx_actions: Option<u32>,
    pub get_tokens: Option<u32>,
    pub get_top_holders: Option<u32>,
    pub get_voters: Option<u32>,

}
impl Default for ElasticConConfig {
    fn default() -> Self {
        ElasticConConfig {
            url: "https://localhost:9200".to_string(),
            path_cert_validation: "/home/andrei/pki/http.crt".to_string(),
            login: "elastic".to_string(),
            pass: "rILpAx=E8ZDhA7S5OF3+".to_string(),
            es_replicas: Some(0),
            chain: "gf".to_string(),
            get_actions: Some(1000),
            get_voters: Some(100),
            get_links: Some(1000),
            get_deltas: Some(1000),
            get_trx_actions: Some(200),

            get_transfers: None,
            get_blocks: None,
            get_created_accounts: None,
            get_key_accounts: None,
            get_proposals: None,
            get_tokens: None,
            get_top_holders: None,
        }
    }
}
static ELASTIC_CON_CONFIG: OnceLock<ElasticConConfig> = OnceLock::new();
const FILE_NAME_ELASTIC_CON_JSON: &str = "elastic-con.json";
pub fn get_elastic_con_config() -> &'static ElasticConConfig {
    ELASTIC_CON_CONFIG.get_or_init(|| {
        println!("Start loading \'ELASTIC_CON\' file: {}.", FILE_NAME_ELASTIC_CON_JSON);
        configs::load_configs_json(FILE_NAME_ELASTIC_CON_JSON, ElasticConConfig::default())
    })
}