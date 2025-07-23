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
    contract: String,
    block: Option<i64>,
    fetch: Option<bool>,
}
impl Display for ReqQuery{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_abi_snapshot_{}_{}_{}", self.contract,self.block.unwrap_or(0),self.fetch.unwrap_or(false))
    }
}
#[get("/v2/history/get_abi_snapshot")]
async fn get(query: Query<ReqQuery>,cache:Data<Mutex<Cache<u32,Bytes>>>) -> impl Responder {
    let start = Instant::now();
    let key = gxhash::gxhash32(query.to_string().as_bytes(),12);
    let cache = cache.lock().unwrap().clone();
    load_data(get_from_elastic,query,cache,key,start).await

}
async fn get_from_elastic(key:u32, query: Query<ReqQuery>,cache:Cache<u32,Bytes>,req_time: Instant) -> Result<Bytes,String>{

    let index = configs::elastic_con::get_elastic_con_config().chain.clone() + "-abi-*";
    let mut must = Vec::new();
    must.push(json!({"term": {"account": query.contract}}));
    if query.block.is_some(){
        must.push(json!({"range": {"block": {"lte": query.block}}}));
    }
    let req = json!(
                {
                        "query": {
                            "bool": {
                                "must": must
                            }
                        },
                        "sort": [{"block": {"order": "desc"} }]
                }
            );

    let client = elastic_hyperion_redis::get_elastic_client().await.unwrap();

    let res_es = client
        .search(SearchParts::Index(&[index.as_str()]))
        .body(req)
        .send()
        .await
        .unwrap();
    if(!res_es.status_code().is_success()){
        let res = res_es.json::<Value>().await.unwrap();
        println!(
            "res: {} \n",
            serde_json::to_string_pretty(&res).unwrap().as_str()
        );
        let err = Err(json!({
            "statusCode": 500,
            "error": "Internal Server Error",
            "message": res["error"]["reason"]
        }).to_string());
        return err;
    }
    let res = res_es.json::<Value>().await.unwrap();
    let hits = res["hits"]["hits"].as_array().ok_or("Invalid response").unwrap();
    let res = if hits.len() > 0{
        let (key,value) = if query.fetch.unwrap_or(false){
            ("abi",hits.get(0).unwrap()["_source"]["abi"].clone())
        }else{
            ("present",Value::Bool(true))
        };
        json!(
            {
                key: value,
                "block_num": hits.get(0).unwrap()["_source"]["block"].clone()
            }
        )
    }else{
        json!(
            {
                "present": false,
                "error": format!("abi not found for {} until block {}",query.contract,query.block.unwrap_or(0))
            }
        )
    };
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
    let res = format!("{}\"query_time\":\"{}\",{}",l,query_time,r);
    let res = Bytes::from(res);
    //println!("Request took {:?} for {}", start.elapsed(), query.account);
    Ok(res)
}
