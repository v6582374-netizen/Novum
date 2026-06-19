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
    time::{Duration, UNIX_EPOCH},
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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreset {
    provider: String,
    label: String,
    description: String,
    base_url: String,
    model: String,
    has_development_key: bool,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillInputSpec {
    name: String,
    label: String,
    #[serde(rename = "type")]
    input_type: String,
    required: bool,
    default_value: Option<serde_json::Value>,
    help: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScienceSkill {
    id: String,
    name: String,
    description: String,
    domain: String,
    source: String,
    source_path: String,
    upstream_commit: String,
    required_inputs: Vec<SkillInputSpec>,
    required_env: Vec<String>,
    execution_mode: String,
    status: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillRunLog {
    timestamp: String,
    level: String,
    message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillRunOutput {
    id: String,
    kind: String,
    title: String,
    content: String,
    file_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillRun {
    id: String,
    skill_id: String,
    skill_name: String,
    status: String,
    started_at: String,
    finished_at: Option<String>,
    error: Option<String>,
    logs: Vec<SkillRunLog>,
    outputs: Vec<SkillRunOutput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillRunContext {
    active_document_id: Option<String>,
    active_document_path: Option<String>,
    selected_text: Option<String>,
    provider: Option<ProviderPayload>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunSkillPayload {
    inputs: serde_json::Value,
    context: SkillRunContext,
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
      CREATE TABLE IF NOT EXISTS skills_cache (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT NOT NULL,
        domain TEXT NOT NULL,
        source TEXT NOT NULL,
        source_path TEXT NOT NULL,
        upstream_commit TEXT NOT NULL,
        execution_mode TEXT NOT NULL,
        status TEXT NOT NULL,
        required_inputs_json TEXT NOT NULL,
        required_env_json TEXT NOT NULL,
        updated_at TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS skill_runs (
        id TEXT PRIMARY KEY,
        skill_id TEXT NOT NULL,
        skill_name TEXT NOT NULL,
        status TEXT NOT NULL,
        inputs_json TEXT NOT NULL,
        context_json TEXT NOT NULL,
        started_at TEXT NOT NULL,
        finished_at TEXT,
        error TEXT,
        logs_json TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS skill_run_outputs (
        id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        title TEXT NOT NULL,
        content TEXT NOT NULL,
        file_path TEXT,
        position INTEGER NOT NULL,
        FOREIGN KEY(run_id) REFERENCES skill_runs(id) ON DELETE CASCADE
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

fn provider_defaults(provider: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match provider {
        "openai-compatible" => Some((
            "OpenAI-compatible",
            "https://api.openai.com/v1",
            "gpt-4o-mini",
        )),
        "deepseek" => Some((
            "DeepSeek 官方",
            "https://api.deepseek.com",
            "deepseek-v4-flash",
        )),
        "test-relay" => Some(("测试中转站", "https://chenghuaai.com/v1", "claude-opus-4-6")),
        _ => None,
    }
}

fn default_provider_settings() -> ProviderSettings {
    let (_, base_url, model) = provider_defaults("openai-compatible").expect("default provider");
    ProviderSettings {
        provider: "openai-compatible".to_string(),
        base_url: base_url.to_string(),
        model: model.to_string(),
        has_api_key: false,
        updated_at: None,
    }
}

fn development_key_env_names(provider: &str) -> &'static [&'static str] {
    match provider {
        "deepseek" => &["DEEPSEEK_API_KEY", "NOVUM_DEEPSEEK_API_KEY"],
        "test-relay" => &["NOVUM_TEST_RELAY_API_KEY", "NOVUM_TEST_PROVIDER_API_KEY"],
        "openai-compatible" => &["OPENAI_API_KEY", "NOVUM_OPENAI_API_KEY"],
        _ => &[],
    }
}

fn development_key_file(provider: &str) -> Result<PathBuf, String> {
    Ok(repo_root_dir()?
        .join("secrets")
        .join(format!("{provider}-api-key.txt")))
}

fn read_development_api_key(provider: &str) -> Result<Option<String>, String> {
    for name in development_key_env_names(provider) {
        if let Ok(value) = std::env::var(name) {
            let trimmed = value.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed));
            }
        }
    }

    let path = development_key_file(provider)?;
    if path.is_file() {
        let value =
            fs::read_to_string(path).map_err(|error| format!("读取本地测试密钥失败：{error}"))?;
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed));
        }
    }

    Ok(None)
}

fn provider_presets() -> Result<Vec<ProviderPreset>, String> {
    Ok(vec![
        ProviderPreset {
            provider: "openai-compatible".to_string(),
            label: "OpenAI-compatible".to_string(),
            description: "自定义 OpenAI-compatible 服务，适合 OpenAI、Azure 兼容网关或本地代理。"
                .to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            has_development_key: false,
        },
        ProviderPreset {
            provider: "deepseek".to_string(),
            label: "DeepSeek 官方".to_string(),
            description: "DeepSeek 官方 OpenAI-compatible API，默认模型 deepseek-v4-flash。"
                .to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            model: "deepseek-v4-flash".to_string(),
            has_development_key: false,
        },
        ProviderPreset {
            provider: "test-relay".to_string(),
            label: "测试中转站".to_string(),
            description: "本地开发测试 preset，默认使用 chenghuaai.com 与 claude-opus-4-6。密钥只从本机环境或 secrets/ 读取。"
                .to_string(),
            base_url: "https://chenghuaai.com/v1".to_string(),
            model: "claude-opus-4-6".to_string(),
            has_development_key: read_development_api_key("test-relay")?.is_some(),
        },
    ])
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
    Ok(repo_root_dir()?.join("services").join("research"))
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
        .map(|health| health.ok)
        .unwrap_or(false)
}

fn repo_root_dir() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法定位项目根目录。".to_string())
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
    let repo_root = repo_root_dir()?;
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
        .env("NOVUM_REPO_ROOT", repo_root)
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

    Err("本地研究服务已启动但未就绪。请确认 services/research 已安装 Python 依赖。".to_string())
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

fn persist_skill_cache(connection: &Connection, skill: &ScienceSkill) -> Result<(), String> {
    let required_inputs_json = serde_json::to_string(&skill.required_inputs)
        .map_err(|error| format!("序列化技能输入失败：{error}"))?;
    let required_env_json = serde_json::to_string(&skill.required_env)
        .map_err(|error| format!("序列化技能环境变量失败：{error}"))?;

    connection
        .execute(
            "
      INSERT INTO skills_cache (
        id, name, description, domain, source, source_path, upstream_commit,
        execution_mode, status, required_inputs_json, required_env_json, updated_at
      )
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
      ON CONFLICT(id) DO UPDATE SET
        name = excluded.name,
        description = excluded.description,
        domain = excluded.domain,
        source = excluded.source,
        source_path = excluded.source_path,
        upstream_commit = excluded.upstream_commit,
        execution_mode = excluded.execution_mode,
        status = excluded.status,
        required_inputs_json = excluded.required_inputs_json,
        required_env_json = excluded.required_env_json,
        updated_at = excluded.updated_at
      ",
            params![
                skill.id,
                skill.name,
                skill.description,
                skill.domain,
                skill.source,
                skill.source_path,
                skill.upstream_commit,
                skill.execution_mode,
                skill.status,
                required_inputs_json,
                required_env_json,
                skill.updated_at
            ],
        )
        .map_err(|error| format!("保存技能缓存失败：{error}"))?;
    Ok(())
}

fn row_to_science_skill(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScienceSkill> {
    let required_inputs_json: String = row.get(9)?;
    let required_env_json: String = row.get(10)?;
    Ok(ScienceSkill {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        domain: row.get(3)?,
        source: row.get(4)?,
        source_path: row.get(5)?,
        upstream_commit: row.get(6)?,
        execution_mode: row.get(7)?,
        status: row.get(8)?,
        required_inputs: serde_json::from_str(&required_inputs_json).unwrap_or_default(),
        required_env: serde_json::from_str(&required_env_json).unwrap_or_default(),
        updated_at: row.get(11)?,
    })
}

fn get_cached_skill(connection: &Connection, id: &str) -> Result<Option<ScienceSkill>, String> {
    connection
        .query_row(
            "
      SELECT id, name, description, domain, source, source_path, upstream_commit,
             execution_mode, status, required_inputs_json, required_env_json, updated_at
      FROM skills_cache
      WHERE id = ?1
      ",
            [id],
            row_to_science_skill,
        )
        .optional()
        .map_err(|error| format!("读取技能缓存失败：{error}"))
}

fn persist_skill_run(
    connection: &Connection,
    run: &SkillRun,
    inputs_json: &str,
    context_json: &str,
) -> Result<(), String> {
    let logs_json =
        serde_json::to_string(&run.logs).map_err(|error| format!("序列化技能日志失败：{error}"))?;
    connection
        .execute(
            "
      INSERT INTO skill_runs (
        id, skill_id, skill_name, status, inputs_json, context_json,
        started_at, finished_at, error, logs_json
      )
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
      ON CONFLICT(id) DO UPDATE SET
        status = excluded.status,
        finished_at = excluded.finished_at,
        error = excluded.error,
        logs_json = excluded.logs_json
      ",
            params![
                run.id,
                run.skill_id,
                run.skill_name,
                run.status,
                inputs_json,
                context_json,
                run.started_at,
                run.finished_at,
                run.error,
                logs_json
            ],
        )
        .map_err(|error| format!("保存技能运行记录失败：{error}"))?;

    connection
        .execute("DELETE FROM skill_run_outputs WHERE run_id = ?1", [&run.id])
        .map_err(|error| format!("清理技能输出失败：{error}"))?;
    for (index, output) in run.outputs.iter().enumerate() {
        connection
            .execute(
                "
        INSERT INTO skill_run_outputs (
          id, run_id, kind, title, content, file_path, position
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
                params![
                    output.id,
                    run.id,
                    output.kind,
                    output.title,
                    output.content,
                    output.file_path,
                    index as i64
                ],
            )
            .map_err(|error| format!("保存技能输出失败：{error}"))?;
    }

    Ok(())
}

fn get_skill_run_from_connection(connection: &Connection, id: &str) -> Result<SkillRun, String> {
    let mut run = connection
        .query_row(
            "
      SELECT id, skill_id, skill_name, status, started_at, finished_at, error, logs_json
      FROM skill_runs
      WHERE id = ?1
      ",
            [id],
            |row| {
                let logs_json: String = row.get(7)?;
                Ok(SkillRun {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    skill_name: row.get(2)?,
                    status: row.get(3)?,
                    started_at: row.get(4)?,
                    finished_at: row.get(5)?,
                    error: row.get(6)?,
                    logs: serde_json::from_str(&logs_json).unwrap_or_default(),
                    outputs: Vec::new(),
                })
            },
        )
        .optional()
        .map_err(|error| format!("读取技能运行记录失败：{error}"))?
        .ok_or_else(|| "找不到技能运行记录。".to_string())?;

    let mut statement = connection
        .prepare(
            "
      SELECT id, kind, title, content, file_path
      FROM skill_run_outputs
      WHERE run_id = ?1
      ORDER BY position ASC
      ",
        )
        .map_err(|error| format!("读取技能输出失败：{error}"))?;
    let rows = statement
        .query_map([id], |row| {
            Ok(SkillRunOutput {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                file_path: row.get(4)?,
            })
        })
        .map_err(|error| format!("读取技能输出列表失败：{error}"))?;
    for row in rows {
        run.outputs
            .push(row.map_err(|error| format!("解析技能输出失败：{error}"))?);
    }

    Ok(run)
}

fn science_skills_dir() -> Result<PathBuf, String> {
    Ok(repo_root_dir()?.join("vendor").join("science-skills"))
}

fn science_skills_skills_dir() -> Result<PathBuf, String> {
    Ok(science_skills_dir()?.join("skills"))
}

fn normalize_skill_id(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    (!normalized.is_empty()).then_some(normalized)
}

fn compact_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_frontmatter(text: &str) -> (std::collections::HashMap<String, String>, String) {
    let mut metadata = std::collections::HashMap::new();
    if !text.starts_with("---") {
        return (metadata, text.to_string());
    }

    let lines = text.lines().collect::<Vec<_>>();
    let Some(closing_index) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (line.trim() == "---").then_some(index))
    else {
        return (metadata, text.to_string());
    };

    let mut current_key: Option<String> = None;
    let mut current_lines = Vec::new();
    let mut flush = |key: &mut Option<String>, lines: &mut Vec<String>| {
        if let Some(key) = key.take() {
            metadata.insert(key, compact_text(&lines.join(" ")));
            lines.clear();
        }
    };

    for line in &lines[1..closing_index] {
        let starts_with_whitespace = line
            .chars()
            .next()
            .map(char::is_whitespace)
            .unwrap_or(false);
        if !starts_with_whitespace && line.contains(':') {
            flush(&mut current_key, &mut current_lines);
            let mut parts = line.splitn(2, ':');
            let key = parts.next().unwrap_or_default().trim().to_string();
            let raw_value = parts.next().unwrap_or_default().trim();
            current_key = Some(key);
            if !matches!(raw_value, ">" | ">-" | "|" | "|-") {
                current_lines.push(raw_value.trim_matches('"').trim_matches('\'').to_string());
            }
        } else if current_key.is_some() {
            current_lines.push(line.trim().to_string());
        }
    }
    flush(&mut current_key, &mut current_lines);

    (metadata, lines[closing_index + 1..].join("\n"))
}

fn first_markdown_heading(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim).map(str::to_string))
        .filter(|value| !value.is_empty())
}

fn first_markdown_paragraph(markdown: &str) -> Option<String> {
    let mut lines = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("---") {
            if !lines.is_empty() {
                break;
            }
            continue;
        }
        if trimmed.starts_with("```")
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("1.")
        {
            if !lines.is_empty() {
                break;
            }
            continue;
        }
        lines.push(trimmed);
    }
    (!lines.is_empty()).then(|| compact_text(&lines.join(" ")))
}

fn extract_required_env(text: &str) -> Vec<String> {
    let mut values = text
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| {
            token.len() > 6
                && token.chars().all(|character| {
                    character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
                })
                && (token.ends_with("API_KEY")
                    || token.ends_with("ACCESS_TOKEN")
                    || token.ends_with("TOKEN")
                    || token.ends_with("SECRET"))
                && !matches!(*token, "API_KEY" | "ACCESS_TOKEN" | "TOKEN" | "SECRET")
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn mentions_uv(text: &str) -> bool {
    text.split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .any(|token| token.eq_ignore_ascii_case("uv"))
}

fn executable_in_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|path| path.join(name).is_file()))
        .unwrap_or(false)
}

fn infer_skill_domain(skill_id: &str, text: &str) -> String {
    let probe = format!(
        "{} {}",
        skill_id,
        text.chars().take(1200).collect::<String>()
    )
    .to_lowercase();
    if ["literature", "pubmed", "arxiv", "biorxiv", "openalex"]
        .iter()
        .any(|term| probe.contains(term))
    {
        return "文献检索".to_string();
    }
    if ["protein", "uniprot", "pdb", "alphafold", "foldseek"]
        .iter()
        .any(|term| probe.contains(term))
    {
        return "蛋白质".to_string();
    }
    if [
        "variant",
        "genome",
        "genomic",
        "gnomad",
        "clinvar",
        "dbsnp",
        "alphagenome",
        "encode",
        "gtex",
        "jaspar",
        "ucsc",
        "unibind",
        "ensembl",
    ]
    .iter()
    .any(|term| probe.contains(term))
    {
        return "基因组学".to_string();
    }
    if ["chembl", "pubchem", "chemistry", "compound"]
        .iter()
        .any(|term| probe.contains(term))
    {
        return "化学".to_string();
    }
    if ["clinical", "openfda", "trial", "disease"]
        .iter()
        .any(|term| probe.contains(term))
    {
        return "临床医学".to_string();
    }
    if ["ontology", "go ", "reactome", "interpro", "string"]
        .iter()
        .any(|term| probe.contains(term))
    {
        return "生物知识库".to_string();
    }
    if ["uv", "workflow", "scienceskillscommon"]
        .iter()
        .any(|term| probe.contains(term))
    {
        return "基础工具".to_string();
    }
    "科学技能".to_string()
}

fn skill_status(skill_id: &str, text: &str, required_env: &[String]) -> String {
    if skill_id == "uv" {
        return "可用".to_string();
    }
    if mentions_uv(text) && !executable_in_path("uv") {
        return "缺少依赖".to_string();
    }
    if required_env.iter().any(|name| {
        std::env::var(name)
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
    }) {
        return "需要配置".to_string();
    }
    "可用".to_string()
}

fn skill_updated_at(path: &Path) -> String {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0))
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(now_string)
}

