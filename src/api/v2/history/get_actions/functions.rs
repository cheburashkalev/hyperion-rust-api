use std::ops::Add;
use crate::api::v2::history::get_actions::ReqQuery;
use serde_json::{Value,Map, json};

pub fn add_sorted_by(query: &Value, query_body: &mut Value, sort_direction: String) {
    let sorted_by = query["sortedBy"].as_str();
    if sorted_by.is_some() {
        let v: Vec<_> = sorted_by.unwrap().split(':').collect();
        query_body["sort"] = json!({
            v[0] : v[1]
        });
    } else {
        query_body["sort"] = json!({
            "global_sequence": sort_direction
        });
    };
}
#[derive(Debug, Deserialize, Serialize)]
pub struct QueryStruct {
    pub(crate) bool: QueryBool,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct QueryBool {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) must: Vec<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) should: Vec<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) must_not: Vec<Value>,
    pub(crate) boost: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) minimum_should_match: Option<u32>,
}
pub fn process_multi_vars(query_struct: &mut QueryStruct, parts: Vec<&str>, field: &str) {
    let mut must = Vec::new();
    let mut must_not = Vec::new();
    parts.iter().for_each(|part| {
        if part.starts_with('!') {
            must_not.push(Value::String(part.replace("!", "")));
        } else {
            must.push(Value::String(part.to_string()));
        }
    });

    if must.len() > 1 {
        let should = must
            .iter()
            .map(|elem| {
                json!({
                    "term": {
                        field: elem
                    }
                })
            })
            .collect::<Vec<Value>>();
        query_struct.bool.must.push(json!(
            {
                "bool":{
                    "should": should
                }
            }
        ));
    } else if must.len() == 1 {
        query_struct.bool.must.push(json!(
            {
                "term":{
                    field: must[0]
                }
            }
        ));
    }

    if must_not.len() > 1 {
        let should = must_not
            .iter()
            .map(|elem| {
                json!({
                    "term": {
                        field: elem
                    }
                })
            })
            .collect::<Vec<Value>>();
        query_struct.bool.must_not.push(json!(
            {
                "bool":{
                    "should": should
                }
            }
        ));
    }
    else if must_not.len() == 1 {
        query_struct.bool.must_not.push(json!(
            {
                "term":{
                    field: must_not[0].as_str().unwrap().replace("!","")
                }
            }
        ));
    }
}
fn add_range_query(query_struct: &mut QueryStruct,prop: &str,pkey: &str,query: &Value){
    let parts = query[prop].as_str().unwrap().split_once('-').unwrap();
    query_struct.bool.must.push(json!({
        "range":{
            pkey:{
                "gte": parts.0,
                "lte": parts.1
            }
        }
    }));
}
use chrono::{DateTime};
use gxhash::{HashSet};
use serde::{Deserialize, Serialize};
use crate::configs;

