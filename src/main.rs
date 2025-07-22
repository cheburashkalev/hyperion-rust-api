mod configs;
mod elastic_hyperion_redis;
mod api;
mod help_structs;

use std::sync::{Mutex};
use actix_web::{get, middleware, post, web, App, HttpResponse, HttpServer, Responder};
use actix_web::web::{Bytes, Data};
use moka::future::Cache;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}


#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cache:Cache<u32,Bytes> = Cache::new(10_000);

    //let redis_client = redis::Client::open("redis://127.0.0.1")
    //    .expect("Failed to connect to Redis");

    //let app_data = web::Data::new(AppState {
    //    redis: Arc::new(Mutex::new(redis_client)),
    //});
    env_logger::init();

    // Создаем общий ресурс для приложения
    //let redis_data = web::Data::new(Mutex::new(redis_client));

    println!("Starting server at http://127.0.0.1:8080");
    //let redis_cfg = configs::redis_con::get_redis_con_config();
    //let connection_info = ConnectionInfo{
    //    addr: ConnectionAddr::Tcp(redis_cfg.url.clone(), redis_cfg.port),
    //    redis: RedisConnectionInfo::default(),
    //};
    //let store = RedisBackend::connect(connection_info).await.unwrap();
    //let storage = Storage::build().expiry_store(store).finish();
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(Mutex::new(cache.clone())))

            //.app_data(Data::new(storage.clone()))
            // enable logger
            .wrap(middleware::Logger::default())
            .service(hello)
            .service(echo)
            .service(api::v2::history::get_created_accounts::get)
            .service(api::v2::history::get_abi_snapshot::get)
            .service(api::v2::history::get_actions::get)
            .service(api::v2::history::get_creator::get)
            .service(api::v2::history::get_block::get)
            .route("/hey", web::get().to(manual_hello))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}