mod functions;

use std::fmt::Display;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::{configs, elastic_hyperion_redis};
use actix_web::{Responder, get, web};

use crate::api::v2::history::get_actions::functions::{
    QueryBool, QueryStruct, add_sorted_by, apply_account_filters, apply_code_action_filters,
    apply_generic_filters, apply_time_filter, get_skip_limit, get_sort_direction,
};
use crate::api::v2::history::load_data;
use crate::api::{get_track_total_hits, merge_action_meta};
use actix_web::web::{Bytes, Data};
use elasticsearch::SearchParts;
use gxhash::{HashSet, HashSetExt};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::sleep;
use web::Query;

#[derive(Debug, Deserialize, Serialize)]
pub struct ReqQuery {
    account: Option<String>,//tested
    track: Option<String>, //tested
    filter: Option<String>,//tested
    sort: Option<String>,//tested
    #[serde(rename = "sortedBy")]
    sorted_by: Option<String>,//tested
    after: Option<String>, //tested
    before: Option<String>, //tested
    skip: Option<String>,     //i64 0+ //tested
    limit: Option<String>,    //i64 1+ //tested
    simple: Option<String>,   //bool //tested
    hot_only: Option<String>, //bool //tested
    #[serde(rename = "noBinary")]
    no_binary: Option<String>, //bool //tested
    #[serde(rename = "checkLib")]
    check_lib: Option<String>, //bool //tested
}
impl Display for ReqQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "get_abi_snapshot_{:?}", serde_json::to_string(self))
    }
}
#[get("/v2/history/get_actions")]
pub async fn get(query: Query<Value>, cache: Data<Mutex<Cache<u32, Bytes>>>) -> impl Responder {
    let start = Instant::now();
    let key = gxhash::gxhash32(query.to_string().as_bytes(), 12);
    let cache = cache.lock().unwrap().clone();
    load_data(get_from_elastic, query, cache, key, start).await
}
async fn get_from_elastic(
    key: u32,
    query: Query<Value>,
    cache: Cache<u32, Bytes>,
    req_time: Instant,
) -> Result<Bytes, String> {
    let req = query.0.clone();
    let parsed_query: ReqQuery = serde_json::from_value(query.0).expect("REASON");
    let (index, hot_only) = match &parsed_query.hot_only {
        Some(e) => {
            if e == "true" {
                (
                    configs::elastic_con::get_elastic_con_config().chain.clone() + "-action",
                    true,
                )
            } else {
                (
                    configs::elastic_con::get_elastic_con_config().chain.clone() + "-action-v1-*",
                    false,
                )
            }
        }
        None => (
            configs::elastic_con::get_elastic_con_config().chain.clone() + "-action-*",
            false,
        ),
    };
    let max_actions = configs::elastic_con::get_elastic_con_config().get_actions;
    let r = get_skip_limit(&parsed_query, max_actions.unwrap_or(200));
    if r.is_err() {
        return Err(r.unwrap_err());
    }
    let (skip, limit) = r.unwrap();
    let sort_direction = get_sort_direction(&parsed_query);
    if sort_direction.is_err() {
        return Err(sort_direction.unwrap_err());
    }
    let sort_direction = sort_direction.unwrap();
    let mut querty_struct = QueryStruct {
        bool: QueryBool {
            must_not: Vec::new(),
            boost: 1.0,
            must: Vec::new(),
            should: Vec::new(),
            minimum_should_match: None,
        },
    };

    apply_account_filters(&req, &mut querty_struct);
    let mut ext_act: HashSet<String> = HashSet::with_capacity(8);
    functions::EXTENDED_ACTIONS.iter().for_each(|x| {
        ext_act.insert(x.to_string());
    });
    apply_generic_filters(&req, &mut querty_struct, &ext_act);
    match apply_time_filter(&req, &mut querty_struct) {
        Ok(e) => e,
        Err(e) => return Err(e),
    };
    apply_code_action_filters(&parsed_query, &mut querty_struct);
    let track_total_hits = get_track_total_hits(&req);
    if track_total_hits.is_err() {
        return Err(track_total_hits.unwrap_err().to_string());
    }
    let mut query_body = json!({
        "track_total_hits": track_total_hits?,
        "query": querty_struct
    });
    add_sorted_by(&req, &mut query_body, sort_direction);
    let client = elastic_hyperion_redis::get_elastic_client().await.unwrap();
    println!(
        "query_body {} \n",
        serde_json::to_string_pretty(&query_body).unwrap().as_str()
    );
    let res_es = client
        .search(SearchParts::Index(&[index.as_str()]))
        .from(skip)
        .size(limit)
        .body(query_body)
        .send()
        .await
        .unwrap();

    if(!res_es.status_code().is_success()){
        let status_code = res_es.status_code().as_u16();

        let res = res_es.json::<Value>().await.unwrap();
        println!(
            "res: {} \n",
            serde_json::to_string_pretty(&res).unwrap().as_str()
        );
        let err = Err(json!({
            "statusCode": status_code,
            "error": "Bad Request",
            "message": res["error"]["reason"]
        }).to_string());
        return err;
    }
    let res = res_es.json::<Value>().await.unwrap();

    println!(
        "res: {} \n",
        serde_json::to_string_pretty(&res).unwrap().as_str()
    );
    let hits = res["hits"]["hits"]
        .as_array()
        .ok_or("Invalid response")
        .unwrap();
    let mut response = json!({
        "lib": 0,
        "total": res["hits"]["total"]
    });
    if hot_only {
        response["hot_only"] = Value::Bool(hot_only);
    }
    let check_lib: bool = parsed_query
        .check_lib
        .clone()
        .unwrap()
        .parse()
        .unwrap_or(false);
    if check_lib {
        let raw_response_info = reqwest::get(
            format!(
                "{}/v1/chain/get_info",
                configs::nodeos::get_node_os_con_config().http
            )
            .as_str(),
        )
        .await;
        if raw_response_info.is_err() {
            return Err(raw_response_info.unwrap_err().to_string());
        }
        let raw_response_info = raw_response_info.unwrap();
        println!("{:?}", raw_response_info);
        let raw_parsed_response_info = raw_response_info.json::<Value>().await;
        if raw_parsed_response_info.is_err() {
            return Err(raw_parsed_response_info.unwrap_err().to_string());
        }
        let response_info = raw_parsed_response_info.unwrap();
        response["lib"] = response_info["last_irreversible_block_num"].clone();
    }
    match parsed_query.simple.as_ref() {
        Some(e) => {
            if e == "true" {
                response["simple_actions"] = Value::Array(Vec::new());
            } else {
                response["actions"] = Value::Array(Vec::new());
            }
        }
        None => response["actions"] = Value::Array(Vec::new()),
    }
    if hits.len() > 0 {
        hits.iter()
            .map(|a| a["_source"].clone())
            .for_each(|action| {
                let mut action = action.clone();
                let raw_data = &action["act"]["data"];
                if raw_data.is_object() {
                    if raw_data["account"].is_string()
                        && raw_data["name"].is_string()
                        && raw_data["authorization"].is_array()
                        && raw_data["data"].is_object()
                    {
                        action["act"]["data"] = (&action["act"]["data"]["data"]).clone();
                    }
                } else {
                    println!("/v2/history/get_actions: unable parse data");
                }
                println!(
                    "query_body {} \n",
                    serde_json::to_string_pretty(&action).unwrap().as_str()
                );
                merge_action_meta(&mut action);
                if let Some(no_binary) = &parsed_query.no_binary {
                    if no_binary == "true" {
                        // Создаем вектор изменений, не удерживая ссылку на action
                        let mut changes = Vec::new();

                        if let Some(data_obj) = action["act"]["data"].as_object() {
                            for (key, value) in data_obj {
                                if let Some(raw_value) = value.as_str() {
                                    if raw_value.len() > 256 {
                                        changes.push((
                                            key.clone(),
                                            Value::String(format!("{}...", &raw_value[0..32])),
                                        ));
                                    }
                                }
                            }
                        }

                        // Применяем изменения после освобождения заимствования
                        if let Some(data) = action["act"]["data"].as_object_mut() {
                            for (key, new_value) in changes {
                                data.insert(key, new_value);
                            }
                        }
                    }
                }
                let simple = &parsed_query.simple;
                match simple {
                    Some(s) => {
                        if s == "true" {
                            let receipts = action["receipts"].as_array().unwrap();
                            let notified = receipts
                                .iter()
                                .map(|r| r["receiver"].as_str().unwrap())
                                .collect::<Vec<&str>>();
                            let lib = response["lib"].as_i64();
                            let o_a_simple_actions = response["simple_actions"].as_array_mut();
                            let actors = action["act"]["authorization"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|a| {
                                    format!(
                                        "{}@{}",
                                        a["actor"].as_str().unwrap(),
                                        a["permission"].as_str().unwrap()
                                    )
                                })
                                .collect::<Vec<String>>()
                                .join(",");
                            let mut simple_action = json!({
                                "block": action["block_num"],
                                "timestamp": action["@timestamp"],
                                "transaction_id": action["trx_id"],
                                "actors": actors,
                                "notified": notified.join(","),
                                "contract": action["act"]["account"],
                                "action": action["act"]["name"],
                                "data": action["act"]["data"]
                            });

                            if lib.is_some() {
                                let lib = lib.unwrap();
                                simple_action["irreversible"] =
                                    Value::Bool(lib > action["block_num"].as_i64().unwrap());
                            }
                            o_a_simple_actions.unwrap().push(simple_action);
                        }
                        else{
                            response["actions"].as_array_mut().unwrap().push(action)
                        }
                    }
                    None => response["actions"].as_array_mut().unwrap().push(action),
                }
            });
    };
    let string_res = response.to_string();
    let response = Bytes::from(string_res.clone());
    cache.insert(key, response.clone()).await;
    let cache = cache.clone();
    let key = key.clone();
    tokio::spawn(async move {
        sleep(Duration::from_secs(1)).await;
        cache.invalidate(&key).await;
    });
    let query_time = format!("{:?}", req_time.elapsed());
    let (l, r) = string_res.split_at(1);
    let response = format!(
        "{}\"query_time\":\"{}\",\"cached\":false,{}",
        l, query_time, r
    );
    let response = Bytes::from(response);
    //println!("Request took {:?} for {}", start.elapsed(), query.account);
    Ok(response)
    //при выдаче ответа поставить "cached" : false
}
