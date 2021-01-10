use actix::clock;
use actix_web::{delete, get, patch, post, web, App, HttpResponse, HttpServer};
use async_postgres::Socket;
use serde::{Deserialize, Serialize};
use serde_json::{map::Map, Value};
use std::{io::ErrorKind, sync::Arc, time::Duration};
use tokio_postgres::{tls::NoTlsStream, Client, Config, Connection, Row};

use crate::conf::{DbThingConf,PostgresConf};
use crate::errors::MyError;
use crate::sql_to_json::SqlJson;

mod conf;
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
        let value: SqlJson = row.try_get(name)?;
        map.insert(name.to_owned(), value.0);
    }
    Ok(Value::Object(map))
}

fn rows_to_json(rows: &[Row]) -> Result<Value, MyError> {
    let jsons: Result<Vec<_>, MyError> = rows.iter().map(row_to_json).collect();
    let mut result = Map::new();
    result.insert("data".to_owned(), Value::Array(jsons?));
    Ok(Value::Object(result))
}

#[get("/api/admin/sys/app")]
async fn admin_sys_app(data: web::Data<AppState>) -> actix_web::Result<web::Json<Value>, MyError> {
    let rows = data
        .client
        .query(
            "
SELECT schema_name AS app
FROM information_schema.schemata
ORDER BY app ASC
",
            &[],
        )
        .await?;
    Ok(web::Json(rows_to_json(&rows)?))
}

#[delete("/api/admin/sys/app")]
async fn admin_sys_app_del(
    req: web::Query<JustApp>,
    data: web::Data<AppState>,
) -> actix_web::Result<HttpResponse, MyError> {
    if !valid_app_name(&req.app) {
        return Err(MyError::InvalidAppName);
    }
    let sql = format!("DROP SCHEMA {} RESTRICT", identifier(&req.app)?);
    data.client.query(&sql as &str, &[]).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize)]
struct NewAppRequest {
    data: Vec<NewAppData>,
}
#[derive(Deserialize)]
struct NewAppData {
    app: String,
}

fn valid_app_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.starts_with("pg_")
        || name.starts_with('_')
        || name == "information_schema"
        || name == "public"
        || name == "admin"
    {
        return false;
    }
    for ch in name.chars() {
        match ch {
            '0'..='9' | 'a'..='z' | '_' => {}
            _ => return false,
        }
    }
    true
}

fn valid_view_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.starts_with('_') {
        return false;
    }
    for ch in name.chars() {
        match ch {
            '0'..='9' | 'a'..='z' | '_' => {}
            _ => return false,
        }
    }
    true
}

fn valid_column_name(name: &str) -> bool {
    valid_view_name(name)
}

#[derive(Serialize)]
struct Empty {}

#[post("/api/admin/sys/app")]
async fn admin_sys_app_new(
    payload: web::Json<NewAppRequest>,
    data: web::Data<AppState>,
) -> actix_web::Result<web::Json<Empty>, MyError> {
    for app in &payload.into_inner().data {
        if !valid_app_name(&app.app) {
            return Err(MyError::InvalidAppName);
        }
        let sql = format!("CREATE SCHEMA {}", identifier(&app.app)?);
        data.client.query(&sql as &str, &[]).await?;
    }
    Ok(web::Json(Empty {}))
}

#[derive(Deserialize)]
struct JustApp {
    app: String,
}

#[derive(Deserialize)]
struct AppAndView {
    app: String,
    view: String,
}

#[delete("/api/admin/sys/view")]
async fn admin_sys_view_del(
    web::Query(query): web::Query<AppAndView>,
    data: web::Data<AppState>,
) -> actix_web::Result<HttpResponse, MyError> {
    if !valid_app_name(&query.app) {
        return Err(MyError::InvalidAppName);
    }
    if !valid_view_name(&query.view) {
        return Err(MyError::InvalidViewName);
    }
    let sql = format!(
        "DROP VIEW {}.{} RESTRICT",
        identifier(&query.app)?,
        identifier(&query.view)?
    );
    if data.client.query(&sql as &str, &[]).await.is_err() {
        // Then it's probably a table.
        let sql = format!(
            "DROP TABLE {}.{} RESTRICT",
            identifier(&query.app)?,
            identifier(&query.view)?
        );
        data.client.query(&sql as &str, &[]).await?;
    }
    Ok(HttpResponse::NoContent().finish())
}

