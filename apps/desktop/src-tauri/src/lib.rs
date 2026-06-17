use chrono::Utc;
use lopdf::Document as PdfDocument;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
  fs,
  io::Read,
  path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRecord {
  id: String,
  title: String,
  file_name: String,
  stored_path: String,
  fingerprint: String,
  page_count: i64,
  status: String,
  created_at: String,
  updated_at: String,
  last_opened_page: i64,
  last_zoom: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfBytes {
  document_id: String,
  file_name: String,
  bytes: Vec<u8>,
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
  app
    .path()
    .app_data_dir()
    .map_err(|error| format!("无法定位应用数据目录：{error}"))
}

fn documents_dir(app: &AppHandle) -> Result<PathBuf, String> {
  Ok(app_data_dir(app)?.join("documents"))
}

fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
  Ok(app_data_dir(app)?.join("novum.sqlite3"))
}

fn ensure_storage(app: &AppHandle) -> Result<(), String> {
  fs::create_dir_all(documents_dir(app)?).map_err(|error| format!("无法创建文献目录：{error}"))?;
  let connection = open_connection(app)?;

  connection
    .execute_batch(
      "
      CREATE TABLE IF NOT EXISTS documents (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        file_name TEXT NOT NULL,
        stored_path TEXT NOT NULL,
        fingerprint TEXT NOT NULL UNIQUE,
        page_count INTEGER NOT NULL,
        status TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        last_opened_page INTEGER NOT NULL DEFAULT 1,
        last_zoom INTEGER NOT NULL DEFAULT 100
      );
      CREATE INDEX IF NOT EXISTS idx_documents_updated_at ON documents(updated_at DESC);
      ",
    )
    .map_err(|error| format!("无法初始化文献数据库：{error}"))?;

  Ok(())
}

fn open_connection(app: &AppHandle) -> Result<Connection, String> {
  let path = database_path(app)?;
  Connection::open(path).map_err(|error| format!("无法打开文献数据库：{error}"))
}

fn row_to_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentRecord> {
  Ok(DocumentRecord {
    id: row.get(0)?,
    title: row.get(1)?,
    file_name: row.get(2)?,
    stored_path: row.get(3)?,
    fingerprint: row.get(4)?,
    page_count: row.get(5)?,
    status: row.get(6)?,
    created_at: row.get(7)?,
    updated_at: row.get(8)?,
    last_opened_page: row.get(9)?,
    last_zoom: row.get(10)?,
  })
}

fn get_document_from_connection(
  connection: &Connection,
  id: &str,
) -> Result<DocumentRecord, String> {
  connection
    .query_row(
      "
      SELECT id, title, file_name, stored_path, fingerprint, page_count, status,
             created_at, updated_at, last_opened_page, last_zoom
      FROM documents
      WHERE id = ?1
      ",
      [id],
      row_to_document,
    )
    .optional()
    .map_err(|error| format!("读取文献记录失败：{error}"))?
    .ok_or_else(|| "找不到这篇文献。".to_string())
}

fn get_document_by_fingerprint(
  connection: &Connection,
  fingerprint: &str,
) -> Result<Option<DocumentRecord>, String> {
  connection
    .query_row(
      "
      SELECT id, title, file_name, stored_path, fingerprint, page_count, status,
             created_at, updated_at, last_opened_page, last_zoom
      FROM documents
      WHERE fingerprint = ?1
      ",
      [fingerprint],
      row_to_document,
    )
    .optional()
    .map_err(|error| format!("读取重复文献记录失败：{error}"))
}

fn sha256_file(path: &Path) -> Result<String, String> {
  let mut file = fs::File::open(path).map_err(|error| format!("无法打开 PDF：{error}"))?;
  let mut hasher = Sha256::new();
  let mut buffer = [0_u8; 64 * 1024];

  loop {
    let read = file
      .read(&mut buffer)
      .map_err(|error| format!("读取 PDF 失败：{error}"))?;
    if read == 0 {
      break;
    }
    hasher.update(&buffer[..read]);
  }

  Ok(format!("{:x}", hasher.finalize()))
}

fn title_from_path(path: &Path) -> String {
  path
    .file_stem()
    .and_then(|value| value.to_str())
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| "未命名文献".to_string())
}

fn validate_pdf_path(path: &Path) -> Result<(), String> {
  if !path.exists() {
    return Err("选择的文件不存在。".to_string());
  }
  if !path.is_file() {
    return Err("请选择一个 PDF 文件，而不是文件夹。".to_string());
  }
  let extension = path
    .extension()
    .and_then(|value| value.to_str())
    .unwrap_or_default()
    .to_lowercase();
  if extension != "pdf" {
    return Err("当前只支持导入 PDF 文件。".to_string());
  }
  Ok(())
}

fn count_pdf_pages(path: &Path) -> Result<i64, String> {
  let document = PdfDocument::load(path).map_err(|error| format!("无法解析 PDF：{error}"))?;
  Ok(document.get_pages().len().max(1) as i64)
}

