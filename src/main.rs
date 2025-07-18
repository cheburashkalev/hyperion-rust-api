mod configs;
mod elastic_hyperion_redis;
mod api;

use std::sync::{Mutex};
use actix_web::{get, middleware, post, web, App, HttpResponse, HttpServer, Responder};
use actix_web::web::{Bytes, Data};
use moka::future::Cache;
use redis::{Client, Connection};

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

// Структура для Redis подключения
struct RedisClient {
    client: Client,
}

impl RedisClient {
    fn new(redis_url: &str) -> redis::RedisResult<Self> {
        let client = Client::open(redis_url)?;
        Ok(RedisClient { client })
    }

    fn get_connection(&self) -> redis::RedisResult<Connection> {
        self.client.get_connection()
    }
}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    elastic_hyperion_redis::get_elastic_client().await;
    let cache:Cache<u32,Bytes> = Cache::new(10_000);

    //let redis_client = redis::Client::open("redis://127.0.0.1")
    //    .expect("Failed to connect to Redis");

    //let app_data = web::Data::new(AppState {
    //    redis: Arc::new(Mutex::new(redis_client)),
    //});
    env_logger::init();

    // Создаем Redis клиент
    let redis_url = "redis://127.0.0.1:6380";

    let redis_client = match RedisClient::new(redis_url) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to create Redis client: {}", e);
            std::process::exit(1);
        }
    };

    // Тестируем подключение
    match redis_client.get_connection() {
        Ok(mut con) => {
            match redis::cmd("PING").query::<String>(&mut con) {
                Ok(_) => println!("Successfully connected to Redis"),
                Err(e) => {
                    eprintln!("Redis ping failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to Redis: {}", e);
            std::process::exit(1);
        }
    }

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
            .route("/hey", web::get().to(manual_hello))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}