fn parse_science_skill(path: &Path) -> Result<ScienceSkill, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("读取技能文件失败：{error}"))?;
    let skill_id = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .ok_or_else(|| "无法解析技能 ID。".to_string())?
        .to_string();
    let (metadata, body) = parse_frontmatter(&text);
    let heading =
        first_markdown_heading(&body).unwrap_or_else(|| skill_id.replace(['_', '-'], " "));
    let name = metadata.get("name").cloned().unwrap_or(heading.clone());
    let description = metadata
        .get("description")
        .cloned()
        .or_else(|| first_markdown_paragraph(&body))
        .unwrap_or(heading);
    let required_env = extract_required_env(&text);
    let has_scripts = path
        .parent()
        .map(|parent| parent.join("scripts").is_dir())
        .unwrap_or(false);
    let execution_mode = if has_scripts && !required_env.is_empty() {
        "hybrid"
    } else if has_scripts {
        "python"
    } else {
        "prompt"
    }
    .to_string();
    let source_path = path
        .strip_prefix(repo_root_dir()?)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    Ok(ScienceSkill {
        id: skill_id.clone(),
        name,
        description: compact_text(&description),
        domain: infer_skill_domain(&skill_id, &text),
        source: "science-skills".to_string(),
        source_path,
        upstream_commit: "33557e0f1faf0f281d255940de58935c61b2143b".to_string(),
        required_inputs: vec![SkillInputSpec {
            name: "task".to_string(),
            label: "任务上下文".to_string(),
            input_type: "textarea".to_string(),
            required: true,
            default_value: None,
            help: Some("描述你希望该技能处理的问题、对象或当前研究任务。".to_string()),
        }],
        required_env: required_env.clone(),
        execution_mode,
        status: skill_status(&skill_id, &text, &required_env),
        updated_at: skill_updated_at(path),
    })
}

