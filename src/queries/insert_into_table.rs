// use super::{
//     postgres_types::{convert_row_fields, ColumnTypeValue, RowFields},
//     query_types::{Query, QueryParams, QueryParamsInsert, QueryResult},
//     table_stats::select_column_stats,
// };
// use crate::db::PgConnection;
// use crate::errors::ApiError;
// use futures::future;
// use tokio_postgres::TransactionBuilder;
// use tokio_postgres::types::ToSql;
// use serde_json::{Map, Value};
// use std::collections::HashMap;

// static INSERT_ROWS_BATCH_COUNT: usize = 2;

// enum InsertResult {
//     Rows(Vec<RowFields>),
//     NumRowsAffected(u64),
// }

// /// Runs an INSERT INTO <table> query
// pub fn insert_into_table(conn: &PgConnection, query: Query) -> Result<QueryResult, ApiError> {
//     // extract query data
//     let mut query_params: QueryParamsInsert;
//     match query.params {
//         QueryParams::Insert(insert_params) => query_params = insert_params,
//         _ => unreachable!("insert_into_table() should not be called without Insert parameter."),
//     };

//     // serde_json::Values can't automatically convert to non-JSON/JSONB columns.
//     // Therefore, get column types of table so we know what types into which the json values are converted.
//     // apparently rust_postgres already does this in the background, would be nice if there was a way to hook into existing functionality...
//     let mut column_types: HashMap<String, String> = HashMap::new();
//     for stat in select_column_stats(conn, &query_params.table)?.into_iter() {
//         column_types.insert(stat.column_name, stat.column_type);
//     }

//     let num_rows = query_params.rows.len();
//     let mut total_num_rows_affected = 0;
//     let mut total_rows_returned = vec![];

//     // let transaction = conn.client.build_transaction().build()

//     // currently, we use 1 database transaction to wrap all inserts
//     // using tokio-postgres transaction builder, we need to:
//     // 1. lazily build all the insert executions in a lazy future (so they don't run right when we call them)
//     // 2. wrap them in a transaction builder so it runs all at once inside a transaction

//     if num_rows >= INSERT_ROWS_BATCH_COUNT {
//         // batch inserts into groups of 100 (see https://www.depesz.com/2007/07/05/how-to-insert-data-to-database-as-fast-as-possible/)

//         let batch_insert = future::lazy(|| {});

//         let mut batch_rows = vec![];
//         for (i, row) in query_params.rows.iter().enumerate() {
//             batch_rows.push(row);

//             if (i + 1) % INSERT_ROWS_BATCH_COUNT == 0 || i == num_rows - 1 {
//                 // do batch inserts on pushed rows
//                 match execute_insert(&conn, &query_params, &column_types, &batch_rows)? {
//                     InsertResult::NumRowsAffected(num_rows_affected) => {
//                         total_num_rows_affected += num_rows_affected
//                     }
//                     InsertResult::Rows(rows) => {
//                         total_rows_returned.extend(rows);
//                     }
//                 };

//                 // reset batch
//                 batch_rows.truncate(0);
//             }
//         }
//     } else {
//         // insert all rows
//         match execute_insert(
//             &conn,
//             &query_params,
//             &column_types,
//             &query_params
//                 .rows
//                 .iter()
//                 .collect::<Vec<&Map<String, Value>>>(),
//         )? {
//             InsertResult::NumRowsAffected(num_rows_affected) => {
//                 total_num_rows_affected += num_rows_affected
//             }
//             InsertResult::Rows(rows) => {
//                 total_rows_returned.extend(rows);
//             }
//         };
//     }

//     transaction.commit()?;

//     if query_params.returning_columns.is_some() {
//         Ok(QueryResult::QueryTableResult(total_rows_returned))
//     } else {
//         Ok(QueryResult::from_num_rows_affected(total_num_rows_affected))
//     }
// }

// /// Runs the actual setting up + execution of the INSERT query
// fn execute_insert<'a>(
//     conn: &PgConnection,
//     query_params: &QueryParamsInsert,
//     column_types: &HashMap<String, String>,
//     rows: &'a [&'a Map<String, Value>],
// ) -> Result<InsertResult, ApiError> {
//     let mut is_return_rows = false;

//     // parse out the columns that have values to assign
//     let columns = get_all_columns_to_insert(rows);
//     let (values_params_str, column_values) = get_insert_params(rows, &columns, column_types)?;

//     // generate the ON CONFLICT string
//     let conflict_clause = match generate_conflict_str(query_params, &columns) {
//         Some(conflict_str) => conflict_str,
//         None => "".to_string(),
//     };

//     // generate the RETURNING string
//     let returning_clause = match generate_returning_clause(query_params) {
//         Some(returning_str) => {
//             is_return_rows = true;
//             returning_str
//         }
//         None => "".to_string(),
//     };

//     // create initial prepared statement
//     let insert_query_str = [
//         "INSERT INTO ",
//         &query_params.table,
//         &[" (", &columns.join(", "), ")"].join(""),
//         " VALUES ",
//         &values_params_str,
//         &conflict_clause,
//         &returning_clause,
//     ]
//     .join("");

//     let prep_statement = conn.client.prepare(&insert_query_str)?;

