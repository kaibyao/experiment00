use super::{postgres_types::RowFields, utils::normalize_columns};
use crate::Error;
use actix_web::HttpRequest;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Deserialize)]
/// All possible query string parameters in an API request
pub struct RequestQueryStringParams {
    /// Comma-separated list of column names for which values are retrieved.
    pub columns: Option<String>,
    /// This param is required in order for DELETE operation to process.
    pub confirm_delete: Option<String>,
    /// The `ON CONFLICT` action to perform (`update` or `nothing`) for POSTing to the table
    /// endpoint (to insert new rows).
    pub conflict_action: Option<String>,
    /// Used in conjunction with `conflict_action`. Comma-separated list of columns that determine
    /// if a row being inserted conflicts with an existing row.
    pub conflict_target: Option<String>,
    /// A comma-separated list of column names for which rows that have duplicate values are
    /// excluded (in a GET/SELECT statement).
    pub distinct: Option<String>,
    /// The FROM clause of an UPDATE statement. Comma-separated list of columns. Does not accept
    /// sub-queries (use /sql endpoint if more advanced expressions are needed).
    pub from: Option<String>,
    /// The WHERE clause of the SQL statement. Remember to URI-encode the final result.
    pub r#where: Option<String>,
    /// Comma-separated list representing the field(s) on which to group the resulting rows (in a
    /// GET/SELECT statement).
    pub group_by: Option<String>,
    /// Comma-separated list representing the field(s) on which to sort the resulting rows (in a
    /// GET/SELECT statement).
    pub order_by: Option<String>,
    /// The maximum number of rows that can be returned (in a GET/SELECT statement).
    pub limit: Option<usize>,
    /// The number of rows to exclude (in a GET/SELECT statement).
    pub offset: Option<usize>,
    /// Comma-separated list of columns to return from the POST/INSERT operation.
    pub returning_columns: Option<String>,
}

#[derive(Debug)]
/// Represents a single SELECT query
pub struct QueryParamsSelect {
    pub distinct: Option<Vec<String>>,
    pub columns: Vec<String>,
    pub table: String,
    pub conditions: Option<String>,
    pub group_by: Option<Vec<String>>,
    pub order_by: Option<Vec<String>>,
    pub limit: usize,
    pub offset: usize,
}

impl QueryParamsSelect {
    /// Fills the struct’s values based on the HttpRequest data.
    pub fn from_http_request(
        req: &HttpRequest,
        query_string_params: RequestQueryStringParams,
    ) -> Result<Self, Error> {
        let default_limit = 10000;
        let default_offset = 0;

        let params = QueryParamsSelect {
            columns: match query_string_params.columns {
                Some(columns_str) => normalize_columns(&columns_str)?,
                None => vec![],
            },
            distinct: match query_string_params.distinct {
                Some(distinct_str) => Some(normalize_columns(&distinct_str)?),
                None => None,
            },
            table: req.match_info().query("table").to_lowercase(),
            conditions: match query_string_params.r#where {
                Some(where_string) => Some(where_string.trim().to_lowercase()),
                None => None,
            },
            group_by: match query_string_params.group_by {
                Some(group_by_str) => Some(normalize_columns(&group_by_str)?),
                None => None,
            },
            order_by: match query_string_params.order_by {
                Some(order_by_str) => Some(normalize_columns(&order_by_str)?),
                None => None,
            },
            limit: match query_string_params.limit {
                Some(limit) => limit,
                None => default_limit,
            },
            offset: match query_string_params.offset {
                Some(offset) => offset,
                None => default_offset,
            },
        };

        Ok(params)
    }
}

#[derive(Debug)]
/// Represents a single INSERT query
pub struct QueryParamsInsert {
    pub conflict_action: Option<String>,
    pub conflict_target: Option<Vec<String>>,
    pub returning_columns: Option<Vec<String>>,
    pub rows: Vec<Map<String, Value>>,
    pub table: String,
}

