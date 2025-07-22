use crate::api::v2::history::load_data;
use crate::{configs, elastic_hyperion_redis};
use actix_web::web::{Bytes, Data};
use actix_web::{Responder, get, web};
use elasticsearch::SearchParts;
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fmt::Display;
use std::sync::Mutex;
use std::time::{Instant};
use web::Query;

#[derive(Debug, Deserialize, Serialize)]
pub struct ReqQuery {
    account: String,
}
impl Display for ReqQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_abi_snapshot_{:?}", serde_json::to_string(self))
    }
}
#[get("/v2/history/get_creator")]
async fn get(query: Query<ReqQuery>, cache: Data<Mutex<Cache<u32, Bytes>>>) -> impl Responder {
    let start = Instant::now();
    let key = gxhash::gxhash32(query.to_string().as_bytes(), 12);
    let cache = cache.lock().unwrap().clone();
    load_data(get_from_elastic, query, cache, key, start).await
}
#[derive(serde::Deserialize)]
struct ElasticHit {
    _source: ElasticSource,
}

#[derive(serde::Deserialize)]
struct ElasticSource {
    act: ElasticAct,
    #[serde(rename = "@newaccount")]
    new_account: Option<ElasticNewAccount>,
    trx_id: String,
    #[serde(rename = "@timestamp")]
    timestamp: String,
}

#[derive(serde::Deserialize)]
struct ElasticAct {
    data: ElasticActData,
}

#[derive(serde::Deserialize)]
struct ElasticActData {
    newact: Option<String>,
}