//     // convert the column values into the actual values we will use for the INSERT statement execution
//     let mut prep_values: Vec<&ToSql> = vec![];
//     for column_value in column_values.iter() {
//         match column_value {
//             ColumnTypeValue::BigInt(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Bool(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::ByteA(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Char(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Citext(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Date(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Decimal(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Float8(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::HStore(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Int(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Json(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::JsonB(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::MacAddr(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Name(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Oid(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Real(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::SmallInt(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Text(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Time(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Timestamp(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::TimestampTz(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::Uuid(col_val) => prep_values.push(col_val),
//             ColumnTypeValue::VarChar(col_val) => prep_values.push(col_val),
//         };
//     }

//     // execute sql & return results
//     if is_return_rows {
//         let results: Vec<RowFields> = prep_statement
//             .query(&prep_values)?
//             .iter()
//             .map(|row| convert_row_fields(&row))
//             .collect::<Result<Vec<RowFields>, ApiError>>()?;
//         Ok(InsertResult::Rows(results))
//     } else {
//         let results = prep_statement.execute(&prep_values)?;
//         Ok(InsertResult::NumRowsAffected(results))
//     }
// }

// /// Generates the ON CONFLICT clause. If conflict action is "nothing", then "DO NOTHING" is returned. If conflict action is "update", then sets all columns that aren't conflict target columns to the excluded row's column value.
// fn generate_conflict_str(query_params: &QueryParamsInsert, columns: &[&str]) -> Option<String> {
//     if let (Some(conflict_action_str), Some(conflict_target_vec)) =
//         (&query_params.conflict_action, &query_params.conflict_target)
//     {
//         // filter out any conflict target columns and convert the remaining columns into "SET <col> = EXCLUDED.<col>" clauses
//         let expanded_conflict_action = if conflict_action_str == "update" {
//             [
//                 "DO UPDATE SET ",
//                 &columns
//                     .iter()
//                     .filter_map(|col| {
//                         match conflict_target_vec
//                             .iter()
//                             .position(|conflict_target| conflict_target == *col)
//                         {
//                             Some(_) => None,
//                             None => Some([*col, "=", "EXCLUDED.", *col].join("")),
//                         }
//                     })
//                     .collect::<Vec<String>>()
//                     .join(", "),
//             ]
//             .join("")
//         } else {
//             "DO NOTHING".to_string()
//         };

//         return Some(
//             [
//                 " ON CONFLICT (",
//                 &conflict_target_vec.join(", "),
//                 ") ",
//                 &expanded_conflict_action,
//             ]
//             .join(""),
//         );
//     }

//     None
// }

// fn generate_returning_clause(query_params: &QueryParamsInsert) -> Option<String> {
//     if let Some(returning_columns) = &query_params.returning_columns {
//         return Some([" RETURNING ", &returning_columns.join(", ")].join(""));
//     }

//     None
// }

// /// Searches all rows being inserted and returns a vector containing all of the column names
// fn get_all_columns_to_insert<'a>(rows: &'a [&'a Map<String, Value>]) -> Vec<&'a str> {
//     // parse out the columns that have values to assign
//     let mut columns: Vec<&str> = vec![];
//     for row in rows.iter() {
//         for column in row.keys() {
//             if columns.iter().position(|&c| c == column).is_none() {
//                 columns.push(column);
//             }
//         }
//     }
//     columns
// }

// /// Returns a Result containing the tuple that contains (the VALUES parameter string, the array of parameter values)
// fn get_insert_params(
//     rows: &[&Map<String, Value>],
//     columns: &[&str],
//     column_types: &HashMap<String, String>,
// ) -> Result<(String, Vec<ColumnTypeValue>), ApiError> {
//     let mut prep_column_number = 1;
//     let mut row_strs = vec![];

//     // generate the array of json-converted-to-rust_postgres values to insert.
//     let nested_column_values_result: Result<Vec<Vec<ColumnTypeValue>>, ApiError> = rows
//         .iter()
//         .map(|row| -> Result<Vec<ColumnTypeValue>, ApiError> {
//             // row_str_arr is used for the prepared statement parameter string
//             let mut row_str_arr: Vec<String> = vec![];
//             let mut column_values: Vec<ColumnTypeValue> = vec![];

//             for column in columns.iter() {
//                 // if the "row" json object has a value for column, then use the rust-converted value, otherwise use the column’s DEFAULT value
//                 match row.get(*column) {
//                     Some(val) => {
//                         let prep_column_number_str =
//                             ["$", &prep_column_number.to_string()].join("");
//                         row_str_arr.push(prep_column_number_str);
//                         prep_column_number += 1;

//                         let column_type = &column_types[*column];
//                         match ColumnTypeValue::from_json(column_type, val) {
//                             Ok(column_type_value) => {
//                                 column_values.push(column_type_value);
//                             }
//                             Err(e) => return Err(e),
//                         }
//                     }
//                     None => {
//                         row_str_arr.push("DEFAULT".to_string());
//                     }
//                 };
//             }

//             row_strs.push(format!("({})", row_str_arr.join(", ")));
//             Ok(column_values)
//         })
//         .collect();

//     let values_str = row_strs.join(", ");

//     let nested_column_values = nested_column_values_result?;
//     let column_values: Vec<ColumnTypeValue> = nested_column_values.into_iter().flatten().collect();

//     Ok((values_str, column_values))
// }
