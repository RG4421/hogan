use anyhow::Result;
use hogan::config::Environment;
use rusqlite::{params, Connection, OpenFlags, NO_PARAMS};
use serde::Deserialize;
use serde::Serialize;

fn open_sql_db(db_path: &str, read_only: bool) -> Result<Connection> {
    let read_flag = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    let conn =
        Connection::open_with_flags(db_path, read_flag | OpenFlags::SQLITE_OPEN_SHARED_CACHE)?;

    if !read_only {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS hogan (
            key STRING PRIMARY KEY,
            data BLOB,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP )",
            NO_PARAMS,
        )?;
    }

    debug!("Opened sqlite connection to {}", db_path);

    Ok(conn)
}

pub fn read_sql_env(db_path: &str, env: &str, sha: &str) -> Result<Option<Environment>> {
    let conn = open_sql_db(db_path, true)?;
    let mut query = conn.prepare("SELECT data FROM hogan WHERE key = ? LIMIT 1")?;
    let key = gen_env_key(sha, env);
    let data: Option<rusqlite::Result<Vec<u8>>> =
        query.query_map(params![key], |row| Ok(row.get(0)?))?.next();
    if let Some(data) = data {
        let decoded: WritableEnvironment = match bincode::deserialize(&data?) {
            Ok(environment) => environment,
            Err(e) => {
                warn!("Unable to deserialize env: {} {:?}", key, e);
                return Err(e.into());
            }
        };
        Ok(Some(decoded.into()))
    } else {
        debug!("Unable to find {} in sqlite db", key);
        Ok(None)
    }
}

pub fn write_sql_env(db_path: &str, env: &str, sha: &str, data: &Environment) -> Result<usize> {
    let conn = open_sql_db(db_path, false)?;
    let key = gen_env_key(sha, env);
    let env_data: WritableEnvironment = data.into();
    let data = bincode::serialize(&env_data)?;

    debug!("Writing to DB. Key: {} Size: {}", key, data.len());

    conn.execute(
        "INSERT INTO hogan (key, data) VALUES (?1, ?2)",
        params![key, data],
    )
    .map_err(|e| e.into())
}

fn gen_env_key(sha: &str, env: &str) -> String {
    format!("{}::{}", sha, env)
}

#[derive(Default, Serialize, Deserialize, Debug)]
struct WritableEnvironment {
    config_data: String,
    environment: String,
    environment_type: Option<String>,
}

impl From<&Environment> for WritableEnvironment {
    fn from(environment: &Environment) -> Self {
        WritableEnvironment {
            config_data: environment.config_data.to_string(),
            environment: environment.environment.to_owned(),
            environment_type: environment.environment_type.to_owned(),
        }
    }
}

impl From<WritableEnvironment> for Environment {
    fn from(environment: WritableEnvironment) -> Self {
        Environment {
            config_data: serde_json::from_str(&environment.config_data).unwrap(),
            environment: environment.environment.to_owned(),
            environment_type: environment.environment_type.to_owned(),
        }
    }
}
