use actix_web::web;
use serde_json::{Value, value::Map};
use tokio_postgres::Client;

use crate::errors::MyError;

pub async fn advance(client: &Client) -> actix_web::Result<web::Json<Value>, MyError> {
    client.query("CREATE SCHEMA admin", &[]).await?;
    client.query("
CREATE VIEW admin.view(app, view)
AS SELECT schemaname AS app, viewname AS view
FROM pg_views
WHERE schemaname <> 'pg_catalog'
AND schemaname <> 'information_schema';
", &[]).await?;
    Ok(web::Json(Value::Object(Map::new())))
}

pub async fn retract(client: &Client) -> actix_web::Result<web::Json<Value>, MyError> {
    client.query("DROP VIEW IF EXISTS admin.view;", &[]).await?;
    client.query("DROP SCHEMA IF EXISTS admin;", &[]).await?;
    Ok(web::Json(Value::Object(Map::new())))
}
