use std::fmt::Display;
use std::sync::{Mutex};
use std::time::{Duration, Instant, SystemTime};
use rayon::iter::ParallelIterator;
use crate::{configs, elastic_hyperion_redis};
use actix_web::{HttpResponse, Responder, get, web};
use actix_web::http::StatusCode;
use actix_web::web::{Bytes, Data};
use elasticsearch::SearchParts;
use moka::future::Cache;
use rayon::iter::IntoParallelRefIterator;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::sleep;
use web::Query;

#[derive(Debug, Deserialize, Serialize)]
pub struct ReqQuery {
    account: String,
    skip: Option<i64>,
    limit: Option<i64>,
}
impl Display for ReqQuery{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_created_accounts_{}_{}_{}", self.account,self.skip.unwrap_or(0),self.limit.unwrap_or(100))
    }
}
#[get("/v2/history/get_created_accounts")]
async fn get(query: Query<ReqQuery>,cache:Data<Mutex<Cache<u32,Bytes>>>) -> impl Responder {
    let start = std::time::Instant::now();
    let key = gxhash::gxhash32(query.to_string().as_bytes(),12);
    let cache = cache.lock().unwrap().clone();
    if cache.contains_key(&key){
        //println!("cache work! {:?}",cache.get(&key).await.unwrap());
        let value = cache.get(&key).await;
        if value.is_some(){
            let res = value.clone().unwrap();
            let res = String::from_utf8(res.to_vec()).unwrap();
            let (l,r) = res.split_at(1);
            let res = format!("{}\"query_time\":\"{:?}\",\"cached\": true,{}",l,start.elapsed(),r);

            HttpResponse::with_body(StatusCode::OK,Bytes::from(res))
        }else{
            println!("cache NOT work 1! {:?}",SystemTime::now());
            HttpResponse::with_body(StatusCode::OK,get_from_elastic(key, &query, cache, &start).await)
        }

    } else {
        println!("cache NOT work 2! {:?}",SystemTime::now());
        HttpResponse::with_body(StatusCode::OK, get_from_elastic(key,&query,cache, &start).await)
    }

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
async fn get_from_elastic(key:u32, query: &Query<ReqQuery>,cache:Cache<u32,Bytes>,req_time: &Instant) -> Bytes{

    let index = configs::elastic_con::get_elastic_con_config().chain.clone() + "-action-*";
    let actor = query.account.to_lowercase();
    let from = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);
    let size: i64 = if limit > 100 { 100 } else { limit };
    //let start = std::time::Instant::now();
    let req = json!(
                {
                        "query": {
                            "bool": {
                                "must": [
                                    {"term": {"act.authorization.actor": actor}},
                                    {"term": {"act.name": "newaccount"}},
                                    {"term": {"act.account": "eosio"}}
                                ]
                            }
                        },
                        "sort": {
                            "global_sequence": "desc"
                        }
                }
            );

    let client = elastic_hyperion_redis::get_elastic_client().await.unwrap();

    let res = client
        .search(SearchParts::Index(&[index.as_str()]))
        .size(size)
        .from(from)
        .body(req)
        .send()
        .await
        .unwrap();
    //println!("Request took {:?} for {}", start.elapsed(), query.account);

    let res = res.json::<Value>().await.unwrap();
    let hits = res["hits"]["hits"].as_array().ok_or("Invalid response").unwrap();

    let accounts = hits.par_iter()
        .map(|hit| {
            let hit: ElasticHit = serde_json::from_value(hit.clone()).unwrap();
            let name = hit._source.act.data.newact
                .or(hit._source.new_account.map(|x| x.newact))
                .ok_or("Missing 'newact'")?;
            Ok::<Value, &str>(json!({
                "name": name,
                "trx_id": hit._source.trx_id,
                "timestamp": hit._source.timestamp,
            }))
        })
        .collect::<Result<Vec<_>, _>>().unwrap();

    // ... выполнение запроса ...
    let res = Bytes::from(json!({"accounts": accounts }).to_string());
    cache.insert(key,res.clone()).await;
    let cache = cache.clone();
    let key = key.clone();
    tokio::spawn(async move{
        sleep(Duration::from_secs(1)).await;
        cache.invalidate(&key).await;
    });
    let query_time =format!("{:?}",req_time.elapsed());
    let res = Bytes::from(json!({"query_time":query_time,"accounts": accounts }).to_string());
    //println!("Request took {:?} for {}", start.elapsed(), query.account);
    res
}
