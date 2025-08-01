use log::{error, info};
use serde_json::Value;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::fs;
use std::time::Instant;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::oneshot;

#[derive(Clone, serde::Serialize)]
struct OperationResult<T> {
    data: T,
    duration_ms: u128,
}

#[derive(Clone, serde::Serialize)]
struct EmptyValueResult {
    index: usize,
    key: String,
}

#[derive(Clone, serde::Serialize)]
struct InvalidNumericResult {
    index: usize,
    key: String,
    value: String,
}

// A custom error type for our commands
#[derive(Debug, thiserror::Error)]
enum CommandError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("SQL parsing error: {0}")]
    Sql(String),
    #[error("{0}")]
    Generic(String),
}

// We must implement serde::Serialize on the error enum
impl serde::Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

type CommandResult<T> = Result<T, CommandError>;

#[tauri::command]
async fn open_and_read_json_file(app: tauri::AppHandle) -> CommandResult<String> {
    let start = Instant::now();
    info!("Attempting to open a JSON file.");

    let (tx, rx) = oneshot::channel::<Option<FilePath>>();

    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .pick_file(move |file_path| {
            let _ = tx.send(file_path);
        });

    let file_path = rx.await.map_err(|e| CommandError::Generic(e.to_string()))?;

    if let Some(file) = file_path {
        let path = std::path::PathBuf::from(file.to_string());
        info!("User selected file: {:?}", &path);
        let content = fs::read_to_string(&path)?;
        let duration = start.elapsed();
        info!("File read successfully in {}ms.", duration.as_millis());
        info!(
            "Logging file content hash (first 1KB) for reference: {:?}",
            &content.as_bytes()[..std::cmp::min(1024, content.len())]
        );
        Ok(content)
    } else {
        info!("User cancelled file dialog.");
        Err(CommandError::Generic("File selection was canceled.".into()))
    }
}

#[tauri::command]
fn find_empty_values(json_str: String) -> CommandResult<OperationResult<Vec<EmptyValueResult>>> {
    let start = Instant::now();
    info!("Starting search for empty values.");

    let v: Value = serde_json::from_str(&json_str)?;
    let mut empty_results = Vec::new();

    if let Some(arr) = v.as_array() {
        for (i, obj) in arr.iter().enumerate() {
            if let Some(map) = obj.as_object() {
                for (key, value) in map.iter() {
                    if value.is_null() || (value.is_string() && value.as_str().unwrap().is_empty())
                    {
                        empty_results.push(EmptyValueResult {
                            index: i,
                            key: key.clone(),
                        });
                    }
                }
            }
        }
    }

    let duration = start.elapsed();
    info!(
        "Found {} empty values in {}ms.",
        empty_results.len(),
        duration.as_millis()
    );
    Ok(OperationResult {
        data: empty_results,
        duration_ms: duration.as_millis(),
    })
}

#[tauri::command]
fn find_invalid_numeric_values(
    json_str: String,
    sql_str: String,
) -> CommandResult<OperationResult<Vec<InvalidNumericResult>>> {
    let start = Instant::now();
    info!("Starting search for invalid numeric values.");

    let dialect = GenericDialect {};
    let ast =
        Parser::parse_sql(&dialect, &sql_str).map_err(|e| CommandError::Sql(e.to_string()))?;

    let mut numeric_columns = std::collections::HashSet::new();
    if let Some(sqlparser::ast::Statement::CreateTable(sqlparser::ast::CreateTable {
        columns,
        ..
    })) = ast.get(0)
    {
        for col in columns {
            let data_type_str = col.data_type.to_string().to_lowercase();
            if data_type_str.contains("int")
                || data_type_str.contains("numeric")
                || data_type_str.contains("decimal")
                || data_type_str.contains("float")
                || data_type_str.contains("double")
            {
                numeric_columns.insert(col.name.value.clone());
            }
        }
    } else {
        return Err(CommandError::Sql(
            "Could not parse a CREATE TABLE statement.".into(),
        ));
    }
    info!("Identified numeric columns from SQL: {:?}", numeric_columns);

    let v: Value = serde_json::from_str(&json_str)?;
    let mut invalid_results = Vec::new();

    if let Some(arr) = v.as_array() {
        for (i, obj) in arr.iter().enumerate() {
            if let Some(map) = obj.as_object() {
                for (key, value) in map.iter() {
                    if numeric_columns.contains(key) {
                        if !value.is_number() {
                            if let Some(s) = value.as_str() {
                                if s.parse::<f64>().is_err() {
                                    invalid_results.push(InvalidNumericResult {
                                        index: i,
                                        key: key.clone(),
                                        value: s.to_string(),
                                    });
                                }
                            } else {
                                invalid_results.push(InvalidNumericResult {
                                    index: i,
                                    key: key.clone(),
                                    value: value.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    let duration = start.elapsed();
    info!(
        "Found {} invalid numeric values in {}ms.",
        invalid_results.len(),
        duration.as_millis()
    );
    Ok(OperationResult {
        data: invalid_results,
        duration_ms: duration.as_millis(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri_plugin_log::{Target, TargetKind};

    let log_plugin = tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::LogDir {
                file_name: Some("app".into()),
            }),
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Webview),
        ])
        .level(log::LevelFilter::Info)
        .build();

    tauri::Builder::default()
        .plugin(log_plugin)
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            open_and_read_json_file,
            find_empty_values,
            find_invalid_numeric_values
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