fn list_science_skills_from_vendor() -> Result<Vec<ScienceSkill>, String> {
    let root = science_skills_skills_dir()?;
    if !root.is_dir() {
        return Err("找不到 vendor/science-skills/skills，请确认上游快照已导入。".to_string());
    }

    let mut skills = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| format!("读取技能目录失败：{error}"))?
    {
        let entry = entry.map_err(|error| format!("读取技能目录项失败：{error}"))?;
        let path = entry.path().join("SKILL.md");
        if path.is_file() {
            skills.push(parse_science_skill(&path)?);
        }
    }
    skills.sort_by(|left, right| {
        left.domain
            .cmp(&right.domain)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(skills)
}

fn get_science_skill_from_vendor(id: &str) -> Result<ScienceSkill, String> {
    let skill_id = normalize_skill_id(id).ok_or_else(|| "技能 ID 不能为空。".to_string())?;
    let path = science_skills_skills_dir()?.join(skill_id).join("SKILL.md");
    if !path.is_file() {
        return Err("找不到这个科学技能。".to_string());
    }
    parse_science_skill(&path)
}

fn make_skill_log(level: &str, message: &str) -> SkillRunLog {
    SkillRunLog {
        timestamp: now_string(),
        level: level.to_string(),
        message: message.to_string(),
    }
}

fn build_skill_dry_run_output(
    skill: &ScienceSkill,
    inputs: &serde_json::Value,
    active_document_id: Option<&str>,
    active_document_path: Option<&str>,
) -> String {
    let task = inputs
        .get("task")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim();
    [
        format!("# {}", skill.name),
        String::new(),
        "## dry-run 结果".to_string(),
        String::new(),
        "Novum 已完成技能选择、上下文绑定和依赖检查。当前阶段不会直接执行上游脚本；真实执行会在后续受控 runner 中接入。".to_string(),
        String::new(),
        "## 技能信息".to_string(),
        String::new(),
        format!("- 技能 ID：`{}`", skill.id),
        format!("- 领域：{}", skill.domain),
        format!("- 来源：{}", skill.source_path),
        format!("- 执行模式：{}", skill.execution_mode),
        format!("- 状态：{}", skill.status),
        String::new(),
        "## 当前上下文".to_string(),
        String::new(),
        format!("- 当前文献 ID：{}", active_document_id.unwrap_or("未绑定当前文献")),
        format!("- 当前文献路径：{}", active_document_path.unwrap_or("无")),
        String::new(),
        "## 用户任务".to_string(),
        String::new(),
        task.to_string(),
    ]
    .join("\n")
}

fn list_cached_skills(connection: &Connection) -> Result<Vec<ScienceSkill>, String> {
    let mut statement = connection
        .prepare(
            "
      SELECT id, name, description, domain, source, source_path, upstream_commit,
             execution_mode, status, required_inputs_json, required_env_json, updated_at
      FROM skills_cache
      ORDER BY domain ASC, name ASC
      ",
        )
        .map_err(|error| format!("读取技能缓存失败：{error}"))?;
    let rows = statement
        .query_map([], row_to_science_skill)
        .map_err(|error| format!("读取技能缓存列表失败：{error}"))?;

    let mut skills = Vec::new();
    for row in rows {
        skills.push(row.map_err(|error| format!("解析技能缓存失败：{error}"))?);
    }
    Ok(skills)
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
    let provider = settings.provider.trim();
    if provider_defaults(provider).is_none() {
        return Err("当前仅支持 OpenAI-compatible、DeepSeek 官方和测试中转站。".to_string());
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
                provider,
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
fn get_provider_presets() -> Result<Vec<ProviderPreset>, String> {
    provider_presets()
}

#[tauri::command]
fn load_development_api_key(provider: String) -> Result<Option<String>, String> {
    let provider = provider.trim();
    if provider != "test-relay" {
        return Err("本机测试密钥仅用于测试中转站。".to_string());
    }
    read_development_api_key(provider)
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
    let url = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );
    let payload = serde_json::json!({
        "model": provider.model,
        "messages": [
            {
                "role": "user",
                "content": "用一句话回复：Novum 模型服务连接成功。"
            }
        ],
        "max_tokens": 32,
        "stream": false
    });
    let result = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("无法创建模型连接客户端：{error}"))?
        .post(url)
        .header("Content-Type", "application/json")
        .bearer_auth(provider.api_key)
        .json(&payload)
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
fn list_skills(app: AppHandle) -> Result<Vec<ScienceSkill>, String> {
    ensure_storage(&app)?;
    let connection = open_connection(&app)?;

    match list_science_skills_from_vendor() {
        Ok(skills) => {
            for skill in &skills {
                persist_skill_cache(&connection, skill)?;
            }
            Ok(skills)
        }
        Err(error) => {
            let cached = list_cached_skills(&connection)?;
            if cached.is_empty() {
                Err(error)
            } else {
                Ok(cached)
            }
        }
    }
}

#[tauri::command]
fn get_skill(app: AppHandle, id: String) -> Result<ScienceSkill, String> {
    ensure_storage(&app)?;
    let connection = open_connection(&app)?;

    match get_science_skill_from_vendor(&id) {
        Ok(skill) => {
            persist_skill_cache(&connection, &skill)?;
            Ok(skill)
        }
        Err(error) => get_cached_skill(&connection, &id)?.ok_or(error),
    }
}

#[tauri::command]
fn run_skill(
    app: AppHandle,
    id: String,
    inputs: serde_json::Value,
    active_document_id: Option<String>,
) -> Result<SkillRun, String> {
    ensure_storage(&app)?;
    if !inputs.is_object() {
        return Err("技能输入必须是结构化对象。".to_string());
    }

    let connection = open_connection(&app)?;
    let (active_document_id, active_document_path) = match active_document_id {
        Some(document_id) if !document_id.trim().is_empty() => {
            let document = get_document_from_connection(&connection, &document_id)?;
            (Some(document.id), Some(document.stored_path))
        }
        _ => (None, None),
    };

    let context = SkillRunContext {
        active_document_id,
        active_document_path,
        selected_text: None,
        provider: None,
    };
    let payload = RunSkillPayload { inputs, context };
    let inputs_json = serde_json::to_string(&payload.inputs)
        .map_err(|error| format!("序列化技能输入失败：{error}"))?;
    let context_json = serde_json::to_string(&payload.context)
        .map_err(|error| format!("序列化技能上下文失败：{error}"))?;

    let skill = get_science_skill_from_vendor(&id).or_else(|_| {
        get_cached_skill(&connection, &id)?.ok_or_else(|| "找不到这个科学技能。".to_string())
    })?;
    persist_skill_cache(&connection, &skill)?;

    let task = payload
        .inputs
        .get("task")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim();
    let now = now_string();
    let mut run = SkillRun {
        id: Uuid::new_v4().to_string(),
        skill_id: skill.id.clone(),
        skill_name: skill.name.clone(),
        status: "running".to_string(),
        started_at: now,
        finished_at: None,
        error: None,
        logs: vec![make_skill_log("info", "技能运行已进入受控 dry-run。")],
        outputs: Vec::new(),
    };

    if task.is_empty() {
        run.status = "failed".to_string();
        run.finished_at = Some(now_string());
        run.error = Some("任务上下文不能为空。".to_string());
        run.logs
            .push(make_skill_log("error", "任务上下文不能为空。"));
    } else if skill.status != "可用" {
        let message = match skill.status.as_str() {
            "缺少依赖" => "该技能缺少本地依赖。请先安装 uv 或按技能说明补齐依赖。".to_string(),
            "需要配置" => format!(
                "该技能需要先配置环境变量：{}。",
                skill.required_env.join("、")
            ),
            _ => "该技能当前不可用。".to_string(),
        };
        run.status = "failed".to_string();
        run.finished_at = Some(now_string());
        run.error = Some(message.clone());
        run.logs.push(make_skill_log("error", &message));
    } else {
        run.outputs.push(SkillRunOutput {
            id: Uuid::new_v4().to_string(),
            kind: "markdown".to_string(),
            title: "技能 dry-run 执行计划".to_string(),
            content: build_skill_dry_run_output(
                &skill,
                &payload.inputs,
                payload.context.active_document_id.as_deref(),
                payload.context.active_document_path.as_deref(),
            ),
            file_path: None,
        });
        run.status = "succeeded".to_string();
        run.finished_at = Some(now_string());
        run.logs
            .push(make_skill_log("info", "技能 dry-run 已完成。"));
    }

    persist_skill_run(&connection, &run, &inputs_json, &context_json)?;
    Ok(run)
}

#[tauri::command]
fn get_skill_run(app: AppHandle, id: String) -> Result<SkillRun, String> {
    ensure_storage(&app)?;
    let connection = open_connection(&app)?;
    get_skill_run_from_connection(&connection, &id)
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
            get_provider_presets,
            load_development_api_key,
            save_provider_settings,
            test_provider_connection,
            get_document_pdf_bytes,
            update_reading_state,
            index_document,
            ask_document,
            list_skills,
            get_skill,
            run_skill,
            get_skill_run,
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
