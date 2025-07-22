use std::time::Instant;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use actix_web::web::{Bytes, Query};
use moka::future::Cache;

pub mod get_created_accounts;
pub mod get_abi_snapshot;
pub mod get_actions;
pub mod get_creator;
pub mod get_block;

pub async fn load_data<'c, 'b, 'a, 'd, F, Fut, T>(
    func: F,
    query: Query<T>,
    cache: Cache<u32, Bytes>,
    key: u32,
    start: Instant,
) -> HttpResponse<Bytes>
where
    F: Fn(u32, Query<T>, Cache<u32, Bytes>, Instant) -> Fut,
    Fut: Future<Output = Result<Bytes,String>> + 'static,
    T: 'static,
{
    if cache.contains_key(&key) {
        if let Some(value) = cache.get(&key).await {
            let res = String::from_utf8(value.to_vec()).unwrap();
            let (l, r) = res.split_at(1);
            let res = format!(
                "{}\"query_time\":\"{:?}\",\"cached\": true,{}",
                l,
                start.elapsed(),
                r
            );
            return HttpResponse::with_body(StatusCode::OK, Bytes::from(res));
        }
    }
    let res = func(key, query, cache, start).await;
    match res{
        Ok(r)=>HttpResponse::with_body(StatusCode::OK,r),
        Err(e)=>HttpResponse::with_body(StatusCode::BAD_REQUEST,Bytes::from(e))
    }
}
pub async fn load_data2<F,T,Fut>(func: F,query: &Query<T>,cache: &Cache<u32,Bytes>,key: &u32,start: &Instant) -> HttpResponse<Bytes>
where
    F: Fn(&u32,&Query<T>,&Cache<u32,Bytes>,&Instant) -> Fut,  // `F` — функция, возвращающая `Fut`
    Fut: Future<Output = Bytes>,
{
    if cache.contains_key(key){
        //println!("cache work! {:?}",cache.get(&key).await.unwrap());
        let value = cache.get(key).await;
        if value.is_some(){
            let res = value.clone().unwrap();
            let res = String::from_utf8(res.to_vec()).unwrap();
            let (l,r) = res.split_at(1);
            let res = format!("{}\"query_time\":\"{:?}\",\"cached\": true,{}",l,start.elapsed(),r);

            HttpResponse::with_body(StatusCode::OK,Bytes::from(res))
        }else{
            //println!("cache NOT work 1! {:?}",SystemTime::now());
            HttpResponse::with_body(StatusCode::OK, func(key, &query, &cache, &start).await)
        }

    } else {
        //println!("cache NOT work 2! {:?}",SystemTime::now());
        HttpResponse::with_body(StatusCode::OK, func(key, &query, &cache, &start).await)
    }

}