#[get("/api/admin/sys/view")]
async fn admin_sys_view(
    web::Query(query): web::Query<JustApp>,
    data: web::Data<AppState>,
) -> actix_web::Result<web::Json<Value>, MyError> {
    let rows = data
        .client
        .query(
            "
WITH a AS (
    SELECT
        information_schema.views.table_schema AS app,
        information_schema.views.table_name AS view,
        ARRAY_AGG(information_schema.view_table_usage.table_name :: VARCHAR) AS tables,
        'derivative' AS view_type
    FROM information_schema.views
    LEFT OUTER JOIN information_schema.view_table_usage
        ON information_schema.views.table_name = information_schema.view_table_usage.view_name
    WHERE information_schema.views.table_schema = $1
    GROUP BY information_schema.views.table_schema, information_schema.views.table_name

    UNION SELECT
        information_schema.tables.table_schema AS app,
        information_schema.tables.table_name AS view,
        ARRAY[information_schema.tables.table_name::VARCHAR] AS tables,
        'table' AS view_type
    FROM information_schema.tables
    WHERE information_schema.tables.table_type = 'BASE TABLE'
    AND information_schema.tables.table_schema = $1
    ORDER BY app ASC, tables ASC, view ASC
), b AS (
    SELECT
        information_schema.columns.table_schema AS app,
        information_schema.columns.table_name AS view,
        information_schema.columns.column_name AS column,
        CASE information_schema.columns.data_type
            WHEN 'character varying' THEN 'string'
            ELSE 'unknown'
        END AS data_type
    FROM information_schema.columns
    WHERE information_schema.columns.table_schema = $1
    ORDER BY table_name ASC, ordinal_position ASC
)
SELECT
    a.app,
    a.view,
    a.tables,
    a.view_type,
    ARRAY_REMOVE(ARRAY_AGG(b.column::VARCHAR),NULL) AS columns,
    ARRAY_REMOVE(ARRAY_AGG(b.data_type::VARCHAR),NULL) AS data_types
FROM a
LEFT OUTER JOIN b
    ON a.app = b.app AND a.view = b.view
GROUP BY a.app, a.view, a.tables, a.view_type
ORDER BY app ASC, tables ASC, view DESC
",
            &[&query.app],
        )
        .await?;
    Ok(web::Json(rows_to_json(&rows)?))
}

#[derive(Deserialize)]
struct PatchViewRequest {
    patch: PatchViewPatch,
}
#[derive(Deserialize)]
struct PatchViewPatch {
    add_columns: Vec<PatchViewAddColumn>,
}
#[derive(Deserialize)]
struct PatchViewAddColumn {
    column: String,
    data_type: String,
}

fn data_type(dt: &str) -> Result<String, MyError> {
    match dt {
        "string" => Ok("character varying".to_owned()),
        _ => Err(MyError::InvalidType),
    }
}

#[patch("/api/admin/sys/view")]
async fn admin_sys_view_patch(
    web::Query(query): web::Query<AppAndView>,
    web::Json(payload): web::Json<PatchViewRequest>,
    data: web::Data<AppState>,
) -> actix_web::Result<web::Json<Empty>, MyError> {
    for add_column in payload.patch.add_columns.into_iter() {
        if !valid_column_name(&add_column.column) {
            return Err(MyError::InvalidColumnName);
        }
        let sql = format!(
            "ALTER TABLE {}.{} ADD {} {}",
            identifier(&query.app)?,
            identifier(&query.view)?,
            identifier(&add_column.column)?,
            data_type(&add_column.data_type)?
        );
        data.client.query(&sql as &str, &[]).await?;
    }
    Ok(web::Json(Empty {}))
}

#[derive(Deserialize)]
struct NewViewRequest {
    data: Vec<NewViewData>,
}
#[derive(Deserialize)]
struct NewViewData {
    view: String,
    prev: Option<String>,
}

#[post("/api/admin/sys/view")]
async fn admin_sys_view_new(
    web::Query(query): web::Query<JustApp>,
    web::Json(payload): web::Json<NewViewRequest>,
    data: web::Data<AppState>,
) -> actix_web::Result<web::Json<Empty>, MyError> {
    if !valid_app_name(&query.app) {
        return Err(MyError::InvalidAppName);
    }
    for view in payload.data.into_iter() {
        if !valid_view_name(&view.view) {
            return Err(MyError::InvalidViewName);
        }
        if let Some(prev) = view.prev {
            if !valid_view_name(&prev) {
                return Err(MyError::InvalidViewName);
            }

            // Try creating the view first because this may fail
            let sql = format!(
                "CREATE VIEW {}.\"_temp\" AS SELECT * FROM {}.{}",
                identifier(&query.app)?,
                identifier(&query.app)?,
                identifier(&prev)?
            );
            data.client.query(&sql as &str, &[]).await?;

            let sql = format!(
                "ALTER TABLE {}.{} RENAME TO {}",
                identifier(&query.app)?,
                identifier(&prev)?,
                identifier(&view.view)?
            );
            data.client.query(&sql as &str, &[]).await?;

            let sql = format!(
                "ALTER VIEW {}.\"_temp\" RENAME TO {}",
                identifier(&query.app)?,
                identifier(&prev)?
            );
            data.client.query(&sql as &str, &[]).await?;
        } else {
            let sql = format!(
                "CREATE TABLE {}.{} ()",
                identifier(&query.app)?,
                identifier(&view.view)?
            );
            data.client.query(&sql as &str, &[]).await?;
        }
    }

    Ok(web::Json(Empty {}))
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

async fn connect(conf: &PostgresConf) -> std::io::Result<(Client, Connection<Socket, NoTlsStream>)> {
    let max_duration = Duration::from_secs(30);
    let mut duration = Duration::from_millis(10);
    loop {
        let mut config = Config::new();
        config.host(&conf.host).user(&conf.user).password(&conf.password);

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
    println!("Loading configuration");
    let conf = DbThingConf::load()?;
    println!("Connecting");
    let (client, connection) = connect(&conf.postgres).await?;

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
            .service(admin_sys_app_del)
            .service(admin_sys_app_new)
            .service(admin_sys_view)
            .service(admin_sys_view_del)
            .service(admin_sys_view_new)
            .service(admin_sys_view_patch)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