pub fn apply_time_filter(query: &Value, query_struct: &mut QueryStruct) -> Result<(), String> {
    let after = query.get("after").and_then(|v| v.as_str());
    let before = query.get("before").and_then(|v| v.as_str());

    // Если оба фильтра отсутствуют - выходим
    if after.is_none() && before.is_none() {
        return Ok(());
    }

    // Обработка пробелов в датах
    let after_str = after.map(|s| s.replace(' ', "T").add("Z"));
    let before_str = before.map(|s| s.replace(' ', "T").add("Z"));

    // Проверяем наличие формата даты (содержит 'T')
    let is_datetime_format = after_str.as_ref().map(|s| s.contains('T')).unwrap_or(false) ||
        before_str.as_ref().map(|s| s.contains('T')).unwrap_or(false);

    if is_datetime_format {
        // Обработка временных меток
        let mut timestamp_filter = Map::new();
        let mut timestamp_range = Map::new();

        if let Some(before) = &before_str {
            let parsed_before = DateTime::parse_from_rfc3339(before).map_err(|_| format!("Invalid date format: {}", before));
            match parsed_before {
                Ok(_)=> clone_json_into_map(&Value::String(before.clone()), &mut timestamp_range, "lte"),
                Err(e)=> return Err(e)
            }
            //clone_json_into_map(&Value::String(DateTime::parse_from_rfc3339(before)
            //    .map(|dt| dt.with_timezone(&Utc))
            //    .or_else(|_| DateTime::from_str(before).map_err(|_| ()))
            //    .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true))
            //    .map_err(|_| format!("Invalid date format: {}", before))?), &mut timestamp_range, "lte");
        } else {
            timestamp_range.insert("lte".to_string(), Value::String("now".to_string()));
        }

        if let Some(after) = &after_str {
            let parsed_after = DateTime::parse_from_rfc3339(after);
            match parsed_after {
                Ok(_)=> clone_json_into_map(&Value::String(after.clone()), &mut timestamp_range, "gte"),
                Err(e)=> return Err(format!("Invalid date format: {} : {}", after,e.to_string()))
            }

        }

        timestamp_filter.insert(
            "@timestamp".to_string(),
            Value::Object(timestamp_range),
        );

        let range_filter = Value::Object(Map::from_iter(vec![
            ("range".to_string(), Value::Object(timestamp_filter))
        ]));

        query_struct.bool.must.push(range_filter);
    } else {
        // Обработка номеров блоков
        let mut block_filter = Map::new();
        let mut block_range = Map::new();

        if let Some(after) = after_str.and_then(|s| s.parse::<i64>().ok()) {
            if after > 0 {
                block_range.insert("gte".to_string(), Value::Number(after.into()));
            }
        }

        if let Some(before) = before_str.and_then(|s| s.parse::<i64>().ok()) {
            if before > 0 {
                block_range.insert("lte".to_string(), Value::Number(before.into()));
            }
        }

        if !block_range.is_empty() {
            block_filter.insert(
                "block_num".to_string(),
                Value::Object(block_range),
            );

            let range_filter = Value::Object(Map::from_iter(vec![
                ("range".to_string(), Value::Object(block_filter))
            ]));

            query_struct.bool.must.push(range_filter);
        }
    }

    Ok(())
}
fn clone_json_into_map(json: &Value, map: &mut Map<String, Value>, key: &str) {
    map.insert(key.to_string(), json.clone());
}
const PRIMARY_TERMS:[&str;15] = [
    "block_num",
    "block_id",
    "global_sequence",
    "producer",
    "@timestamp",
    "creator_action_ordinal",
    "action_ordinal",
    "cpu_usage_us",
    "net_usage_words",
    "trx_id",
    "receipts.receiver",
    "receipts.global_sequence",
    "receipts.recv_sequence",
    "receipts.auth_sequence.account",
    "receipts.auth_sequence.sequence"
];
pub(crate) const EXTENDED_ACTIONS:[&str;8] = [
    "transfer",
    "newaccount",
    "updateauth",
    "buyram",
    "buyrambytes",
    "delegatebw",
    "undelegatebw",
    "voteproducer"];
