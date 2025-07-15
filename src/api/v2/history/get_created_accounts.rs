use std::collections::HashMap;
use actix_web::{get, web, HttpResponse, Responder};
use elasticsearch::SearchParts;
use redis::TypedCommands;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client;
use web::Query;
use crate::{configs, elastic_hyperion_redis};
use crate::elastic_hyperion_redis::get_redis_client;

#[derive(Debug, Deserialize,Serialize)]
pub struct ReqQuery {
    account: String,
    skip: Option<i64>,
    limit: Option<i64>,
}
#[get("/v2/history/get_created_accounts")]
async fn get(query: Query<ReqQuery>) -> impl Responder {
    let mut client = get_redis_client().await.unwrap();
    client.get()
    let limit = query.limit.unwrap_or(100);
    let size: i64 = if limit > 100 { 100 } else { limit };
    let index = configs::elastic_con::get_elastic_con_config().chain.clone() + "-action-*";
    let from = query.skip.unwrap_or(0);
    let req = json!(
        {
			    "query": {
				    "bool": {
					    "must": [
						    {"term": {"act.authorization.actor": query.account.to_lowercase()}},
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

    let res = client.search(SearchParts::Index(&[index.as_str()])).size(size).from(from).body(req).send().await.unwrap();
    HttpResponse::Ok().body(res.json::<Value>().await.unwrap().to_string())
}