#[derive(serde::Deserialize)]
struct ElasticNewAccount {
    newact: String,
}
const REQUEST_BODY_BLOCK: &str = "{\"block_num_or_id\": \"1\"}";
const REQUEST_BODY_ACCOUNT: &str = "{\"account_name\": \"";
async fn get_from_elastic(
    key: u32,
    query: Query<ReqQuery>,
    cache: Cache<u32, Bytes>,
    req_time: Instant,
) -> Result<Bytes, String> {
    if query.account == configs::nodeos::get_node_os_con_config().eosio_alias {
        let client = reqwest::Client::new();
        let raw_response_block = client
            .post(format!(
                "{}/v1/chain/get_block",
                configs::nodeos::get_node_os_con_config().http
            ))
            .json(&serde_json::from_str::<Value>(REQUEST_BODY_BLOCK).unwrap())
            .send()
            .await;
        if raw_response_block.is_err() {
            return Err(raw_response_block.unwrap_err().to_string());
        }
        let raw_response_block = raw_response_block.unwrap();

        let raw_parsed_response_block = raw_response_block.json::<Value>().await;
        if raw_parsed_response_block.is_err() {
            return Err(raw_parsed_response_block.unwrap_err().to_string());
        }

        let response_block = raw_parsed_response_block.unwrap();
        println!("{}",response_block.to_string());
        let string_res = json!({
            "timestamp": response_block["timestamp"],
            "creator": "__self__",
            "block_num": 1,
            "trx_id": ""
        })
        .to_string();
        let res = Bytes::from(string_res.clone());
        cache.remove(&key).await;
        cache.insert(key, res.clone()).await;
        let (l, r) = string_res.split_at(1);
        let query_time = format!("{:?}", req_time.elapsed());
        let res = format!("{}\"query_time\":\"{}\",{}", l, query_time, r);
        let res = Bytes::from(res);
        return Ok(res);
    }
    let index = configs::elastic_con::get_elastic_con_config().chain.clone() + "-action-*";
    let req = json!(
        {
            "query": {
                "bool": {
                    "must": [
                        {"term": {"@newaccount.newact": query.account}}
                    ]
                }
            }
        }
    );

    let client = elastic_hyperion_redis::get_elastic_client().await.unwrap();

    let res = client
        .search(SearchParts::Index(&[index.as_str()]))
        .size(1)
        .body(req)
        .send()
        .await
        .unwrap();

    let res = res.json::<Value>().await.unwrap();
    let hits = res["hits"]["hits"]
        .as_array()
        .ok_or("Invalid response")?;
    if hits.len() == 1{
        let result = hits[0]["_source"].clone();
        let string_res = json!({
            "timestamp": result["@timestamp"],
            "creator": result["act"]["data"]["creator"],
            "block_num": result["block_num"],
            "trx_id": result["trx_id"]
        }).to_string();
        let res = Bytes::from(string_res.clone());
        cache.remove(&key).await;
        cache.insert(key, res.clone()).await;
        let (l, r) = string_res.split_at(1);
        let query_time = format!("{:?}", req_time.elapsed());
        let res = format!("{}\"query_time\":\"{}\",{}", l, query_time, r);
        let res = Bytes::from(res);
        return Ok(res);
    }else{
        let client = reqwest::Client::new();
        let raw_response_account_info = client
            .post(format!(
                "{}/v1/chain/get_account",
                configs::nodeos::get_node_os_con_config().http
            ))
            .json(&serde_json::from_str::<Value>(&*format!("{}{}\"}}", REQUEST_BODY_ACCOUNT, query.account).to_string()).unwrap())
            .send()
            .await;
        match raw_response_account_info {
            Ok(v)=>{
                let account_info = v.json::<Value>().await.unwrap();
                if(!account_info["created"].is_string()){
                    return Err("account not found".to_string())
                }
                let req = json!({
                    "query": {
                        "bool": {
                            "must": [
                                {"term": {"@timestamp": account_info["timestamp"]}}
                            ]
                        }
                    }
                });

                let client = elastic_hyperion_redis::get_elastic_client().await.unwrap();
                let index = configs::elastic_con::get_elastic_con_config().chain.clone() + "-block-*";
                let res = client
                    .search(SearchParts::Index(&[index.as_str()]))
                    .size(1)
                    .body(req)
                    .send()
                    .await
                    .unwrap();

                let res = res.json::<Value>().await.unwrap();
                let hits = res["hits"]["hits"]
                    .as_array()
                    .ok_or("Invalid response")?;
                if hits.len() > 0 && hits[0]["_source"].is_object(){
                    let client = reqwest::Client::new();
                    let raw_response_block = client
                        .post(format!(
                            "{}/v1/chain/get_block",
                            configs::nodeos::get_node_os_con_config().http
                        ))
                        .json(REQUEST_BODY_BLOCK)
                        .send()
                        .await;
                    if raw_response_block.is_err() {
                        return Err(raw_response_block.unwrap_err().to_string());
                    }
                    let raw_response_block = raw_response_block.unwrap();

                    let raw_parsed_response_block = raw_response_block.json::<Value>().await;
                    if raw_parsed_response_block.is_err() {
                        return Err(raw_parsed_response_block.unwrap_err().to_string());
                    }
                    let response_block = raw_parsed_response_block.unwrap();
                    let mut creator: Value = Value::Null;
                    let mut trx_id: Value = Value::Null;
                    response_block["transactions"].as_array().unwrap().iter().for_each(|transaction|{

                        if transaction["trx"]["transaction"].is_object(){
                            let actions = transaction["trx"]["transaction"]["actions"].as_array();
                            if actions.is_some() {
                                let actions = actions.unwrap();
                                actions.iter().for_each(|act|{
                                    if act["name"].as_str().unwrap() == "newaccount"{
                                        let name = act["data"]["name"].as_str().unwrap();
                                        if name == query.account{
                                            creator = act["data"]["creator"].clone();
                                            trx_id = transaction["id"].clone();
                                        }
                                    }
                                });
                            }

                        }
                    });
                    let string_res = json!({
                        "timestamp": account_info["created"],
                        "creator": creator,
                        "block_num": response_block["block_num"],
                        "trx_id": trx_id
                    }).to_string();
                    let res = Bytes::from(string_res.clone());
                    cache.remove(&key).await;
                    cache.insert(key, res.clone()).await;
                    let (l, r) = string_res.split_at(1);
                    let query_time = format!("{:?}", req_time.elapsed());
                    let res = format!("{}\"query_time\":\"{}\",{}", l, query_time, r);
                    let res = Bytes::from(res);
                    Ok(res)
                }else{
                    Err("account creation not found".to_string())
                }
            },
            Err(_)=>Err("account not found".to_string())
        }
    }

}