#[tauri::command]
fn import_pdf_from_path(app: AppHandle, path: String) -> Result<DocumentRecord, String> {
  ensure_storage(&app)?;

  let source_path = PathBuf::from(path);
  validate_pdf_path(&source_path)?;

  let fingerprint = sha256_file(&source_path)?;
  let id = fingerprint
    .get(0..24)
    .ok_or_else(|| "无法生成文献 ID。".to_string())?
    .to_string();
  let file_name = source_path
    .file_name()
    .and_then(|value| value.to_str())
    .ok_or_else(|| "无法读取文件名。".to_string())?
    .to_string();

  let connection = open_connection(&app)?;
  if let Some(existing) = get_document_by_fingerprint(&connection, &fingerprint)? {
    return Ok(existing);
  }

  let page_count = count_pdf_pages(&source_path)?;
  let stored_path = documents_dir(&app)?.join(format!("{id}.pdf"));
  fs::copy(&source_path, &stored_path).map_err(|error| format!("复制 PDF 失败：{error}"))?;

  let now = Utc::now().to_rfc3339();
  let stored_path_string = stored_path.to_string_lossy().to_string();

  connection
    .execute(
      "
      INSERT INTO documents (
        id, title, file_name, stored_path, fingerprint, page_count, status,
        created_at, updated_at, last_opened_page, last_zoom
      )
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, 100)
      ",
      params![
        id,
        title_from_path(&source_path),
        file_name,
        stored_path_string,
        fingerprint,
        page_count,
        "已导入",
        now,
        now
      ],
    )
    .map_err(|error| format!("写入文献库失败：{error}"))?;

  get_document_from_connection(&connection, &id)
}

#[tauri::command]
fn list_documents(app: AppHandle) -> Result<Vec<DocumentRecord>, String> {
  ensure_storage(&app)?;
  let connection = open_connection(&app)?;
  let mut statement = connection
    .prepare(
      "
      SELECT id, title, file_name, stored_path, fingerprint, page_count, status,
             created_at, updated_at, last_opened_page, last_zoom
      FROM documents
      ORDER BY updated_at DESC
      ",
    )
    .map_err(|error| format!("读取文献库失败：{error}"))?;

  let rows = statement
    .query_map([], row_to_document)
    .map_err(|error| format!("读取文献列表失败：{error}"))?;

  let mut documents = Vec::new();
  for row in rows {
    documents.push(row.map_err(|error| format!("解析文献记录失败：{error}"))?);
  }
  Ok(documents)
}

#[tauri::command]
fn get_document(app: AppHandle, id: String) -> Result<DocumentRecord, String> {
  ensure_storage(&app)?;
  let connection = open_connection(&app)?;
  get_document_from_connection(&connection, &id)
}

#[tauri::command]
fn get_document_pdf_bytes(app: AppHandle, id: String) -> Result<PdfBytes, String> {
  ensure_storage(&app)?;
  let connection = open_connection(&app)?;
  let document = get_document_from_connection(&connection, &id)?;
  let path = PathBuf::from(&document.stored_path);

  let bytes = fs::read(path).map_err(|error| format!("读取 PDF 文件失败：{error}"))?;
  Ok(PdfBytes {
    document_id: document.id,
    file_name: document.file_name,
    bytes,
  })
}

#[tauri::command]
fn update_reading_state(
  app: AppHandle,
  id: String,
  page: i64,
  zoom: i64,
) -> Result<DocumentRecord, String> {
  ensure_storage(&app)?;
  let connection = open_connection(&app)?;
  let document = get_document_from_connection(&connection, &id)?;
  let safe_page = page.clamp(1, document.page_count.max(1));
  let safe_zoom = zoom.clamp(60, 180);
  let now = Utc::now().to_rfc3339();

  connection
    .execute(
      "
      UPDATE documents
      SET last_opened_page = ?1, last_zoom = ?2, updated_at = ?3
      WHERE id = ?4
      ",
      params![safe_page, safe_zoom, now, id],
    )
    .map_err(|error| format!("保存阅读状态失败：{error}"))?;

  get_document_from_connection(&connection, &document.id)
}

#[tauri::command]
fn delete_document(app: AppHandle, id: String) -> Result<bool, String> {
  ensure_storage(&app)?;
  let connection = open_connection(&app)?;
  let document = get_document_from_connection(&connection, &id)?;

  connection
    .execute("DELETE FROM documents WHERE id = ?1", [&id])
    .map_err(|error| format!("删除文献记录失败：{error}"))?;

  let stored_path = PathBuf::from(document.stored_path);
  if stored_path.exists() {
    fs::remove_file(stored_path).map_err(|error| format!("删除 PDF 文件失败：{error}"))?;
  }

  Ok(true)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      ensure_storage(&app.handle())?;
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      import_pdf_from_path,
      list_documents,
      get_document,
      get_document_pdf_bytes,
      update_reading_state,
      delete_document
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::{env, fs};

  fn temp_path(file_name: &str) -> PathBuf {
    env::temp_dir().join(format!("novum-{}-{file_name}", std::process::id()))
  }

  #[test]
  fn title_from_path_uses_file_stem() {
    let path = Path::new("/tmp/复杂论文标题.pdf");
    assert_eq!(title_from_path(path), "复杂论文标题");
  }

  #[test]
  fn validate_pdf_path_rejects_non_pdf() {
    let path = temp_path("not-a-pdf.txt");
    fs::write(&path, b"hello").expect("write temp file");

    let result = validate_pdf_path(&path);
    fs::remove_file(&path).expect("remove temp file");

    assert!(result.is_err());
  }

  #[test]
  fn sha256_file_is_stable() {
    let path = temp_path("hash.pdf");
    fs::write(&path, b"abc").expect("write temp file");

    let hash = sha256_file(&path).expect("hash temp file");
    fs::remove_file(&path).expect("remove temp file");

    assert_eq!(
      hash,
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
  }
}
