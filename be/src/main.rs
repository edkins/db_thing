use actix::clock;
use actix_web::{get, web, App, HttpServer};
use serde_json::{map::Map, Value};
use std::{io::ErrorKind, sync::Arc, time::Duration};
use async_postgres::Socket;
use tokio_postgres::{Client, Config, Connection, Row, tls::NoTlsStream};

use crate::errors::MyError;
use crate::sql_to_json::SqlJson;

mod errors;
mod sql_to_json;

#[derive(Clone)]
struct AppState {
    client: Arc<Client>,
}

fn identifier(id: &str) -> Result<String, MyError> {
    let mut result = String::new();
    result.push('"');
    for ch in id.chars() {
        if ch == '\0' {
            return Err(MyError::NullInSqlToken);
        } else if ch == '"' {
            result.push_str("\"\"");
        } else {
            result.push(ch);
        }
    }
    result.push('"');
    Ok(result)
}

fn row_to_json(row: &Row) -> Result<Value, MyError> {
    let mut map = Map::new();
    for column in row.columns() {
        let name = column.name();
        let value:SqlJson = row.try_get(name)?;
        map.insert(name.to_owned(), value.0);
    }
    Ok(Value::Object(map))
}

fn rows_to_json(rows: &[Row]) -> Result<Value, MyError> {
    let jsons:Result<Vec<_>,MyError> = rows.into_iter().map(row_to_json).collect();
    let mut result = Map::new();
    result.insert("data".to_owned(), Value::Array(jsons?));
    Ok(Value::Object(result))
}

#[get("/api/admin/view/{view}")]
async fn admin_table(
    web::Path(view): web::Path<String>,
    data: web::Data<AppState>,
) -> actix_web::Result<web::Json<Value>, MyError> {
    let sql = format!("SELECT * FROM {}", identifier(&view)?);
    let rows = data.client.query(&sql as &str, &[]).await?;
    Ok(web::Json(rows_to_json(&rows)?))
}

async fn connect() -> std::io::Result<(Client, Connection<Socket,NoTlsStream>)> {
    let max_duration = Duration::from_secs(30);
    let mut duration = Duration::from_millis(10);
    loop {
        let mut config = Config::new();
        config.host("postgres").user("postgres");

        match async_postgres::connect(config).await {
            Ok(x) => return Ok(x),
            Err(e) => {
                if duration > max_duration {
                    return Err(std::io::Error::new(ErrorKind::ConnectionRefused, e));
                }
                println!("db connection error. Sleeping for {:?}", duration);
                clock::delay_for(duration).await;
                duration *= 2;
            }
        }
    }
}

async fn do_connection(connection: Connection<Socket, NoTlsStream>) {
    let _ = connection.await;
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Connecting");
    let (client, connection) = connect().await?;

    println!("Spawning connection");
    actix::spawn(do_connection(connection));

    let app_state = web::Data::new(AppState {
        client: Arc::new(client),
    });

    println!("Creating server");
    HttpServer::new(move || App::new().app_data(app_state.clone()).service(admin_table))
        .bind("0.0.0.0:8080")?
        .run()
        .await
}
