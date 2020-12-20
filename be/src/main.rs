use actix::clock;
use actix_web::{get, post, web, App, HttpServer};
use async_postgres::Socket;
use serde_json::{map::Map, Value};
use std::{io::ErrorKind, sync::Arc, time::Duration};
use tokio_postgres::{tls::NoTlsStream, Client, Config, Connection, Row};

use crate::errors::MyError;
use crate::sql_to_json::SqlJson;

mod errors;
mod migration;
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
        let value: SqlJson = row.try_get(name)?;
        map.insert(name.to_owned(), value.0);
    }
    Ok(Value::Object(map))
}

fn rows_to_json(rows: &[Row]) -> Result<Value, MyError> {
    let jsons: Result<Vec<_>, MyError> = rows.into_iter().map(row_to_json).collect();
    let mut result = Map::new();
    result.insert("data".to_owned(), Value::Array(jsons?));
    Ok(Value::Object(result))
}

#[get("/api/admin/sys/app")]
async fn admin_sys_app(data: web::Data<AppState>) -> actix_web::Result<web::Json<Value>, MyError> {
    let rows = data.client.query("
SELECT schema_name AS app
FROM information_schema.schemata
WHERE schema_name <> 'information_schema' AND schema_name <> 'pg_catalog'
", &[]).await?;
    Ok(web::Json(rows_to_json(&rows)?))
}

#[get("/api/admin/sys/view")]
async fn admin_sys_view(data: web::Data<AppState>) -> actix_web::Result<web::Json<Value>, MyError> {
    let rows = data.client.query("
SELECT table_schema AS app, table_name AS view
FROM information_schema.views
WHERE table_schema <> 'information_schema' AND table_schema <> 'pg_catalog'
", &[]).await?;
    Ok(web::Json(rows_to_json(&rows)?))
}

#[get("/api/{app}/view/{view}")]
async fn get_view(
    web::Path((app, view)): web::Path<(String, String)>,
    data: web::Data<AppState>,
) -> actix_web::Result<web::Json<Value>, MyError> {
    let sql = format!("SELECT * FROM {}.{}", identifier(&app)?, identifier(&view)?);
    let rows = data.client.query(&sql as &str, &[]).await?;
    Ok(web::Json(rows_to_json(&rows)?))
}

async fn connect() -> std::io::Result<(Client, Connection<Socket, NoTlsStream>)> {
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

#[post("/api/admin/migration/advance")]
async fn admin_migration_advance(
    data: web::Data<AppState>,
) -> actix_web::Result<web::Json<Value>, MyError> {
    migration::advance(&*data.client).await
}

#[post("/api/admin/migration/retract")]
async fn admin_migration_retract(
    data: web::Data<AppState>,
) -> actix_web::Result<web::Json<Value>, MyError> {
    migration::retract(&*data.client).await
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
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(get_view)
            .service(admin_sys_app)
            .service(admin_sys_view)
            .service(admin_migration_advance)
            .service(admin_migration_retract)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
