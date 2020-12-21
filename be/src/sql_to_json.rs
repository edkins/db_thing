use serde_json::Value;
use std::error::Error;
use tokio_postgres::types::{FromSql, Type};

pub struct SqlJson(pub Value);

fn opt_to_string(o: Option<String>) -> Value {
    match o {
        None => Value::Null,
        Some(s) => Value::String(s),
    }
}

impl<'a> FromSql<'a> for SqlJson {
    fn accepts(ty: &Type) -> bool {
        matches!(
            *ty,
            Type::BOOL
                | Type::CHAR
                | Type::INT2
                | Type::INT4
                | Type::INT8
                | Type::FLOAT4
                | Type::FLOAT8
                | Type::VARCHAR
                | Type::TEXT
                | Type::NAME
                | Type::VARCHAR_ARRAY
        )
    }

    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + 'static + Sync + Send>> {
        match *ty {
            Type::BOOL => Ok(SqlJson(Value::Bool(bool::from_sql(ty, raw)?))),
            Type::CHAR => Ok(SqlJson(Value::Number(i8::from_sql(ty, raw)?.into()))),
            Type::INT2 => Ok(SqlJson(Value::Number(i16::from_sql(ty, raw)?.into()))),
            Type::INT4 => Ok(SqlJson(Value::Number(i32::from_sql(ty, raw)?.into()))),
            Type::INT8 => Ok(SqlJson(Value::Number(i64::from_sql(ty, raw)?.into()))),
            Type::VARCHAR | Type::TEXT | Type::NAME => {
                Ok(SqlJson(Value::String(String::from_sql(ty, raw)?)))
            }
            Type::VARCHAR_ARRAY => Ok(SqlJson(Value::Array(
                Vec::<Option<String>>::from_sql(ty, raw)?
                    .into_iter()
                    .map(opt_to_string)
                    .collect(),
            ))),
            _ => panic!(),
        }
    }

    fn from_sql_null(_ty: &Type) -> Result<Self, Box<dyn Error + 'static + Sync + Send>> {
        Ok(SqlJson(Value::Null))
    }

    fn from_sql_nullable(
        ty: &Type,
        raw: Option<&'a [u8]>,
    ) -> Result<Self, Box<dyn Error + 'static + Sync + Send>> {
        if let Some(raw) = raw {
            Self::from_sql(ty, raw)
        } else {
            Self::from_sql_null(ty)
        }
    }
}
