pub mod elastic_con;
pub mod redis_con;
pub mod nodeos;

use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::OnceLock;
use serde_json::{json, Value};
use log::{log, warn};
use serde::{de, Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
struct Loading {
    pub load_configs_from_etc: bool
}

impl Default for Loading {
    fn default() -> Self {
        Loading {
            load_configs_from_etc: true
        }
    }
}


// Lazy init
static LOADING_CONFIG: OnceLock<Loading> = OnceLock::new();
const PATH_CONFIGS_JSON: &str = "configs/";
const PATH_WORKDIR: &str = "./";

const PATH_LOADING_JSON: &str = "./configs/loading.json";

const PATH_ETC: &str = "/etc/hyperion-rust-api";
fn create_file_loading_json(config: &Loading){
    let json = serde_json::to_string_pretty(config).unwrap();
    fs::create_dir_all(format!("{}{}",PATH_WORKDIR, PATH_CONFIGS_JSON)).unwrap();
    let mut file = fs::File::create(PATH_LOADING_JSON).unwrap();
    file.write_all(json.as_bytes()).unwrap();
}
fn get_loading_config() -> &'static Loading {
    LOADING_CONFIG.get_or_init(|| {
        println!("Start load Loading file PATH: {}.",PATH_LOADING_JSON);
        let file = fs::read_to_string(PATH_LOADING_JSON);
        let config = Loading::default();
        match file {
            Ok(text) => {
                let config_raw = serde_json::from_str::<Loading>(&text);
                match config_raw {
                    Ok(config_file) => {
                        config_file
                    },
                    Err(e) => {
                        warn!("Error load file PATH: {}. Start Init from struct Loading::default(). FROM RUST LANG: {}.",PATH_LOADING_JSON,e);
                        create_file_loading_json(&config);
                        config
                    }
                }
            },
            Err(e)=>{
                warn!("Error load file PATH: {}. Start Init from struct Loading::default(). FROM RUST LANG: {}.",PATH_LOADING_JSON,e);
                create_file_loading_json(&config);
                config
            }
        }

    })
}


pub fn get_part_path_to_configs() -> String{
    if get_load_configs_from_etc(){
        format!("{}{}",PATH_ETC, PATH_CONFIGS_JSON )
    }
    else {
        format!("{}{}",PATH_WORKDIR, PATH_CONFIGS_JSON )
    }
}
fn get_load_configs_from_etc() -> bool {
    get_loading_config().load_configs_from_etc
}
pub fn save_json<T>(config: &T,path: &Path)
where
    T: ?Sized + Serialize,
{
    let json = serde_json::to_string_pretty(config).unwrap();

    fs::create_dir_all(path.parent().unwrap()).unwrap_or_else(|e|
        {
            panic!("Error saving config to: \'{}\' . Error: {}",path.display(), e);
        }) ;
    let mut file = fs::File::create(path).unwrap();
    file.write_all(json.as_bytes()).unwrap();
}
pub fn save_configs_json<T>(config: &T,file_name: &str)
where
    T: ?Sized + Serialize,
{
    save_json(config,&Path::new(format!("{}{}",get_part_path_to_configs(),file_name).as_str()));
}
pub fn load_configs_json<T>(file_name: &str,def:T) -> T
where
    T: for<'de> Deserialize<'de> + Default + Serialize,
{
    let file = fs::read_to_string(format!("{}{}",get_part_path_to_configs(),file_name));
    match file {
        Ok(text) => {
            read_config(&text, file_name,def).unwrap()
        },
        Err(e)=>{
            warn!("Error load file PATH: {}. Start Init from struct T::default(). FROM RUST LANG: {}.",file_name,e);
            save_configs_json(&def,file_name);
            def
        }
    }

}
fn read_config<'a, T>(text: &'a str,file_path: &str,def:T) -> serde_json::Result<T>
where
    T: de::Deserialize<'a> + Default + ?Sized +Serialize,
{
    let config_raw = serde_json::from_str::<T>(text);
    match config_raw {
        Ok(config) => {
            Ok(config)
        },
        Err(e) => {
            warn!("Error load file PATH: {}. Start Init from struct T::default(). FROM RUST LANG: {}.",file_path,e);
            save_configs_json(&def,file_path);
            Ok(def)
        }
    }
}