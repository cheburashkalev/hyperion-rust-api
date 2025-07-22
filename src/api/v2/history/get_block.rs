use std::fmt::Display;
use std::sync::{Mutex};
use std::time::{Duration, Instant};

use crate::{configs, elastic_hyperion_redis};
use actix_web::{Responder, get, web};

use actix_web::web::{Bytes, Data};
use elasticsearch::SearchParts;
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::sleep;
use web::Query;
use crate::api::v2::history::load_data;

#[derive(Debug, Deserialize, Serialize)]
pub struct ReqQuery {
    block_num: i64
}
impl Display for ReqQuery{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_block_{}", self.block_num)
    }
}
#[get("/v2/history/get_block")]
async fn get(query: Query<ReqQuery>,cache:Data<Mutex<Cache<u32,Bytes>>>) -> impl Responder {
    let start = Instant::now();
    let key = gxhash::gxhash32(query.to_string().as_bytes(),12);
    let cache = cache.lock().unwrap().clone();
    load_data(get_from_elastic,query,cache,key,start).await

}
async fn get_from_elastic(key:u32, query: Query<ReqQuery>,cache:Cache<u32,Bytes>,req_time: Instant) -> Result<Bytes,String>{

    if(query.block_num < 1){
        return Err("Invalid block number".to_string());
    }
    let index = configs::elastic_con::get_elastic_con_config().chain.clone() + "-block-*";
    let req = json!(
                {
                        "query": {
                            "bool": {
                                "must": {
                                    "term": {"block_num": query.block_num}
                                }
                            }
                        }
                }
            );

    let client = elastic_hyperion_redis::get_elastic_client().await.unwrap();

    let res = client
        .search(SearchParts::Index(&[index.as_str()]))
        .body(req)
        .send()
        .await
        .unwrap();

    let res = res.json::<Value>().await.unwrap();
    let hits = res["hits"]["hits"].as_array().ok_or("Invalid response").unwrap();
    if hits.len() == 0{
        return Err(format!("Block {} not found",query.block_num));
    }
    let block = &hits[0]["_source"];
    let mut res = json!({
        "lib":0,
        "total": {
            "value": 1,
            "relation": "eq"
        },
        "@timestamp": block["@timestamp"],
        "block_num": block["block_num"],
        "block_id": block["block_id"],
        "prev_id": block["prev_id"],
        "producer": block["producer"],
        "schedule_version": block["schedule_version"],
        "cpu_usage": block["cpu_usage"],
        "net_usage": block["net_usage"],
        "trx_count": block["trx_count"],
    });
    if block.as_object().unwrap().iter().any(|x|{x.0 == "new_producers" && !x.1.is_null()}){
        res["new_producers"] = block["new_producers"].clone();
    }
    let string_res = res.to_string();
    let res = Bytes::from(string_res.clone());
    cache.insert(key, res.clone()).await;
    let cache = cache.clone();
    let key = key.clone();
    tokio::spawn(async move{
        sleep(Duration::from_secs(1)).await;
        cache.invalidate(&key).await;
    });
    let query_time =format!("{:?}",req_time.elapsed());
    let (l,r) =string_res.split_at(1);
    let res = format!("{}\"query_time\":\"{}\",\"cached\":false,{}",l,query_time,r);
    let res = Bytes::from(res);
    //println!("Request took {:?} for {}", start.elapsed(), query.account);
    Ok(res)
}
