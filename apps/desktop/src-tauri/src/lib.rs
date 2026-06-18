use chrono::Utc;
use lopdf::Document as PdfDocument;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::Duration,
};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsInput {
    provider: String,
    base_url: String,
    model: String,
    has_api_key: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    provider: String,
    base_url: String,
    model: String,
    has_api_key: bool,
    updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionResult {
    ok: bool,
    message: String,
    checked_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRunLog {
    timestamp: String,
    level: String,
    message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRun {
    id: String,
    kind: String,
    status: String,
    document_id: String,
    started_at: String,
    finished_at: Option<String>,
    error: Option<String>,
    logs: Vec<ResearchRunLog>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QaCitation {
    id: String,
    document_id: String,
    title: String,
    page: Option<i64>,
    excerpt: String,
    source_label: String,
    confidence: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QaAnswer {
    id: String,
    document_id: String,
    question: String,
    answer: String,
    citations: Vec<QaCitation>,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskDocumentResult {
    run: ResearchRun,
    answer: QaAnswer,
}

#[derive(Default)]
struct ResearchProcessState {
    child: Mutex<Option<Child>>,
    port: Mutex<Option<u16>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResearchHealth {
    ok: bool,
    paperqa_available: bool,
}

#[derive(Debug, Deserialize)]
struct IndexServiceResponse {
    run: ResearchRun,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AskServiceResponse {
    run_id: String,
    answer: String,
    citations: Vec<QaCitation>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderPayload {
    base_url: String,
    model: String,
    api_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexDocumentPayload {
    document_id: String,
    pdf_path: String,
    index_path: String,
    provider: ProviderPayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AskDocumentPayload {
    document_id: String,
    pdf_path: String,
    index_path: String,
    question: String,
    provider: ProviderPayload,
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("无法定位应用数据目录：{error}"))
}

fn documents_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("documents"))
}

fn indexes_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("indexes").join("paperqa"))
}

fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("novum.sqlite3"))
}

fn ensure_storage(app: &AppHandle) -> Result<(), String> {
    fs::create_dir_all(documents_dir(app)?)
        .map_err(|error| format!("无法创建文献目录：{error}"))?;
    fs::create_dir_all(indexes_dir(app)?).map_err(|error| format!("无法创建索引目录：{error}"))?;
    let connection = open_connection(app)?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("无法启用文献数据库外键：{error}"))?;

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
      CREATE TABLE IF NOT EXISTS provider_settings (
        id TEXT PRIMARY KEY,
        provider TEXT NOT NULL,
        base_url TEXT NOT NULL,
        model TEXT NOT NULL,
        has_api_key INTEGER NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS document_indexes (
        document_id TEXT PRIMARY KEY,
        status TEXT NOT NULL,
        index_path TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        error TEXT,
        FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
      );
      CREATE TABLE IF NOT EXISTS research_runs (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        status TEXT NOT NULL,
        document_id TEXT NOT NULL,
        started_at TEXT NOT NULL,
        finished_at TEXT,
        error TEXT,
        logs_json TEXT NOT NULL,
        FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
      );
      CREATE TABLE IF NOT EXISTS qa_threads (
        id TEXT PRIMARY KEY,
        document_id TEXT NOT NULL,
        question TEXT NOT NULL,
        answer TEXT NOT NULL,
        run_id TEXT NOT NULL,
        created_at TEXT NOT NULL,
        FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE,
        FOREIGN KEY(run_id) REFERENCES research_runs(id)
      );
      CREATE TABLE IF NOT EXISTS qa_citations (
        id TEXT PRIMARY KEY,
        thread_id TEXT NOT NULL,
        document_id TEXT NOT NULL,
        page INTEGER,
        excerpt TEXT NOT NULL,
        source_label TEXT NOT NULL,
        confidence REAL,
        position INTEGER NOT NULL,
        FOREIGN KEY(thread_id) REFERENCES qa_threads(id) ON DELETE CASCADE,
        FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
      );
      ",
        )
        .map_err(|error| format!("无法初始化文献数据库：{error}"))?;

    Ok(())
}

fn open_connection(app: &AppHandle) -> Result<Connection, String> {
    let path = database_path(app)?;
    let connection =
        Connection::open(path).map_err(|error| format!("无法打开文献数据库：{error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("无法启用文献数据库外键：{error}"))?;
    Ok(connection)
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
    path.file_stem()
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

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn default_provider_settings() -> ProviderSettings {
    ProviderSettings {
        provider: "openai-compatible".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        model: "gpt-4o-mini".to_string(),
        has_api_key: false,
        updated_at: None,
    }
}

fn read_provider_settings(connection: &Connection) -> Result<ProviderSettings, String> {
    connection
        .query_row(
            "
      SELECT provider, base_url, model, has_api_key, updated_at
      FROM provider_settings
      WHERE id = 'default'
      ",
            [],
            |row| {
                Ok(ProviderSettings {
                    provider: row.get(0)?,
                    base_url: row.get(1)?,
                    model: row.get(2)?,
                    has_api_key: row.get::<_, i64>(3)? == 1,
                    updated_at: Some(row.get(4)?),
                })
            },
        )
        .optional()
        .map_err(|error| format!("读取模型配置失败：{error}"))
        .map(|settings| settings.unwrap_or_else(default_provider_settings))
}

fn provider_payload(
    settings: &ProviderSettings,
    api_key: String,
) -> Result<ProviderPayload, String> {
    let key = api_key.trim().to_string();
    if key.is_empty() {
        return Err("请先在模型配置中保存 API Key。".to_string());
    }
    Ok(ProviderPayload {
        base_url: settings.base_url.trim().trim_end_matches('/').to_string(),
        model: settings.model.trim().to_string(),
        api_key: key,
    })
}

fn research_service_dir() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(3)
        .map(|root| root.join("services").join("research"))
        .ok_or_else(|| "无法定位本地研究服务目录。".to_string())
}

fn free_local_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("无法分配研究服务端口：{error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("无法读取研究服务端口：{error}"))
}

fn service_url(port: u16, path: &str) -> String {
    format!("http://127.0.0.1:{port}{path}")
}

fn check_research_health(port: u16) -> bool {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(900))
        .build()
        .and_then(|client| client.get(service_url(port, "/health")).send())
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<ResearchHealth>())
        .map(|health| health.ok && health.paperqa_available)
        .unwrap_or(false)
}

fn ensure_research_service(
    app: &AppHandle,
    state: &tauri::State<'_, ResearchProcessState>,
) -> Result<u16, String> {
    if let Some(port) = *state
        .port
        .lock()
        .map_err(|_| "研究服务状态锁定失败。".to_string())?
    {
        if check_research_health(port) {
            return Ok(port);
        }
    }

    let service_dir = research_service_dir()?;
    if !service_dir.exists() {
        return Err("找不到 services/research，本地研究服务尚未初始化。".to_string());
    }

    let port = free_local_port()?;
    let venv_python = service_dir.join(".venv").join("bin").join("python");
    let python = std::env::var("NOVUM_PYTHON").unwrap_or_else(|_| {
        if venv_python.exists() {
            venv_python.to_string_lossy().to_string()
        } else {
            "python3".to_string()
        }
    });
    let app_data = app_data_dir(app)?;

    let child = Command::new(python)
        .arg("-m")
        .arg("uvicorn")
        .arg("novum_research.app:app")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .current_dir(&service_dir)
        .env("PYTHONPATH", &service_dir)
        .env("NOVUM_APP_DATA_DIR", app_data)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            format!("无法启动本地研究服务：{error}。请先在 services/research 安装 Python 依赖。")
        })?;

    {
        let mut child_slot = state
            .child
            .lock()
            .map_err(|_| "研究服务进程锁定失败。".to_string())?;
        if let Some(mut previous) = child_slot.take() {
            let _ = previous.kill();
        }
        *child_slot = Some(child);
    }

    for _ in 0..30 {
        if check_research_health(port) {
            *state
                .port
                .lock()
                .map_err(|_| "研究服务状态锁定失败。".to_string())? = Some(port);
            return Ok(port);
        }
        thread::sleep(Duration::from_millis(200));
    }

    Err("本地研究服务已启动但未就绪。请确认 services/research 已安装 PaperQA 依赖。".to_string())
}

fn make_run(kind: &str, document_id: &str, message: &str) -> ResearchRun {
    let now = now_string();
    ResearchRun {
        id: Uuid::new_v4().to_string(),
        kind: kind.to_string(),
        status: "running".to_string(),
        document_id: document_id.to_string(),
        started_at: now.clone(),
        finished_at: None,
        error: None,
        logs: vec![ResearchRunLog {
            timestamp: now,
            level: "info".to_string(),
            message: message.to_string(),
        }],
    }
}

fn persist_run(connection: &Connection, run: &ResearchRun) -> Result<(), String> {
    let logs_json = serde_json::to_string(&run.logs).unwrap_or_else(|_| "[]".to_string());
    connection
        .execute(
            "
      INSERT INTO research_runs (
        id, kind, status, document_id, started_at, finished_at, error, logs_json
      )
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
      ON CONFLICT(id) DO UPDATE SET
        status = excluded.status,
        finished_at = excluded.finished_at,
        error = excluded.error,
        logs_json = excluded.logs_json
      ",
            params![
                run.id,
                run.kind,
                run.status,
                run.document_id,
                run.started_at,
                run.finished_at,
                run.error,
                logs_json
            ],
        )
        .map_err(|error| format!("保存研究任务失败：{error}"))?;
    Ok(())
}

fn finish_run(
    connection: &Connection,
    run: &mut ResearchRun,
    status: &str,
    message: &str,
) -> Result<(), String> {
    let is_error = status == "failed";
    run.status = status.to_string();
    run.finished_at = Some(now_string());
    run.error = is_error.then(|| message.to_string());
    run.logs.push(ResearchRunLog {
        timestamp: now_string(),
        level: if is_error { "error" } else { "info" }.to_string(),
        message: message.to_string(),
    });
    persist_run(connection, run)
}

fn set_document_status(connection: &Connection, id: &str, status: &str) -> Result<(), String> {
    connection
        .execute(
            "UPDATE documents SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now_string(), id],
        )
        .map_err(|error| format!("更新文献状态失败：{error}"))?;
    Ok(())
}

fn upsert_document_index(
    connection: &Connection,
    document_id: &str,
    status: &str,
    index_path: &str,
    error: Option<&str>,
) -> Result<(), String> {
    let now = now_string();
    connection
        .execute(
            "
      INSERT INTO document_indexes (
        document_id, status, index_path, created_at, updated_at, error
      )
      VALUES (?1, ?2, ?3, ?4, ?4, ?5)
      ON CONFLICT(document_id) DO UPDATE SET
        status = excluded.status,
        index_path = excluded.index_path,
        updated_at = excluded.updated_at,
        error = excluded.error
      ",
            params![document_id, status, index_path, now, error],
        )
        .map_err(|error| format!("保存索引状态失败：{error}"))?;
    Ok(())
}

fn extract_service_error(response: reqwest::blocking::Response) -> String {
    let status = response.status();
    match response.text() {
        Ok(text) if !text.trim().is_empty() => serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("detail")
                    .and_then(|detail| detail.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("研究服务返回错误 {status}：{text}")),
        _ => format!("研究服务返回错误 {status}。"),
    }
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
fn get_provider_settings(app: AppHandle) -> Result<ProviderSettings, String> {
    ensure_storage(&app)?;
    let connection = open_connection(&app)?;
    read_provider_settings(&connection)
}

#[tauri::command]
fn save_provider_settings(
    app: AppHandle,
    settings: ProviderSettingsInput,
) -> Result<ProviderSettings, String> {
    ensure_storage(&app)?;
    if settings.provider != "openai-compatible" {
        return Err("Phase 3 仅支持 OpenAI-compatible 模型服务。".to_string());
    }
    if settings.base_url.trim().is_empty() {
        return Err("Base URL 不能为空。".to_string());
    }
    if settings.model.trim().is_empty() {
        return Err("模型名称不能为空。".to_string());
    }

    let connection = open_connection(&app)?;
    connection
        .execute(
            "
      INSERT INTO provider_settings (
        id, provider, base_url, model, has_api_key, updated_at
      )
      VALUES ('default', ?1, ?2, ?3, ?4, ?5)
      ON CONFLICT(id) DO UPDATE SET
        provider = excluded.provider,
        base_url = excluded.base_url,
        model = excluded.model,
        has_api_key = excluded.has_api_key,
        updated_at = excluded.updated_at
      ",
            params![
                settings.provider,
                settings.base_url.trim().trim_end_matches('/').to_string(),
                settings.model.trim().to_string(),
                if settings.has_api_key { 1 } else { 0 },
                now_string()
            ],
        )
        .map_err(|error| format!("保存模型配置失败：{error}"))?;

    read_provider_settings(&connection)
}

#[tauri::command]
fn test_provider_connection(
    app: AppHandle,
    api_key: String,
) -> Result<ProviderConnectionResult, String> {
    ensure_storage(&app)?;
    let connection = open_connection(&app)?;
    let settings = read_provider_settings(&connection)?;
    let provider = provider_payload(&settings, api_key)?;
    let checked_at = now_string();
    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
    let result = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("无法创建模型连接客户端：{error}"))?
        .get(url)
        .bearer_auth(provider.api_key)
        .send();

    match result {
        Ok(response) if response.status().is_success() => Ok(ProviderConnectionResult {
            ok: true,
            message: "模型服务连接成功。".to_string(),
            checked_at,
        }),
        Ok(response) => Ok(ProviderConnectionResult {
            ok: false,
            message: extract_service_error(response),
            checked_at,
        }),
        Err(error) => Ok(ProviderConnectionResult {
            ok: false,
            message: format!("模型服务连接失败：{error}"),
            checked_at,
        }),
    }
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
fn index_document(
    app: AppHandle,
    state: tauri::State<'_, ResearchProcessState>,
    id: String,
    api_key: String,
) -> Result<ResearchRun, String> {
    ensure_storage(&app)?;
    let connection = open_connection(&app)?;
    let document = get_document_from_connection(&connection, &id)?;
    let settings = read_provider_settings(&connection)?;
    let provider = provider_payload(&settings, api_key)?;
    let index_path = indexes_dir(&app)?
        .join(&document.id)
        .to_string_lossy()
        .to_string();
    let mut run = make_run("index_document", &document.id, "开始建立 PaperQA 索引。");

    persist_run(&connection, &run)?;
    set_document_status(&connection, &document.id, "索引中")?;
    upsert_document_index(&connection, &document.id, "索引中", &index_path, None)?;

    let port = match ensure_research_service(&app, &state) {
        Ok(port) => port,
        Err(error) => {
            set_document_status(&connection, &document.id, "索引失败")?;
            upsert_document_index(
                &connection,
                &document.id,
                "索引失败",
                &index_path,
                Some(&error),
            )?;
            finish_run(&connection, &mut run, "failed", &error)?;
            return Err(error);
        }
    };

    let payload = IndexDocumentPayload {
        document_id: document.id.clone(),
        pdf_path: document.stored_path.clone(),
        index_path: index_path.clone(),
        provider,
    };

    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|error| format!("无法创建研究服务客户端：{error}"))?
        .post(service_url(port, "/documents/index"))
        .json(&payload)
        .send();

    match response {
        Ok(response) if response.status().is_success() => {
            let service_response = response
                .json::<IndexServiceResponse>()
                .map_err(|error| format!("解析索引响应失败：{error}"))?;
            run.logs.push(ResearchRunLog {
                timestamp: now_string(),
                level: "info".to_string(),
                message: format!("Python 研究任务 {} 已完成。", service_response.run.id),
            });
            set_document_status(&connection, &document.id, "已索引")?;
            upsert_document_index(&connection, &document.id, "已索引", &index_path, None)?;
            finish_run(&connection, &mut run, "succeeded", "PaperQA 索引已完成。")?;
            Ok(run)
        }
        Ok(response) => {
            let error = extract_service_error(response);
            set_document_status(&connection, &document.id, "索引失败")?;
            upsert_document_index(
                &connection,
                &document.id,
                "索引失败",
                &index_path,
                Some(&error),
            )?;
            finish_run(&connection, &mut run, "failed", &error)?;
            Err(error)
        }
        Err(error) => {
            let message = format!("调用研究服务失败：{error}");
            set_document_status(&connection, &document.id, "索引失败")?;
            upsert_document_index(
                &connection,
                &document.id,
                "索引失败",
                &index_path,
                Some(&message),
            )?;
            finish_run(&connection, &mut run, "failed", &message)?;
            Err(message)
        }
    }
}

#[tauri::command]
fn ask_document(
    app: AppHandle,
    state: tauri::State<'_, ResearchProcessState>,
    id: String,
    question: String,
    api_key: String,
) -> Result<AskDocumentResult, String> {
    ensure_storage(&app)?;
    let trimmed_question = question.trim().to_string();
    if trimmed_question.is_empty() {
        return Err("问题不能为空。".to_string());
    }

    let connection = open_connection(&app)?;
    let document = get_document_from_connection(&connection, &id)?;
    let settings = read_provider_settings(&connection)?;
    let provider = provider_payload(&settings, api_key)?;
    let index_path: String = connection
        .query_row(
            "SELECT index_path FROM document_indexes WHERE document_id = ?1 AND status = '已索引'",
            [&document.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("读取索引状态失败：{error}"))?
        .ok_or_else(|| "当前文献尚未索引，请先索引后再提问。".to_string())?;

    let mut run = make_run(
        "ask_document",
        &document.id,
        "开始调用 PaperQA 回答当前问题。",
    );
    persist_run(&connection, &run)?;

    let port = match ensure_research_service(&app, &state) {
        Ok(port) => port,
        Err(error) => {
            set_document_status(&connection, &document.id, "问答失败")?;
            finish_run(&connection, &mut run, "failed", &error)?;
            return Err(error);
        }
    };

    let payload = AskDocumentPayload {
        document_id: document.id.clone(),
        pdf_path: document.stored_path.clone(),
        index_path,
        question: trimmed_question.clone(),
        provider,
    };

    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|error| format!("无法创建研究服务客户端：{error}"))?
        .post(service_url(port, "/documents/ask"))
        .json(&payload)
        .send();

    match response {
        Ok(response) if response.status().is_success() => {
            let service_response = response
                .json::<AskServiceResponse>()
                .map_err(|error| format!("解析问答响应失败：{error}"))?;
            run.logs.push(ResearchRunLog {
                timestamp: now_string(),
                level: "info".to_string(),
                message: format!("Python 研究任务 {} 已完成。", service_response.run_id),
            });
            finish_run(&connection, &mut run, "succeeded", "PaperQA 问答已完成。")?;

            let answer_id = Uuid::new_v4().to_string();
            let created_at = now_string();
            connection
                .execute(
                    "
          INSERT INTO qa_threads (id, document_id, question, answer, run_id, created_at)
          VALUES (?1, ?2, ?3, ?4, ?5, ?6)
          ",
                    params![
                        answer_id,
                        document.id,
                        trimmed_question,
                        service_response.answer,
                        run.id,
                        created_at
                    ],
                )
                .map_err(|error| format!("保存问答记录失败：{error}"))?;

            let mut citations = Vec::new();
            for (index, mut citation) in service_response.citations.into_iter().enumerate() {
                citation.document_id = document.id.clone();
                if citation.title == document.id {
                    citation.title = document.title.clone();
                }
                connection
                    .execute(
                        "
            INSERT INTO qa_citations (
              id, thread_id, document_id, page, excerpt, source_label, confidence, position
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
                        params![
                            citation.id,
                            answer_id,
                            citation.document_id,
                            citation.page,
                            citation.excerpt,
                            citation.source_label,
                            citation.confidence,
                            index as i64
                        ],
                    )
                    .map_err(|error| format!("保存引用记录失败：{error}"))?;
                citations.push(citation);
            }

            Ok(AskDocumentResult {
                run,
                answer: QaAnswer {
                    id: answer_id,
                    document_id: document.id,
                    question: trimmed_question,
                    answer: service_response.answer,
                    citations,
                    created_at,
                },
            })
        }
        Ok(response) => {
            let error = extract_service_error(response);
            set_document_status(&connection, &document.id, "问答失败")?;
            finish_run(&connection, &mut run, "failed", &error)?;
            Err(error)
        }
        Err(error) => {
            let message = format!("调用研究服务失败：{error}");
            set_document_status(&connection, &document.id, "问答失败")?;
            finish_run(&connection, &mut run, "failed", &message)?;
            Err(message)
        }
    }
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
        .manage(ResearchProcessState::default())
        .setup(|app| {
            ensure_storage(&app.handle())?;
            let salt_path = app
                .path()
                .app_local_data_dir()
                .map_err(|error| format!("无法定位 Stronghold 数据目录：{error}"))?
                .join("stronghold-salt.txt");
            app.handle()
                .plugin(tauri_plugin_stronghold::Builder::with_argon2(&salt_path).build())?;
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
            get_provider_settings,
            save_provider_settings,
            test_provider_connection,
            get_document_pdf_bytes,
            update_reading_state,
            index_document,
            ask_document,
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
