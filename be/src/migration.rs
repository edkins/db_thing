use actix_web::web;
use serde_json::{value::Map, Value};
use tokio_postgres::Client;

use crate::errors::MyError;

pub async fn advance(_client: &Client) -> actix_web::Result<web::Json<Value>, MyError> {
/*    client.query("CREATE SCHEMA admin", &[]).await?;
    client
        .query(
            "
CREATE VIEW admin.view(app, view)
AS SELECT table_schema AS app, table_name AS view
FROM information_schema.views
WHERE table_schema <> 'pg_catalog'
AND table_schema <> 'information_schema'
",
            &[],
        )
        .await?;
    client
        .query(
            "
CREATE VIEW admin.view_table_usage(app, view, \"table\", \"type\")
AS SELECT view_schema AS app, view_name AS view, table_name AS \"table\", 'table' AS \"type\"
FROM information_schema.view_table_usage
WHERE view_schema <> 'pg_catalog' AND view_schema <> 'information_schema'
AND view_schema = table_schema
UNION SELECT view_schema AS app, view_name AS view, 'system' AS \"table\", 'system' AS \"type\"
FROM information_schema.view_table_usage
WHERE view_schema <> 'pg_catalog' AND view_schema <> 'information_schema'
AND (table_schema = 'pg_catalog' OR table_schema = 'information_schema')
GROUP BY view_schema, view_name
",
            &[],
        )
        .await?;*/
    Ok(web::Json(Value::Object(Map::new())))
}

pub async fn retract(_client: &Client) -> actix_web::Result<web::Json<Value>, MyError> {
    /*client.query("DROP VIEW IF EXISTS admin.view", &[]).await?;
    client
        .query("DROP VIEW IF EXISTS admin.view_table_usage", &[])
        .await?;
    client.query("DROP SCHEMA IF EXISTS admin", &[]).await?;*/
    Ok(web::Json(Value::Object(Map::new())))
}
