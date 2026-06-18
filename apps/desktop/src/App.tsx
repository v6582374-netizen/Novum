import { invoke } from '@tauri-apps/api/core'
import { appDataDir, join } from '@tauri-apps/api/path'
import { open } from '@tauri-apps/plugin-dialog'
import { Stronghold, type Store } from '@tauri-apps/plugin-stronghold'
import {
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
  type UIEvent as ReactUIEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import {
  BookOpen,
  BrainCircuit,
  ChevronLeft,
  ChevronRight,
  Command,
  FileSearch,
  FileText,
  FlaskConical,
  Library,
  Loader2,
  MessageSquareText,
  PanelRight,
  Play,
  Search,
  Settings,
  Sparkles,
  Trash2,
  Upload,
  ZoomIn,
  ZoomOut,
} from 'lucide-react'
import * as pdfjsLib from 'pdfjs-dist'
import type { PDFDocumentProxy } from 'pdfjs-dist'
import pdfWorker from 'pdfjs-dist/build/pdf.worker.min.mjs?url'
import './App.css'

pdfjsLib.GlobalWorkerOptions.workerSrc = pdfWorker

type PanelId = 'library' | 'qa' | 'skills' | 'settings'

type DocumentRecord = {
  id: string
  title: string
  fileName: string
  storedPath: string
  fingerprint: string
  pageCount: number
  status: string
  createdAt: string
  updatedAt: string
  lastOpenedPage: number
  lastZoom: number
}

type PdfBytes = {
  documentId: string
  fileName: string
  bytes: number[]
}

type ProviderKind = 'openai-compatible' | 'deepseek' | 'test-relay'

type ProviderSettings = {
  provider: ProviderKind
  baseUrl: string
  model: string
  hasApiKey: boolean
  updatedAt: string | null
}

type ProviderPreset = {
  provider: ProviderKind
  label: string
  description: string
  baseUrl: string
  model: string
  hasDevelopmentKey: boolean
}

type ProviderConnectionResult = {
  ok: boolean
  message: string
  checkedAt: string
}

type ResearchRun = {
  id: string
  kind: string
  status: string
  documentId: string
  startedAt: string
  finishedAt: string | null
  error: string | null
  logs: Array<{
    timestamp: string
    level: string
    message: string
  }>
}

type QaCitation = {
  id: string
  documentId: string
  title: string
  page: number | null
  excerpt: string
  sourceLabel: string
  confidence: number | null
}

type QaAnswer = {
  id: string
  documentId: string
  question: string
  answer: string
  citations: QaCitation[]
  createdAt: string
}

type AskDocumentResult = {
  run: ResearchRun
  answer: QaAnswer
}

type SkillInputSpec = {
  name: string
  label: string
  type: 'text' | 'textarea' | 'file' | 'select' | 'number' | 'boolean'
  required: boolean
  defaultValue: string | number | boolean | null
  help: string | null
}

type ScienceSkill = {
  id: string
  name: string
  description: string
  domain: string
  source: 'science-skills'
  sourcePath: string
  upstreamCommit: string
  requiredInputs: SkillInputSpec[]
  requiredEnv: string[]
  executionMode: 'python' | 'prompt' | 'hybrid'
  status: '可用' | '缺少依赖' | '需要配置' | '不可用'
  updatedAt: string
}

type SkillRun = {
  id: string
  skillId: string
  skillName: string
  status: 'queued' | 'running' | 'succeeded' | 'failed'
  startedAt: string
  finishedAt: string | null
  error: string | null
  logs: Array<{
    timestamp: string
    level: 'info' | 'warning' | 'error'
    message: string
  }>
  outputs: Array<{
    id: string
    kind: 'markdown' | 'json' | 'file' | 'text'
    title: string
    content: string
    filePath: string | null
  }>
}

const DEFAULT_PDF_PANE_WIDTH = 520
const MIN_PDF_PANE_WIDTH = 420
const MAX_PDF_PANE_WIDTH = 900
const STRONGHOLD_PASSWORD = 'novum-phase3-local-vault'
const STRONGHOLD_CLIENT = 'novum'
const LEGACY_PROVIDER_API_KEY = 'openai-compatible-api-key'
const PROVIDER_API_KEY_PREFIX = 'provider-api-key:'

const panels: Array<{
  id: PanelId
  label: string
  icon: typeof Library
}> = [
  { id: 'library', label: '文献库', icon: Library },
  { id: 'qa', label: '论文问答', icon: BrainCircuit },
  { id: 'skills', label: '技能市场', icon: FlaskConical },
  { id: 'settings', label: '模型配置', icon: Settings },
]

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

function uniquePages(pages: number[], pageCount: number) {
  return Array.from(
    new Set(pages.map((page) => clamp(page, 1, pageCount)).filter(Boolean)),
  )
}

function cleanupPdfDocument(document: PDFDocumentProxy | null) {
  void document?.cleanup()
}

async function withCredentialStore<T>(
  action: (store: Store) => Promise<T>,
) {
  const { stronghold, client } = await getCredentialClient()
  try {
    return await action(client.getStore())
  } finally {
    await stronghold.save()
    await stronghold.unload()
  }
}

async function getCredentialClient() {
  const dataDir = await appDataDir()
  const vaultPath = await join(dataDir, 'novum-credentials.stronghold')
  const stronghold = await Stronghold.load(vaultPath, STRONGHOLD_PASSWORD)
  try {
    return { stronghold, client: await stronghold.loadClient(STRONGHOLD_CLIENT) }
  } catch {
    return { stronghold, client: await stronghold.createClient(STRONGHOLD_CLIENT) }
  }
}

function providerApiKeyName(provider: ProviderKind) {
  return `${PROVIDER_API_KEY_PREFIX}${provider}`
}

async function saveProviderApiKey(provider: ProviderKind, apiKey: string) {
  const trimmed = apiKey.trim()
  if (!trimmed) return

  const encoded = Array.from(new TextEncoder().encode(trimmed))
  await withCredentialStore(async (store) => {
    await store.insert(providerApiKeyName(provider), encoded)
  })
}

async function readProviderApiKey(provider: ProviderKind) {
  return withCredentialStore(async (store) => {
    const value = await store.get(providerApiKeyName(provider))
    if (!value && provider === 'openai-compatible') {
      const legacyValue = await store.get(LEGACY_PROVIDER_API_KEY)
      return legacyValue ? new TextDecoder().decode(legacyValue) : ''
    }
    return value ? new TextDecoder().decode(value) : ''
  })
}

type PdfPageCanvasProps = {
  active: boolean
  pageNumber: number
  pdfDocument: PDFDocumentProxy
  stageWidth: number
  zoom: number
  onRenderError: (message: string) => void
}

function PdfPageCanvas({
  active,
  pageNumber,
  pdfDocument,
  stageWidth,
  zoom,
  onRenderError,
}: PdfPageCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)

  useEffect(() => {
    let cancelled = false
    let renderTask: { promise: Promise<unknown>; cancel: () => void } | null = null

    async function renderPage() {
      const canvas = canvasRef.current
      if (!canvas) return

      const page = await pdfDocument.getPage(pageNumber)
      if (cancelled || !canvasRef.current) return

      const context = canvas.getContext('2d')
      if (!context) return

      const baseViewport = page.getViewport({ scale: 1 })
      const availableWidth = Math.max(280, stageWidth || DEFAULT_PDF_PANE_WIDTH)
      const fitScale = clamp(availableWidth / baseViewport.width, 0.35, 1.6)
      const viewport = page.getViewport({ scale: fitScale * (zoom / 100) })
      const devicePixelRatio = window.devicePixelRatio || 1

      canvas.width = Math.floor(viewport.width * devicePixelRatio)
      canvas.height = Math.floor(viewport.height * devicePixelRatio)
      canvas.style.width = `${viewport.width}px`
      canvas.style.height = `${viewport.height}px`

      context.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0)
      renderTask = page.render({ canvas, canvasContext: context, viewport })
      await renderTask.promise
    }

    void renderPage().catch((reason) => {
      const message = String(reason)
      if (message.includes('RenderingCancelledException')) return
      if (!cancelled) {
        onRenderError(message)
      }
    })

    return () => {
      cancelled = true
      renderTask?.cancel()
    }
  }, [onRenderError, pageNumber, pdfDocument, stageWidth, zoom])

  return (
    <article
      className={`pdf-page-shell ${active ? 'active' : ''}`}
      data-page={pageNumber}
      aria-label={`第 ${pageNumber} 页`}
    >
      <canvas className="pdf-canvas" ref={canvasRef} />
      <span className="pdf-page-label">第 {pageNumber} 页</span>
    </article>
  )
}

