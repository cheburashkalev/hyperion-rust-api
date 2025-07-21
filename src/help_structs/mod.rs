use chrono::{DateTime, Utc};
use serde::Deserialize;
pub mod eosio_datetime_format {
    use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

    // The signature of a serialize_with function must follow the pattern:
    //
    //    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
    //    where
    //        S: Serializer
    //
    // although it may also be generic over the input types T.
    #[allow(dead_code)]
    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", date.format(FORMAT));
        serializer.serialize_str(&s)
    }

    // The signature of a deserialize_with function must follow the pattern:
    //
    //    fn deserialize<'de, D>(D) -> Result<T, D::Error>
    //    where
    //        D: Deserializer<'de>
    //
    // although it may also be generic over the output types T.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s= String::deserialize(deserializer);
        let s =match s {
            Ok(v)=> v,
            Err(e)=> return Err(serde::de::Error::custom(e))
        };
        let len = s.len();
        let slice_len = if s.contains('.') {
            len.saturating_sub(4)
        } else {
            len
        };

        // match Utc.datetime_from_str(&s, FORMAT) {
        let sliced = &s[0..slice_len];
        match NaiveDateTime::parse_from_str(sliced, FORMAT) {
            Err(_e) => {
                Ok(None)
                //eprintln!("DateTime Fail {} {:#?}", sliced, _e);
                //Err(serde::de::Error::custom(_e))
            }
            Ok(dt) => Ok(Some(Utc.from_utc_datetime(&dt))),
        }
    }
}
#[derive(Debug, Deserialize)]
pub(crate) struct GetInfoResult {
    pub server_version: String,
    pub chain_id: String,
    pub head_block_num: u64,
    pub last_irreversible_block_num: u64,
    pub last_irreversible_block_id: String,
    #[serde(with = "eosio_datetime_format")]
    pub last_irreversible_block_time: Option<DateTime<Utc>>,
    pub head_block_id: String,
    #[serde(with = "eosio_datetime_format")]
    pub head_block_time: Option<DateTime<Utc>>,
    pub head_block_producer: String,
    pub virtual_block_cpu_limit: u64,
    pub virtual_block_net_limit: u64,
    pub block_cpu_limit: u64,
    pub block_net_limit: u64,
    pub server_version_string:  Option<String>,
    pub fork_db_head_block_num: Option<u64>,
    pub fork_db_head_block_id: Option<String>,
    pub server_full_version_string: Option<String>,
    pub first_block_num: Option<u64>,
}