pub fn apply_generic_filters(
    query: &Value,
    query_struct: &mut QueryStruct,
    allowed_extra_params: &HashSet<String>,
) {
    // Получаем объект из JSON Value
    let Some(query_obj) = query.as_object() else { return };

    for (prop, value) in query_obj {
        // Разбиваем ключ на части по точкам
        let pair: Vec<&str> = prop.split('.').collect();
        let first_part = pair.first().cloned().unwrap_or("");

        // Проверяем условие для обработки
        if pair.len() > 1 || PRIMARY_TERMS.contains(&first_part) {
            // Определяем ключ для Elasticsearch
            let pkey = if pair.len() > 1 && allowed_extra_params.contains(first_part) {
                format!("@{}", prop)
            } else {
                prop.clone()
            };

            // Получаем строковое значение
            let Some(value_str) = value.as_str() else { continue };

            // Обработка range-запроса (если есть дефис)
            if value_str.contains('-') {
                add_range_query(query_struct, &pkey, value_str, query);
            }
            // Обработка нескольких значений (через запятую)
            else {
                let parts: Vec<&str> = value_str.split(',').collect();
                if parts.len() > 1 {
                    process_multi_vars(query_struct, parts, &prop);
                }
                // Обработка одиночного значения
                else {
                    // Специальная обработка для поля @transfer.memo
                    if pkey == "@transfer.memo" {
                        let mut _q_obj = json!(
                            {
                                &pkey:{
                                    "querty":parts[0]
                                }
                            }
                        );
                        if query["match_fuzziness"].is_string() {
                            _q_obj[&pkey]["fuzziness"] = query["match_fuzziness"].clone();
                        }
                        if query["match_operator"].is_string() {
                            _q_obj[&pkey]["operator"] = query["match_operator"].clone();
                        }

                        query_struct.bool.must.push(json!({
                            "match": _q_obj
                        }));
                    }else{
                        let and_parts = parts[0].split(' ').collect::<Vec<&str>>();
                        if and_parts.len() > 1 {
                            and_parts.iter().for_each(|value| {
                                query_struct.bool.must.push(json!(
                                    {
                                        "term":{
                                            &pkey:value
                                        }
                                    }));
                            })
                        } else{
                            if parts[0].starts_with('!') {
                                query_struct.bool.must_not.push(json!(
                                    {
                                        "term":{
                                            &pkey : parts[0].replace("!","")
                                        }
                                    }));
                            } else {
                                query_struct.bool.must.push(json!(
                                    {
                                        "term":{
                                            &pkey : parts[0]
                                        }
                                    }));
                            }
                        }
                    }
                }
            }
        }
    }
}
const TERMS:[&str;3] = [
    "notified",
    "receipts.receiver",
    "act.authorization.actor"
];
pub fn make_should_array(query: &Value) -> Vec<Value>{
    TERMS.iter().map(|entry|{
        json!({"term":{*entry : query["account"].clone()}})
    }).collect()
}
pub fn apply_code_action_filters(query: &ReqQuery, query_struct: &mut QueryStruct){
    if query.filter.is_some(){
        let terms: Vec<Value> = query.filter.clone().unwrap().split(',').collect::<Vec<&str>>().iter().map(|filter|{
            let filter = *filter;
            Value::from(if filter != "*:*" {
                let parts = filter.split(':').collect::<Vec<&str>>();
                if parts.len() == 2 {
                    let code = parts[0];
                    let method = parts[1];
                    if code != "*" {
                        return json!({
                            "term":{
                                "act.account": code
                            }
                        });
                    }
                    if method != "*" {
                        return json!({
                            "term":{
                                "act.name": method
                            }
                        });
                    }
                }
            })
        }).collect::<Vec<Value>>();
        if terms.len() > 0{
            query_struct.bool.should = terms;
            query_struct.bool.minimum_should_match = Some(1);
        }
    }
}

pub fn get_skip_limit(query: &ReqQuery,max_limit:i64) -> Result<(i64, i64),String> {
    let skip = query.skip.clone().unwrap_or(String::from("0"));
    let skip = skip.parse::<i64>().unwrap_or(0);
    if skip != 0{
        if skip < 0{
            return Err("skip must be greater than 0".to_string());
        }
        if skip > 10000{
            return Err("skip is above maximum internal limit: 10000. please limit your search scope or use pagination with before/after parameters".to_string());
        }
    }
    let limit = query.limit.clone().unwrap_or(String::from("200"));
    let limit = limit.parse::<i64>().unwrap_or(configs::elastic_con::get_elastic_con_config().get_actions.unwrap_or(200));
   if limit > max_limit{
        return Err(format!("limit too big, maximum: {}",max_limit));
    }
    if limit < 1{
        return Err("limit must be greater than 0".to_string());
    }
    Ok((skip,limit))
}
pub fn get_sort_direction(query: &ReqQuery) -> Result<String,String> {
    let sort = query.sort.clone().unwrap_or(String::from("desc"));
    if &sort == "asc" || &sort == "1"{
        Ok(String::from("asc"))
    } else if &sort == "desc" || &sort == "-1"{
        Ok(String::from("desc"))
    } else {
        return Err("invalid sort direction".to_string());
    }
}
pub fn apply_account_filters(query: &Value,query_struct: &mut QueryStruct){
    if query["account"].is_string(){
        query_struct.bool.must.push(json!(
            {
                "bool": {"should":make_should_array(query)}
            }
        ));
    }
}