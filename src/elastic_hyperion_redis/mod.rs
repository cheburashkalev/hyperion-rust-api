use crate::configs;
use crate::configs::{elastic_con, redis_con};
use elasticsearch::auth::Credentials::Basic;
use elasticsearch::cert::{Certificate, CertificateValidation};
use elasticsearch::http::transport::{SingleNodeConnectionPool, TransportBuilder};
use elasticsearch::ilm::{IlmDeleteLifecycleParts, IlmPutLifecycleParts};
use elasticsearch::indices::IndicesPutTemplateParts;
use elasticsearch::{Elasticsearch, IndexParts};
use std::error::Error;
use std::sync::OnceLock;
use redis::{Client, Connection, RedisConnectionInfo};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

static ELASTIC_CON: OnceLock<Elasticsearch> = OnceLock::new();
//10.10.25.11
pub async fn get_elastic_client() -> Result<&'static Elasticsearch, Box<dyn Error>> {
    let elas = ELASTIC_CON.get();
    if elas.is_some() {
        Ok(elas.unwrap())
    } else {
        let config = elastic_con::get_elastic_con_config();
        let mut http_crt_file = File::open(config.path_cert_validation.as_str())
            .await
            .unwrap_or_else(|e|{
                panic!("Unable load crt. Path: {}", config.path_cert_validation);
            });
        let mut http_buf = Vec::new();
        http_crt_file.read_to_end(&mut http_buf).await?;

        let client = ELASTIC_CON.get_or_init(|| {
            let conn_pool = SingleNodeConnectionPool::new(config.url.parse().unwrap());
            let http_cert = Certificate::from_pem(http_buf.as_slice()).unwrap();
            let transport = TransportBuilder::new(conn_pool)
                .auth(Basic(config.login.clone(), config.pass.clone()))
                .cert_validation(CertificateValidation::Certificate(http_cert))
                .build()
                .unwrap();
            Elasticsearch::new(transport)
        });
        Ok(client)
    }
}
static REDIS_CON: OnceLock<Connection> = OnceLock::new();
//10.10.25.11
pub async fn get_redis_client() -> Result<&'static Connection, Box<dyn Error>> {
    let elas = REDIS_CON.get();
    if elas.is_some() {
        Ok(elas.unwrap())
    } else {
        let config = redis_con::get_redis_con_config();
        let elas = REDIS_CON.get_or_init(
            || {
                let info = RedisConnectionInfo {
                    password: Some(config.pass.clone()),
                    ..Default::default()
                };
                let client = Client::open(redis::ConnectionInfo {
                    addr: redis::ConnectionAddr::Tcp(config.url.clone(), config.port),
                    redis: info,
                }).unwrap();
                client.get_connection().unwrap()
            });

        Ok(elas)
    }
}
