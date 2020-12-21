use actix_web::error;
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum MyError {
    InvalidAppName,
    InvalidColumnName,
    InvalidType,
    InvalidViewName,
    NullInSqlToken,
    Postgres(tokio_postgres::error::Error),
}

impl error::ResponseError for MyError {}

impl From<tokio_postgres::error::Error> for MyError {
    fn from(e: tokio_postgres::error::Error) -> Self {
        MyError::Postgres(e)
    }
}
