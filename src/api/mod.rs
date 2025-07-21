use std::str::FromStr;
use serde_json::{Map, Number, Value};

pub mod v2;
pub(crate) fn get_track_total_hits(query:&Value)->Result<Value, &str>{
    let o_track = &query["track"];
     if (o_track.is_string()){
         let track = o_track.as_str().unwrap();
         if track == "true"{
             return Ok(Value::Bool(true))
         }else if track == "false"{
             return Ok(Value::Bool(false))
         } else{
             let parsed = u32::from_str(track).unwrap_or(0);
             if(parsed > 0) {
                return Ok(Value::Number(Number::from(parsed)));
             } else {
                 return Err("failed to parse track param");
             }
         }
     }
     Ok(Value::Number(Number::from(10000)))
}
pub fn merge(a: &mut Value, b: &Value) {
    match (a, b) {
        // Оба значения - объекты
        (Value::Object(a_obj), Value::Object(b_obj)) => {
            for (key, b_val) in b_obj {
                // Пропускаем null-значения (аналог undefined)
                if b_val.is_null() {
                    continue;
                }

                if let Some(a_val) = a_obj.get_mut(key) {
                    // Рекурсивное слияние
                    merge(a_val, b_val);
                } else {
                    // Клонируем значение, если ключа нет в первом объекте
                    a_obj.insert(key.clone(), b_val.clone());
                }
            }
        }

        // Оба значения - массивы
        (Value::Array(a_arr), Value::Array(b_arr)) => {
            for (i, b_item) in b_arr.iter().enumerate() {
                if i < a_arr.len() {
                    // Рекурсивное слияние элементов
                    merge(&mut a_arr[i], b_item);
                } else {
                    // Добавляем новые элементы
                    a_arr.push(b_item.clone());
                }
            }
        }

        // Все остальные случаи - перезаписываем значение
        (a_val, b_val) if !b_val.is_null() => {
            *a_val = b_val.clone();
        }

        // Во всех остальных случаях ничего не делаем
        _ => {}
    }
}
pub(crate) fn merge_action_meta(action: &mut Value){
    let name = action["act"]["name"].as_str().unwrap().to_string();
    if action[format!("@{}",name)].is_object(){
        let mut act_data = action[format!("@{}",name)].clone();
        merge(&mut act_data, &action["act"]["data"]);
        action["act"]["data"] = act_data;
        let mut o_action: Map<String, Value> = action.as_object().unwrap().clone();
        o_action.remove(name.as_str());
        *action = Value::Object(o_action);
    }
    action["timestamp"] = action["@timestamp"].clone();
}