impl QueryParamsInsert {
    /// Fills the struct’s values based on the HttpRequest data.
    pub fn from_http_request(
        req: &HttpRequest,
        body: Value,
        query_string_params: RequestQueryStringParams,
    ) -> Result<Self, Error> {
        // generate ON CONFLICT data
        let conflict_action = match query_string_params.conflict_action {
            Some(action_str) => Some(action_str.to_string().to_lowercase()),
            None => None,
        };
        let conflict_target: Option<Vec<String>> = match query_string_params.conflict_target {
            Some(targets_str) => Some(normalize_columns(&targets_str)?),
            None => None,
        };
        if (conflict_action.is_some() && conflict_target.is_none())
            || (conflict_action.is_none() && conflict_target.is_some())
        {
            return Err(Error::generate_error("INCORRECT_REQUEST_BODY", "`conflict_action` and `conflict_target` must both be present for the `ON CONFLICT` clause to be generated correctly.".to_string()));
        }

        if let (Some(conflict_action_str), Some(conflict_target_vec)) =
            (&conflict_action, &conflict_target)
        {
            // Some validation checking of conflict_action and conflict_target
            if conflict_action_str != "nothing" && conflict_action_str != "update" {
                return Err(Error::generate_error(
                    "INCORRECT_REQUEST_BODY",
                    "Valid options for `conflict_action` are: `nothing`, `update`.".to_string(),
                ));
            }

            if conflict_target_vec.is_empty() {
                return Err(Error::generate_error(
                    "INCORRECT_REQUEST_BODY",
                    "`conflict_target` must be a comma-separated list of column names and include at least one column name.".to_string(),
                ));
            }

            if conflict_target_vec
                .iter()
                .any(|conflict_target_str| *conflict_target_str == "")
            {
                return Err(Error::generate_error(
                    "INCORRECT_REQUEST_BODY",
                    "<Empty string> is not a valid column name for the parameter`conflict_target`."
                        .to_string(),
                ));
            }
        }

        // generate RETURNING data
        let returning_columns = match query_string_params.returning_columns {
            Some(columns_str) => {
                if columns_str == "" {
                    return Err(Error::generate_error(
                        "INCORRECT_REQUEST_BODY",
                        "`returning_columns` must be a comma-separated list of column names and include at least one column name.".to_string(),
                    ));
                }

                let returning_columns_vec = normalize_columns(&columns_str)?;
                Some(returning_columns_vec)
            }
            None => None,
        };

        let rows: Vec<Map<String, Value>> = match body.as_array() {
            Some(body_rows_to_insert) => {
                if !body_rows_to_insert
                .iter().all(Value::is_object) {
                    return Err(Error::generate_error("INCORRECT_REQUEST_BODY", "The body needs to be an array of objects where each object represents a row and whose key-values represent column names and their values.".to_string()));
                }

                body_rows_to_insert
                .iter().map(|json_value| {
                    if let Some(row_obj_map) = json_value.as_object() {
                        row_obj_map.clone()
                    } else {
                        unreachable!("Taken care of via above conditional.")
                    }
                })
                .collect()
            },
            None => return Err(Error::generate_error("INCORRECT_REQUEST_BODY", "The body needs to be an array of objects where each object represents a row and whose key-values represent column names and their values.".to_string())),
        };

        Ok(QueryParamsInsert {
            conflict_action,
            conflict_target,
            returning_columns,
            rows,
            table: req.match_info().query("table").to_lowercase(),
        })
    }
}

#[derive(Debug)]
/// Parameters used to generate an `UPDATE` SQL statement.
pub struct QueryParamsUpdate {
    /// A JSON object whose key-values represent column names and the values to set.
    pub column_values: Map<String, Value>,
    /// WHERE expression.
    pub conditions: Option<String>,
    /// List of (foreign key) columns whose values are returned.
    pub returning_columns: Option<Vec<String>>,
    // Name of table to update.
    pub table: String,
}

impl QueryParamsUpdate {
    pub fn from_http_request(
        req: &HttpRequest,
        body: Value,
        query_string_params: RequestQueryStringParams,
    ) -> Result<Self, Error> {
        let column_values = match body.as_object() {
            Some(column_values) => column_values.clone(),
            None => return Err(Error::generate_error(
                "INCORRECT_REQUEST_BODY",
                "Request body must be a JSON object whose key-values represent column names and the values to set. String values must contain quotes or else they will be evaluated as expressions and not strings.".to_string(),
            ))
        };
        let returning_columns = match query_string_params.returning_columns {
            Some(columns_str) => {
                if columns_str == "" {
                    return Err(Error::generate_error(
                        "INCORRECT_REQUEST_BODY",
                        "`returning_columns` must be a comma-separated list of column names and include at least one column name.".to_string(),
                    ));
                }

                let returning_columns_vec = normalize_columns(&columns_str)?;
                Some(returning_columns_vec)
            }
            None => None,
        };
        let table = req.match_info().query("table").to_lowercase();
        let conditions = match query_string_params.r#where {
            Some(where_string) => Some(where_string.trim().to_lowercase()),
            None => None,
        };

        Ok(QueryParamsUpdate {
            column_values,
            conditions,
            returning_columns,
            table,
        })
    }
}

#[derive(Serialize)]
pub struct RowsAffectedQueryResult {
    num_rows: u64,
}

#[derive(Serialize)]
#[serde(untagged)]
/// Represents the response from sending a QueryTask to DbExecutor
pub enum QueryResult {
    QueryTableResult(Vec<RowFields>),
    RowsAffected(RowsAffectedQueryResult),
}

impl QueryResult {
    pub fn from_num_rows_affected(num_rows: u64) -> Self {
        QueryResult::RowsAffected(RowsAffectedQueryResult { num_rows })
    }
}