function App() {
  const pdfStageRef = useRef<HTMLDivElement | null>(null)
  const currentPageRef = useRef(1)
  const selectedDocumentIdRef = useRef<string | null>(null)
  const pdfResizeActiveRef = useRef(false)
  const [activePanel, setActivePanel] = useState<PanelId>('library')
  const [documents, setDocuments] = useState<DocumentRecord[]>([])
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(null)
  const [documentQuery, setDocumentQuery] = useState('')
  const [skills, setSkills] = useState<ScienceSkill[]>([])
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null)
  const [skillQuery, setSkillQuery] = useState('')
  const [skillDomain, setSkillDomain] = useState('全部')
  const [skillInputs, setSkillInputs] = useState<Record<string, string>>({ task: '' })
  const [latestSkillRun, setLatestSkillRun] = useState<SkillRun | null>(null)
  const [currentPage, setCurrentPage] = useState(1)
  const [zoom, setZoom] = useState(100)
  const [pdfDocument, setPdfDocument] = useState<PDFDocumentProxy | null>(null)
  const [isLoadingLibrary, setIsLoadingLibrary] = useState(true)
  const [isLoadingSkills, setIsLoadingSkills] = useState(true)
  const [isImporting, setIsImporting] = useState(false)
  const [isLoadingPdf, setIsLoadingPdf] = useState(false)
  const [isRunningSkill, setIsRunningSkill] = useState(false)
  const [isPdfCollapsed, setIsPdfCollapsed] = useState(false)
  const [isResizingPdf, setIsResizingPdf] = useState(false)
  const [pdfPaneWidth, setPdfPaneWidth] = useState(DEFAULT_PDF_PANE_WIDTH)
  const [pdfStageWidth, setPdfStageWidth] = useState(DEFAULT_PDF_PANE_WIDTH)
  const [isCommandOpen, setIsCommandOpen] = useState(false)
  const [selectedCitationPage, setSelectedCitationPage] = useState(1)
  const [providerSettings, setProviderSettings] = useState<ProviderSettings | null>(null)
  const [providerPresets, setProviderPresets] = useState<ProviderPreset[]>([])
  const [providerForm, setProviderForm] = useState({
    provider: 'openai-compatible' as ProviderKind,
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-4o-mini',
    apiKey: '',
  })
  const [isSavingProvider, setIsSavingProvider] = useState(false)
  const [isTestingProvider, setIsTestingProvider] = useState(false)
  const [isIndexingDocument, setIsIndexingDocument] = useState(false)
  const [isAskingDocument, setIsAskingDocument] = useState(false)
  const [paperQuestion, setPaperQuestion] = useState('')
  const [latestAnswer, setLatestAnswer] = useState<QaAnswer | null>(null)
  const [activeCitationId, setActiveCitationId] = useState<string | null>(null)
  const [latestRun, setLatestRun] = useState<ResearchRun | null>(null)
  const [message, setMessage] = useState('正在读取本地文献库。')
  const [error, setError] = useState<string | null>(null)

  const selectedDocument = useMemo(
    () => documents.find((document) => document.id === selectedDocumentId) ?? null,
    [documents, selectedDocumentId],
  )
  const selectedSkill = useMemo(
    () => skills.find((skill) => skill.id === selectedSkillId) ?? skills[0] ?? null,
    [selectedSkillId, skills],
  )
  const visibleSkillRun = latestSkillRun?.skillId === selectedSkill?.id ? latestSkillRun : null
  const isProviderReady = Boolean(providerSettings?.hasApiKey)
  const pdfPageCount = pdfDocument?.numPages ?? selectedDocument?.pageCount ?? 0
  const pdfPageNumbers = useMemo(
    () => Array.from({ length: pdfPageCount }, (_, index) => index + 1),
    [pdfPageCount],
  )
  const appShellStyle = {
    '--pdf-pane-width': `${pdfPaneWidth}px`,
  } as CSSProperties

  useEffect(() => {
    selectedDocumentIdRef.current = selectedDocumentId
  }, [selectedDocumentId])

  useEffect(() => {
    currentPageRef.current = currentPage
  }, [currentPage])

  const reportPdfRenderError = useCallback((renderError: string) => {
    setError(renderError)
    setMessage('渲染 PDF 页面失败。')
  }, [])

  const scrollPdfToPage = useCallback((page: number, behavior: ScrollBehavior = 'smooth') => {
    window.requestAnimationFrame(() => {
      const stage = pdfStageRef.current
      const target = stage?.querySelector<HTMLElement>(`[data-page="${page}"]`)
      if (!stage || !target) return

      const stageRect = stage.getBoundingClientRect()
      const targetRect = target.getBoundingClientRect()
      stage.scrollTo({
        top: stage.scrollTop + targetRect.top - stageRect.top,
        behavior,
      })
    })
  }, [])

  const panelTitle = useMemo(() => {
    if (activePanel === 'library') return '文献库'
    if (activePanel === 'qa') return '论文问答'
    if (activePanel === 'skills') return '技能市场'
    return '模型配置'
  }, [activePanel])

  const citationPages = useMemo(() => {
    if (!selectedDocument) return []
    return uniquePages(
      [currentPage, currentPage + 2, selectedDocument.pageCount],
      selectedDocument.pageCount,
    )
  }, [currentPage, selectedDocument])

  const filteredDocuments = useMemo(() => {
    const query = documentQuery.trim().toLocaleLowerCase()
    if (!query) return documents

    return documents.filter((document) => {
      const searchableText = [
        document.title,
        document.fileName,
        document.status,
        `${document.pageCount}`,
      ]
        .join(' ')
        .toLocaleLowerCase()

      return searchableText.includes(query)
    })
  }, [documentQuery, documents])

  const skillDomains = useMemo(
    () => ['全部', ...Array.from(new Set(skills.map((skill) => skill.domain))).sort()],
    [skills],
  )

  const filteredSkills = useMemo(() => {
    const query = skillQuery.trim().toLocaleLowerCase()
    return skills.filter((skill) => {
      const matchesDomain = skillDomain === '全部' || skill.domain === skillDomain
      const searchableText = [
        skill.name,
        skill.id,
        skill.description,
        skill.domain,
        skill.status,
        skill.executionMode,
      ]
        .join(' ')
        .toLocaleLowerCase()
      return matchesDomain && (!query || searchableText.includes(query))
    })
  }, [skillDomain, skillQuery, skills])

  const selectDocumentState = useCallback((document: DocumentRecord | null) => {
    setSelectedDocumentId(document?.id ?? null)
    if (!document) {
      setCurrentPage(1)
      setSelectedCitationPage(1)
      setZoom(100)
      return
    }

    const nextPage = clamp(document.lastOpenedPage || 1, 1, document.pageCount)
    setCurrentPage(nextPage)
    setSelectedCitationPage(nextPage)
    setZoom(clamp(document.lastZoom || 100, 60, 100))
  }, [])

  const loadDocuments = useCallback(async (preferredDocumentId?: string | null) => {
    setIsLoadingLibrary(true)
    setError(null)
    try {
      const library = await invoke<DocumentRecord[]>('list_documents')
      const targetId = preferredDocumentId ?? selectedDocumentIdRef.current
      const nextDocument =
        library.find((document) => document.id === targetId) ?? library[0] ?? null

      setDocuments(library)
      selectDocumentState(nextDocument)
      setMessage(library.length ? '已载入本地文献库。' : '文献库为空，请先导入 PDF。')
    } catch (reason) {
      setError(String(reason))
      setMessage('读取本地文献库失败。')
    } finally {
      setIsLoadingLibrary(false)
    }
  }, [selectDocumentState])

  const loadSkills = useCallback(async (preferredSkillId?: string | null) => {
    setIsLoadingSkills(true)
    setError(null)
    try {
      const registry = await invoke<ScienceSkill[]>('list_skills')
      setSkills(registry)
      setSelectedSkillId((current) => {
        const targetId = preferredSkillId ?? current
        return registry.find((skill) => skill.id === targetId)?.id ?? registry[0]?.id ?? null
      })
      setMessage(registry.length ? '已载入 Science Skills 注册表。' : '技能注册表为空。')
    } catch (reason) {
      setError(String(reason))
      setMessage('读取科学技能注册表失败。')
    } finally {
      setIsLoadingSkills(false)
    }
  }, [])

  useEffect(() => {
    void Promise.resolve().then(() => loadDocuments())
  }, [loadDocuments])

  useEffect(() => {
    void Promise.resolve().then(() => loadSkills())
  }, [loadSkills])

  useEffect(() => {
    async function loadProviderSettings() {
      try {
        const [settings, presets] = await Promise.all([
          invoke<ProviderSettings>('get_provider_settings'),
          invoke<ProviderPreset[]>('get_provider_presets'),
        ])
        setProviderPresets(presets)
        setProviderSettings(settings)
        setProviderForm((current) => ({
          ...current,
          provider: settings.provider,
          baseUrl: settings.baseUrl,
          model: settings.model,
          apiKey: '',
        }))
      } catch (reason) {
        setError(String(reason))
      }
    }

    void loadProviderSettings()
  }, [])

  useEffect(() => {
    if (isPdfCollapsed) return

    const stage = pdfStageRef.current
    if (!stage) return

    const observer = new ResizeObserver(([entry]) => {
      setPdfStageWidth(entry.contentRect.width)
    })

    observer.observe(stage)
    setPdfStageWidth(Math.max(280, stage.clientWidth - 44))

    return () => observer.disconnect()
  }, [isPdfCollapsed])

  useEffect(() => {
    let cancelled = false
    let loadingTask: ReturnType<typeof pdfjsLib.getDocument> | null = null

    async function loadPdf() {
      if (!selectedDocumentId) {
        setPdfDocument((previous) => {
          cleanupPdfDocument(previous)
          return null
        })
        return
      }

      setIsLoadingPdf(true)
      setError(null)
      try {
        const payload = await invoke<PdfBytes>('get_document_pdf_bytes', {
          id: selectedDocumentId,
        })
        if (cancelled) return

        loadingTask = pdfjsLib.getDocument({
          data: Uint8Array.from(payload.bytes),
        })
        const loadedPdf = await loadingTask.promise
        if (cancelled) {
          cleanupPdfDocument(loadedPdf)
          return
        }

        setPdfDocument((previous) => {
          if (previous !== loadedPdf) {
            cleanupPdfDocument(previous)
          }
          return loadedPdf
        })
      } catch (reason) {
        if (!cancelled) {
          setError(String(reason))
          setPdfDocument((previous) => {
            cleanupPdfDocument(previous)
            return null
          })
          setMessage('加载 PDF 失败。')
        }
      } finally {
        if (!cancelled) {
          setIsLoadingPdf(false)
        }
      }
    }

    void loadPdf()
    return () => {
      cancelled = true
      void loadingTask?.destroy()
    }
  }, [selectedDocumentId])

  useEffect(() => {
    if (!pdfDocument || isLoadingPdf || isPdfCollapsed) return

    const timeout = window.setTimeout(() => {
      scrollPdfToPage(currentPageRef.current, 'auto')
    }, 120)

    return () => window.clearTimeout(timeout)
  }, [isLoadingPdf, isPdfCollapsed, pdfDocument, scrollPdfToPage, selectedDocumentId])

  useEffect(() => {
    if (!selectedDocumentId) return

    const timeout = window.setTimeout(() => {
      invoke<DocumentRecord>('update_reading_state', {
        id: selectedDocumentId,
        page: currentPage,
        zoom,
      })
        .then((updated) => {
          setDocuments((current) =>
            current.map((document) => (document.id === updated.id ? updated : document)),
          )
        })
        .catch((reason) => {
          setError(String(reason))
        })
    }, 350)

    return () => window.clearTimeout(timeout)
  }, [currentPage, selectedDocumentId, zoom])

  function selectPanel(panel: PanelId) {
    setActivePanel(panel)
    setIsCommandOpen(false)
    setMessage(`已切换到「${panels.find((item) => item.id === panel)?.label}」。`)
  }

  function selectDocument(document: DocumentRecord) {
    selectDocumentState(document)
    setActivePanel('library')
    setLatestAnswer(null)
    setActiveCitationId(null)
    setMessage(`已打开文献「${document.title}」。`)
  }

  async function importPdf() {
    setIsImporting(true)
    setError(null)
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'PDF 文献', extensions: ['pdf'] }],
      })

      if (!selected || Array.isArray(selected)) {
        setMessage('已取消导入。')
        return
      }

      const imported = await invoke<DocumentRecord>('import_pdf_from_path', {
        path: selected,
      })
      await loadDocuments(imported.id)
      setActivePanel('library')
      setMessage(`已导入文献「${imported.title}」。`)
    } catch (reason) {
      setError(String(reason))
      setMessage('导入 PDF 失败。')
    } finally {
      setIsImporting(false)
    }
  }

  async function deleteDocument(document: DocumentRecord) {
    const confirmed = window.confirm(`确定要删除「${document.title}」吗？此操作会移除本地 PDF 副本。`)
    if (!confirmed) return

    setError(null)
    try {
      await invoke<boolean>('delete_document', { id: document.id })
      const remainingDocuments = documents.filter((item) => item.id !== document.id)
      const currentIndex = documents.findIndex((item) => item.id === document.id)
      const nextDocument =
        remainingDocuments[currentIndex] ?? remainingDocuments[currentIndex - 1] ?? null

      if (document.id === selectedDocumentId) {
        setPdfDocument((previous) => {
          cleanupPdfDocument(previous)
          return null
        })
      }

      await loadDocuments(nextDocument?.id ?? null)
      setMessage(`已删除文献「${document.title}」。`)
    } catch (reason) {
      setError(String(reason))
      setMessage('删除文献失败。')
    }
  }

  async function deleteSelectedDocument() {
    if (!selectedDocument) return
    await deleteDocument(selectedDocument)
  }

  async function persistProviderSettings(showSuccess = true) {
    const apiKey = providerForm.apiKey.trim()
    if (apiKey) {
      await saveProviderApiKey(providerForm.provider, apiKey)
    }

    const settings = await invoke<ProviderSettings>('save_provider_settings', {
      settings: {
        provider: providerForm.provider,
        baseUrl: providerForm.baseUrl,
        model: providerForm.model,
        hasApiKey: Boolean(apiKey || providerSettings?.hasApiKey),
      },
    })
    setProviderSettings(settings)
    setProviderForm((current) => ({ ...current, apiKey: '' }))
    if (showSuccess) {
      setMessage('模型配置已保存。')
    }
    return settings
  }

  async function saveProviderSettings() {
    setIsSavingProvider(true)
    setError(null)
    try {
      await persistProviderSettings()
    } catch (reason) {
      setError(String(reason))
      setMessage('保存模型配置失败。')
    } finally {
      setIsSavingProvider(false)
    }
  }

  function applyProviderPreset(preset: ProviderPreset) {
    setProviderForm((current) => ({
      ...current,
      provider: preset.provider,
      baseUrl: preset.baseUrl,
      model: preset.model,
      apiKey: '',
    }))
    setMessage(`已切换到「${preset.label}」配置。`)
  }

  async function importDevelopmentApiKey() {
    setError(null)
    try {
      const apiKey = await invoke<string | null>('load_development_api_key', {
        provider: providerForm.provider,
      })
      if (!apiKey) {
        setMessage('未找到本机测试密钥。请使用环境变量或 secrets/ 本地文件。')
        return
      }
      await saveProviderApiKey(providerForm.provider, apiKey)
      const settings = await invoke<ProviderSettings>('save_provider_settings', {
        settings: {
          provider: providerForm.provider,
          baseUrl: providerForm.baseUrl,
          model: providerForm.model,
          hasApiKey: true,
        },
      })
      setProviderSettings(settings)
      setProviderForm((current) => ({ ...current, apiKey: '' }))
      const presets = await invoke<ProviderPreset[]>('get_provider_presets')
      setProviderPresets(presets)
      setMessage('已从本机安全位置导入测试密钥。')
    } catch (reason) {
      setError(String(reason))
      setMessage('导入本机测试密钥失败。')
    }
  }

  async function testProviderConnection() {
    setIsTestingProvider(true)
    setError(null)
    try {
      await persistProviderSettings(false)
      const apiKey = providerForm.apiKey.trim() || await readProviderApiKey(providerForm.provider)
      const result = await invoke<ProviderConnectionResult>('test_provider_connection', {
        apiKey,
      })
      setMessage(result.message)
      if (!result.ok) {
        setError(result.message)
      }
    } catch (reason) {
      setError(String(reason))
      setMessage('模型服务连接测试失败。')
    } finally {
      setIsTestingProvider(false)
    }
  }

  async function indexSelectedDocument() {
    if (!selectedDocument) {
      setMessage('请先导入并选择一篇 PDF 文献。')
      return
    }

    setIsIndexingDocument(true)
    setError(null)
    setActivePanel('qa')
    try {
      const apiKey = await readProviderApiKey(providerSettings?.provider ?? providerForm.provider)
      const run = await invoke<ResearchRun>('index_document', {
        id: selectedDocument.id,
        apiKey,
      })
      setLatestRun(run)
      await loadDocuments(selectedDocument.id)
      setMessage('PaperQA 索引已完成，可以开始提问。')
    } catch (reason) {
      setError(String(reason))
      setMessage('PaperQA 索引失败。')
      await loadDocuments(selectedDocument.id)
    } finally {
      setIsIndexingDocument(false)
    }
  }

  async function askSelectedDocument() {
    if (!selectedDocument) {
      setMessage('请先导入并选择一篇 PDF 文献。')
      return
    }

    setIsAskingDocument(true)
    setError(null)
    setActivePanel('qa')
    try {
      const apiKey = await readProviderApiKey(providerSettings?.provider ?? providerForm.provider)
      const result = await invoke<AskDocumentResult>('ask_document', {
        id: selectedDocument.id,
        question: paperQuestion,
        apiKey,
      })
      setLatestRun(result.run)
      setLatestAnswer(result.answer)
      setActiveCitationId(result.answer.citations[0]?.id ?? null)
      setMessage('PaperQA 已返回真实问答结果。')
    } catch (reason) {
      setError(String(reason))
      setMessage('PaperQA 问答失败。')
    } finally {
      setIsAskingDocument(false)
    }
  }

  function followCitation(citation: QaCitation) {
    setActiveCitationId(citation.id)
    if (!citation.page) {
      setMessage('这条引用暂未返回可跳转页码。')
      return
    }
    jumpToPage(citation.page)
    setMessage(`已跳转到引用来源第 ${citation.page} 页。`)
  }

  function runPaperQa() {
    setActivePanel('qa')
    if (!selectedDocument) {
      setMessage('请先导入并选择一篇 PDF 文献。')
      return
    }
    if (selectedDocument.status === '已索引') {
      setMessage('请输入问题并发起 PaperQA 问答。')
      return
    }
    setMessage('请先为当前文献建立 PaperQA 索引。')
  }

  function selectSkill(skill: ScienceSkill) {
    setSelectedSkillId(skill.id)
    setActivePanel('skills')
    setMessage(`已选中技能「${skill.name}」。`)
  }

  async function runSkill() {
    setActivePanel('skills')
    if (!selectedSkill) {
      setMessage('请先选择一个科学技能。')
      return
    }
    if (!skillInputs.task?.trim()) {
      setMessage('请先填写技能任务上下文。')
      return
    }

    setIsRunningSkill(true)
    setError(null)
    try {
      const run = await invoke<SkillRun>('run_skill', {
        id: selectedSkill.id,
        inputs: skillInputs,
        activeDocumentId: selectedDocument?.id ?? null,
      })
      setLatestSkillRun(run)
      if (run.status === 'failed') {
        setError(run.error ?? '技能运行失败。')
        setMessage('技能运行失败。')
      } else {
        setMessage(`技能「${selectedSkill.name}」dry-run 已完成。`)
      }
    } catch (reason) {
      setError(String(reason))
      setMessage('技能运行失败。')
    } finally {
      setIsRunningSkill(false)
    }
  }

  function jumpToPage(page: number) {
    if (!selectedDocument) return
    const nextPage = clamp(page, 1, pdfPageCount || selectedDocument.pageCount)
    setCurrentPage(nextPage)
    setSelectedCitationPage(nextPage)
    setMessage(`已跳转到第 ${nextPage} 页。`)
    scrollPdfToPage(nextPage)
  }

  function adjustZoom(delta: number) {
    setZoom((current) => {
      const next = clamp(current + delta, 60, 100)
      setMessage(`PDF 缩放已调整为 ${next}%。`)
      return next
    })
  }

  function handlePdfScroll(event: ReactUIEvent<HTMLDivElement>) {
    const stage = event.currentTarget
    const pages = Array.from(stage.querySelectorAll<HTMLElement>('.pdf-page-shell'))
    if (!pages.length) return

    const stageRect = stage.getBoundingClientRect()
    const viewportCenter = stageRect.top + stageRect.height / 2
    const closestPage = pages.reduce(
      (closest, page) => {
        const pageRect = page.getBoundingClientRect()
        const distance = Math.abs(pageRect.top + pageRect.height / 2 - viewportCenter)
        return distance < closest.distance
          ? { distance, pageNumber: Number(page.dataset.page) }
          : closest
      },
      { distance: Number.POSITIVE_INFINITY, pageNumber: currentPageRef.current },
    ).pageNumber

    if (!Number.isFinite(closestPage) || closestPage === currentPageRef.current) return

    currentPageRef.current = closestPage
    setCurrentPage(closestPage)
    setSelectedCitationPage(closestPage)
  }

  function startPdfResize(event: ReactPointerEvent<HTMLDivElement>) {
    if (isPdfCollapsed) return

    pdfResizeActiveRef.current = true
    setIsResizingPdf(true)
    event.currentTarget.setPointerCapture(event.pointerId)
    event.preventDefault()
  }

  function resizePdfPane(event: ReactPointerEvent<HTMLDivElement>) {
    if (!pdfResizeActiveRef.current || isPdfCollapsed) return

    const maxWidth = clamp(
      window.innerWidth - 292 - 360 - 8,
      MIN_PDF_PANE_WIDTH,
      MAX_PDF_PANE_WIDTH,
    )
    setPdfPaneWidth(clamp(window.innerWidth - event.clientX, MIN_PDF_PANE_WIDTH, maxWidth))
  }

  function stopPdfResize(event: ReactPointerEvent<HTMLDivElement>) {
    if (!pdfResizeActiveRef.current) return

    pdfResizeActiveRef.current = false
    setIsResizingPdf(false)
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  return (
    <main
      className={`app-shell ${isPdfCollapsed ? 'pdf-collapsed' : ''} ${
        isResizingPdf ? 'is-resizing-pdf' : ''
      }`}
      style={appShellStyle}
    >
      <aside className="sidebar" aria-label="科研工作区导航">
        <div className="brand-row">
          <div className="brand-mark">N</div>
          <div>
            <p className="eyebrow">Novum</p>
            <h1>科研工作台</h1>
          </div>
        </div>

        <button
          className={`command-button ${isCommandOpen ? 'active' : ''}`}
          type="button"
          onClick={() => {
            setIsCommandOpen((value) => !value)
            setMessage('命令面板已切换。')
          }}
        >
          <Command size={16} aria-hidden="true" />
          <span>命令面板</span>
          <kbd>Cmd K</kbd>
        </button>

        <nav className="nav-stack">
          {panels.map((panel) => {
            const Icon = panel.icon

            return (
              <button
                className={`nav-item ${activePanel === panel.id ? 'active' : ''}`}
                key={panel.id}
                type="button"
                onClick={() => selectPanel(panel.id)}
              >
                <Icon size={16} aria-hidden="true" />
                {panel.label}
              </button>
            )
          })}
        </nav>

        <section className="sidebar-section" id="library">
          <div className="section-heading">
            <span>当前文献</span>
            <button
              className="icon-button"
              type="button"
              title="导入 PDF"
              onClick={() => void importPdf()}
              disabled={isImporting}
            >
              {isImporting ? (
                <Loader2 className="spin" size={15} aria-hidden="true" />
              ) : (
                <Upload size={15} aria-hidden="true" />
              )}
            </button>
          </div>

          <label className="library-search">
            <Search size={14} aria-hidden="true" />
            <input
              type="search"
              value={documentQuery}
              placeholder="查找标题、文件名或状态"
              aria-label="查找文献"
              onChange={(event) => setDocumentQuery(event.target.value)}
            />
          </label>

          <div className="paper-list">
            {isLoadingLibrary ? (
              <div className="empty-state compact">正在读取文献库...</div>
            ) : documents.length ? (
              filteredDocuments.length ? (
                filteredDocuments.map((document) => (
                  <div
                    className={`paper-item ${
                      document.id === selectedDocumentId ? 'selected' : ''
                    }`}
                    key={document.id}
                  >
                    <button
                      className={`paper-row ${
                        document.id === selectedDocumentId ? 'selected' : ''
                      }`}
                      type="button"
                      onClick={() => selectDocument(document)}
                    >
                      <BookOpen size={15} aria-hidden="true" />
                      <span>
                        <strong>{document.title}</strong>
                        <small>
                          {document.pageCount} 页 | {document.status} | 第{' '}
                          {document.lastOpenedPage} 页
                        </small>
                      </span>
                    </button>
                    <button
                      className="row-delete-button"
                      type="button"
                      title={`删除「${document.title}」`}
                      aria-label={`删除「${document.title}」`}
                      onClick={() => void deleteDocument(document)}
                    >
                      <Trash2 size={14} aria-hidden="true" />
                    </button>
                  </div>
                ))
              ) : (
                <div className="empty-state compact">没有找到匹配文献。</div>
              )
            ) : (
              <button className="empty-state compact" type="button" onClick={() => void importPdf()}>
                文献库为空，点击导入 PDF。
              </button>
            )}
          </div>
        </section>

        <section className="sidebar-section" id="skills">
          <div className="section-heading">
            <span>科学技能</span>
            <button
              className="icon-button"
              type="button"
              title="刷新技能"
              onClick={() => void loadSkills(selectedSkill?.id)}
              disabled={isLoadingSkills}
            >
              {isLoadingSkills ? (
                <Loader2 className="spin" size={15} aria-hidden="true" />
              ) : (
                <Search size={15} aria-hidden="true" />
              )}
            </button>
          </div>
          <label className="library-search">
            <Search size={14} aria-hidden="true" />
            <input
              type="search"
              value={skillQuery}
              placeholder="查找技能、领域或状态"
              aria-label="查找科学技能"
              onChange={(event) => setSkillQuery(event.target.value)}
            />
          </label>
          <label className="library-search">
            <FlaskConical size={14} aria-hidden="true" />
            <select
              value={skillDomain}
              aria-label="按领域筛选技能"
              onChange={(event) => setSkillDomain(event.target.value)}
            >
              {skillDomains.map((domain) => (
                <option key={domain} value={domain}>
                  {domain}
                </option>
              ))}
            </select>
          </label>
          <div className="skill-list">
            {isLoadingSkills ? (
              <div className="empty-state compact">正在读取技能注册表...</div>
            ) : filteredSkills.length ? (
              filteredSkills.map((skill) => (
                <button
                  className={`skill-row ${skill.id === selectedSkill?.id ? 'selected' : ''}`}
                  key={skill.id}
                  type="button"
                  onClick={() => selectSkill(skill)}
                >
                  <span>
                    <strong>{skill.name}</strong>
                    <small>
                      {skill.domain} | {skill.status} | {skill.executionMode}
                    </small>
                  </span>
                  <ChevronRight size={15} aria-hidden="true" />
                </button>
              ))
            ) : (
              <div className="empty-state compact">没有找到匹配技能。</div>
            )}
          </div>
        </section>
      </aside>

      <section className="workspace" aria-label="论文工作台">
        <header className="workspace-topbar">
          <div>
            <p className="eyebrow">长期科研模式</p>
            <h2>{panelTitle}</h2>
          </div>
          <div className="status-strip" aria-label="本地运行状态">
            <span>{documents.length ? `本地文献 ${documents.length} 篇` : '文献库为空'}</span>
            <span>{isProviderReady ? '模型已配置' : '模型尚未配置'}</span>
            <span>PaperQA 本地服务</span>
          </div>
        </header>

        <section className="query-panel" id="paperqa" aria-live="polite">
          <div className="query-input">
            <FileSearch size={18} aria-hidden="true" />
            <span>
              {selectedDocument
                ? `当前文献：${selectedDocument.title}`
                : '请先导入 PDF，再围绕文献发起问题。'}
            </span>
            <button
              type="button"
              onClick={() => {
                if (selectedDocument?.status === '已索引') {
                  void askSelectedDocument()
                  return
                }
                void indexSelectedDocument()
              }}
              disabled={!selectedDocument || isIndexingDocument || isAskingDocument}
            >
              {isIndexingDocument || isAskingDocument ? (
                <Loader2 className="spin" size={16} aria-hidden="true" />
              ) : (
                <Sparkles size={16} aria-hidden="true" />
              )}
              {selectedDocument?.status === '已索引' ? '发起论文问答' : '索引当前文献'}
            </button>
          </div>
        </section>

        {isCommandOpen ? (
          <section className="command-panel" aria-label="命令面板">
            <p className="eyebrow">可用命令</p>
            <div className="command-list">
              <button type="button" onClick={() => void importPdf()}>
                导入 PDF
              </button>
              <button type="button" onClick={runPaperQa}>
                对当前文献提问
              </button>
              <button type="button" onClick={() => void indexSelectedDocument()}>
                索引当前文献
              </button>
              <button type="button" onClick={() => selectPanel('skills')}>
                打开技能市场
              </button>
              <button type="button" onClick={() => void runSkill()} disabled={!selectedSkill}>
                运行当前技能
              </button>
              {filteredSkills.slice(0, 4).map((skill) => (
                <button
                  type="button"
                  key={skill.id}
                  onClick={() => {
                    selectSkill(skill)
                    setIsCommandOpen(false)
                  }}
                >
                  打开：{skill.name}
                </button>
              ))}
            </div>
          </section>
        ) : null}

        <section className="answer-panel">
          <div className="answer-header">
            <MessageSquareText size={18} aria-hidden="true" />
            <div>
              <p className="eyebrow">当前状态</p>
              <h3>{selectedDocument ? selectedDocument.title : '尚未选择文献'}</h3>
            </div>
          </div>

          {selectedDocument ? (
            <div className="document-detail">
              <p>
                文件名：<strong>{selectedDocument.fileName}</strong>
              </p>
              <p>
                页数：{selectedDocument.pageCount} | 阅读位置：第 {currentPage} 页 | 缩放：
                {zoom}%
              </p>
              <p>
                索引状态：{selectedDocument.status}。Phase 3 会基于当前文献 ID 调用本地
                PaperQA 研究服务。
              </p>
            </div>
          ) : (
            <div className="empty-state">
              <FileText size={24} aria-hidden="true" />
              <strong>还没有导入文献</strong>
              <span>点击左侧导入按钮，选择一份本地 PDF 开始。</span>
            </div>
          )}

          <div className="message-line" aria-live="polite">
            {error ?? message}
          </div>

          {activePanel === 'skills' ? (
            <section className="skill-detail-panel" aria-label="科学技能详情">
              {selectedSkill ? (
                <>
                  <div className="section-heading">
                    <span>{selectedSkill.name}</span>
                    <span>{selectedSkill.status}</span>
                  </div>
                  <p>{selectedSkill.description}</p>
                  <div className="skill-meta-grid">
                    <span>领域：{selectedSkill.domain}</span>
                    <span>模式：{selectedSkill.executionMode}</span>
                    <span>来源：{selectedSkill.source}</span>
                    <span>Commit：{selectedSkill.upstreamCommit.slice(0, 8)}</span>
                  </div>
                  {selectedSkill.requiredEnv.length ? (
                    <div className="dependency-note">
                      需要配置：{selectedSkill.requiredEnv.join('、')}
                    </div>
                  ) : null}
                  <div className="skill-input-grid">
                    {selectedSkill.requiredInputs.map((input) => (
                      <label className="form-field" key={input.name}>
                        <span>
                          {input.label}
                          {input.required ? ' *' : ''}
                        </span>
                        {input.type === 'textarea' ? (
                          <textarea
                            value={skillInputs[input.name] ?? ''}
                            placeholder={input.help ?? '输入技能任务上下文'}
                            onChange={(event) =>
                              setSkillInputs((current) => ({
                                ...current,
                                [input.name]: event.target.value,
                              }))
                            }
                          />
                        ) : (
                          <input
                            value={skillInputs[input.name] ?? ''}
                            placeholder={input.help ?? input.label}
                            onChange={(event) =>
                              setSkillInputs((current) => ({
                                ...current,
                                [input.name]: event.target.value,
                              }))
                            }
                          />
                        )}
                      </label>
                    ))}
                  </div>
                  <div className="form-actions">
                    <button
                      type="button"
                      onClick={() => void runSkill()}
                      disabled={isRunningSkill || !selectedSkill}
                    >
                      {isRunningSkill ? (
                        <Loader2 className="spin" size={15} aria-hidden="true" />
                      ) : (
                        <Play size={15} aria-hidden="true" />
                      )}
                      运行 dry-run
                    </button>
                    <button
                      type="button"
                      onClick={() => void loadSkills(selectedSkill.id)}
                      disabled={isLoadingSkills}
                    >
                      {isLoadingSkills ? (
                        <Loader2 className="spin" size={15} aria-hidden="true" />
                      ) : (
                        <Search size={15} aria-hidden="true" />
                      )}
                      刷新 registry
                    </button>
                  </div>
                  {visibleSkillRun ? (
                    <div className="skill-run-panel">
                      <div className="section-heading">
                        <span>运行记录</span>
                        <span>{visibleSkillRun.status}</span>
                      </div>
                      <div className="skill-log-list">
                        {visibleSkillRun.logs.map((log) => (
                          <span className={log.level} key={`${log.timestamp}-${log.message}`}>
                            {log.level} | {log.message}
                          </span>
                        ))}
                      </div>
                      {visibleSkillRun.outputs.map((output) => (
                        <article className="skill-output" key={output.id}>
                          <strong>{output.title}</strong>
                          <pre>{output.content}</pre>
                        </article>
                      ))}
                    </div>
                  ) : null}
                </>
              ) : (
                <div className="empty-state">
                  <FlaskConical size={24} aria-hidden="true" />
                  <strong>尚未载入科学技能</strong>
                  <span>请确认本地研究服务可启动，并刷新技能注册表。</span>
                </div>
              )}
            </section>
          ) : null}

          {activePanel === 'settings' ? (
            <section className="settings-panel" aria-label="模型配置">
              <div className="section-heading">
                <span>模型服务 Provider</span>
                <span>
                  {providerSettings?.provider === providerForm.provider && providerSettings?.hasApiKey
                    ? '当前 provider 已保存密钥'
                    : '当前 provider 未保存密钥'}
                </span>
              </div>
              <div className="provider-preset-grid">
                {providerPresets.map((preset) => (
                  <button
                    className={providerForm.provider === preset.provider ? 'selected' : ''}
                    type="button"
                    key={preset.provider}
                    onClick={() => applyProviderPreset(preset)}
                  >
                    <strong>{preset.label}</strong>
                    <span>{preset.description}</span>
                    <small>
                      {preset.model}
                      {preset.hasDevelopmentKey ? ' | 本机密钥可用' : ''}
                    </small>
                  </button>
                ))}
              </div>
              <label className="form-field">
                <span>Provider 类型</span>
                <input value={providerForm.provider} readOnly />
              </label>
              <label className="form-field">
                <span>Base URL</span>
                <input
                  value={providerForm.baseUrl}
                  placeholder="https://api.deepseek.com"
                  onChange={(event) =>
                    setProviderForm((current) => ({ ...current, baseUrl: event.target.value }))
                  }
                />
              </label>
              <label className="form-field">
                <span>模型</span>
                <input
                  value={providerForm.model}
                  placeholder="deepseek-v4-flash"
                  onChange={(event) =>
                    setProviderForm((current) => ({ ...current, model: event.target.value }))
                  }
                />
              </label>
              <label className="form-field">
                <span>API Key</span>
                <input
                  type="password"
                  value={providerForm.apiKey}
                  placeholder={providerSettings?.hasApiKey ? '已保存，留空则继续使用原密钥' : '输入 API Key'}
                  onChange={(event) =>
                    setProviderForm((current) => ({ ...current, apiKey: event.target.value }))
                  }
                />
              </label>
              <div className="form-actions">
                <button type="button" onClick={() => void saveProviderSettings()} disabled={isSavingProvider}>
                  {isSavingProvider ? <Loader2 className="spin" size={15} aria-hidden="true" /> : <Settings size={15} aria-hidden="true" />}
                  保存配置
                </button>
                <button type="button" onClick={() => void testProviderConnection()} disabled={isTestingProvider}>
                  {isTestingProvider ? <Loader2 className="spin" size={15} aria-hidden="true" /> : <Sparkles size={15} aria-hidden="true" />}
                  测试连接
                </button>
                <button type="button" onClick={() => void importDevelopmentApiKey()}>
                  <Sparkles size={15} aria-hidden="true" />
                  导入本机测试密钥
                </button>
              </div>
            </section>
          ) : null}

          {selectedDocument ? (
            <section className="paperqa-panel" aria-label="PaperQA 问答">
              <div className="section-heading">
                <span>PaperQA</span>
                <span>{latestRun ? `${latestRun.kind} | ${latestRun.status}` : '等待任务'}</span>
              </div>
              <textarea
                value={paperQuestion}
                placeholder="围绕当前文献提出一个可被来源支撑的问题"
                onChange={(event) => setPaperQuestion(event.target.value)}
              />
              <div className="form-actions">
                <button
                  type="button"
                  onClick={() => void indexSelectedDocument()}
                  disabled={isIndexingDocument || !isProviderReady}
                >
                  {isIndexingDocument ? <Loader2 className="spin" size={15} aria-hidden="true" /> : <FileSearch size={15} aria-hidden="true" />}
                  {selectedDocument.status === '索引失败' ? '重试索引' : '索引当前文献'}
                </button>
                <button
                  type="button"
                  onClick={() => void askSelectedDocument()}
                  disabled={isAskingDocument || selectedDocument.status !== '已索引' || !paperQuestion.trim()}
                >
                  {isAskingDocument ? <Loader2 className="spin" size={15} aria-hidden="true" /> : <Sparkles size={15} aria-hidden="true" />}
                  发起问答
                </button>
              </div>
            </section>
          ) : null}

          {latestAnswer ? (
            <section className="qa-answer-card" aria-label="PaperQA 回答">
              <p className="eyebrow">真实 PaperQA 回答</p>
              <h3>{latestAnswer.question}</h3>
              <p>{latestAnswer.answer}</p>
              <div className="citation-list">
                {latestAnswer.citations.length ? (
                  latestAnswer.citations.map((citation) => (
                    <button
                      className={citation.id === activeCitationId ? 'selected' : ''}
                      type="button"
                      key={citation.id}
                      onClick={() => followCitation(citation)}
                    >
                      <strong>{citation.page ? `第 ${citation.page} 页` : '页码未知'}</strong>
                      <span>{citation.excerpt}</span>
                    </button>
                  ))
                ) : (
                  <div className="empty-state compact">PaperQA 未返回结构化引用。</div>
                )}
              </div>
            </section>
          ) : null}

          {selectedDocument ? (
            <div className="source-row" aria-label="页码快捷跳转">
              {citationPages.map((page) => (
                <button
                  className={page === selectedCitationPage ? 'selected' : ''}
                  type="button"
                  key={page}
                  onClick={() => jumpToPage(page)}
                >
                  第 {page} 页
                </button>
              ))}
            </div>
          ) : null}
        </section>

        <section className="task-board" aria-label="研究任务状态">
          <article>
            <p className="eyebrow">已完成闭环</p>
            <h3>本地文献库</h3>
            <p>PDF 导入、复制、持久化、删除和阅读状态已经接入。</p>
          </article>
          <article>
            <p className="eyebrow">当前阶段</p>
            <h3>科学技能市场</h3>
            <p>从 science-skills 读取真实技能，支持搜索、筛选和 dry-run。</p>
          </article>
          <article>
            <p className="eyebrow">下一步</p>
            <h3>受控技能执行器</h3>
            <p>在白名单 runner 中执行技能脚本，记录日志和结构化产物。</p>
          </article>
        </section>
      </section>

      <div
        className="pdf-resize-handle"
        role={isPdfCollapsed ? undefined : 'separator'}
        aria-label={isPdfCollapsed ? undefined : '调整 PDF 预览宽度'}
        aria-orientation={isPdfCollapsed ? undefined : 'vertical'}
        onDoubleClick={() => setPdfPaneWidth(DEFAULT_PDF_PANE_WIDTH)}
        onPointerCancel={stopPdfResize}
        onPointerDown={startPdfResize}
        onPointerMove={resizePdfPane}
        onPointerUp={stopPdfResize}
      />

      {isPdfCollapsed ? (
        <aside className="pdf-pane pdf-pane-collapsed" aria-label="PDF 预览已折叠">
          <button
            className="icon-button pdf-expand-button"
            type="button"
            title="展开 PDF"
            onClick={() => {
              setIsPdfCollapsed(false)
              setMessage('PDF 预览已展开。')
            }}
          >
            <ChevronLeft size={15} aria-hidden="true" />
          </button>
          <span>PDF</span>
        </aside>
      ) : (
      <aside className="pdf-pane" aria-label="PDF 预览">
        <header className="pdf-toolbar">
          <div>
            <p className="eyebrow">PDF 预览</p>
            <h2>{selectedDocument?.title ?? '未选择文献'}</h2>
          </div>
          <div className="toolbar-actions">
            <button
              className="icon-button"
              type="button"
              title="上一页"
              onClick={() => jumpToPage(currentPage - 1)}
              disabled={!selectedDocument || currentPage <= 1}
            >
              <ChevronLeft size={15} aria-hidden="true" />
            </button>
            <button
              className="icon-button"
              type="button"
              title="下一页"
              onClick={() => jumpToPage(currentPage + 1)}
              disabled={!selectedDocument || currentPage >= (pdfPageCount || 1)}
            >
              <ChevronRight size={15} aria-hidden="true" />
            </button>
            <button
              className="icon-button"
              type="button"
              title="缩小"
              onClick={() => adjustZoom(-10)}
              disabled={!selectedDocument || zoom <= 60}
            >
              <ZoomOut size={15} aria-hidden="true" />
            </button>
            <button
              className="icon-button"
              type="button"
              title="放大"
              onClick={() => adjustZoom(10)}
              disabled={!selectedDocument || zoom >= 100}
            >
              <ZoomIn size={15} aria-hidden="true" />
            </button>
            <button
              className="icon-button"
              type="button"
              title="折叠 PDF"
              onClick={() => {
                setIsPdfCollapsed(true)
                setMessage('PDF 预览已折叠。')
              }}
            >
              <ChevronRight size={15} aria-hidden="true" />
            </button>
          </div>
        </header>

        <div className="pdf-stage" ref={pdfStageRef} onScroll={handlePdfScroll}>
          {isLoadingPdf ? (
            <div className="empty-state">
              <Loader2 className="spin" size={24} aria-hidden="true" />
              <strong>正在加载 PDF</strong>
            </div>
          ) : selectedDocument && pdfDocument ? (
            <div className="pdf-document">
              {pdfPageNumbers.map((pageNumber) => (
                <PdfPageCanvas
                  active={pageNumber === currentPage}
                  key={`${selectedDocument.id}-${pageNumber}`}
                  pageNumber={pageNumber}
                  pdfDocument={pdfDocument}
                  stageWidth={pdfStageWidth}
                  zoom={zoom}
                  onRenderError={reportPdfRenderError}
                />
              ))}
            </div>
          ) : (
            <div className="empty-state">
              <FileText size={24} aria-hidden="true" />
              <strong>等待 PDF</strong>
              <span>导入文献后，这里会显示真实 PDF 页面。</span>
            </div>
          )}
        </div>

        <footer className="pdf-footer">
          <button type="button" onClick={() => jumpToPage(selectedCitationPage)} disabled={!selectedDocument}>
            <PanelRight size={15} aria-hidden="true" />
            同步引用
          </button>
          <button type="button" onClick={() => void runSkill()} disabled={!selectedSkill}>
            <Play size={15} aria-hidden="true" />
            运行技能
          </button>
          <button
            className="danger-button"
            type="button"
            onClick={() => void deleteSelectedDocument()}
            disabled={!selectedDocument}
          >
            <Trash2 size={15} aria-hidden="true" />
            删除
          </button>
        </footer>
      </aside>
      )}
    </main>
  )
}

export default App
