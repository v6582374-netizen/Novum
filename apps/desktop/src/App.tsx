import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
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

const DEFAULT_PDF_PANE_WIDTH = 520
const MIN_PDF_PANE_WIDTH = 420
const MAX_PDF_PANE_WIDTH = 900

const skills = [
  {
    name: 'AlphaGenome 检索',
    domain: '基因组学',
    state: '待接入',
  },
  {
    name: 'UniProt 证据提取',
    domain: '生物学',
    state: '待接入',
  },
  {
    name: 'AFDB 结构搜索',
    domain: '蛋白质',
    state: '待接入',
  },
]

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
  const [selectedSkillIndex, setSelectedSkillIndex] = useState(0)
  const [currentPage, setCurrentPage] = useState(1)
  const [zoom, setZoom] = useState(100)
  const [pdfDocument, setPdfDocument] = useState<PDFDocumentProxy | null>(null)
  const [isLoadingLibrary, setIsLoadingLibrary] = useState(true)
  const [isImporting, setIsImporting] = useState(false)
  const [isLoadingPdf, setIsLoadingPdf] = useState(false)
  const [isPdfCollapsed, setIsPdfCollapsed] = useState(false)
  const [isResizingPdf, setIsResizingPdf] = useState(false)
  const [pdfPaneWidth, setPdfPaneWidth] = useState(DEFAULT_PDF_PANE_WIDTH)
  const [pdfStageWidth, setPdfStageWidth] = useState(DEFAULT_PDF_PANE_WIDTH)
  const [isCommandOpen, setIsCommandOpen] = useState(false)
  const [selectedCitationPage, setSelectedCitationPage] = useState(1)
  const [message, setMessage] = useState('正在读取本地文献库。')
  const [error, setError] = useState<string | null>(null)

  const selectedDocument = useMemo(
    () => documents.find((document) => document.id === selectedDocumentId) ?? null,
    [documents, selectedDocumentId],
  )
  const selectedSkill = skills[selectedSkillIndex]
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

  useEffect(() => {
    void Promise.resolve().then(() => loadDocuments())
  }, [loadDocuments])

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

  function runPaperQa() {
    setActivePanel('qa')
    if (!selectedDocument) {
      setMessage('请先导入并选择一篇 PDF 文献。')
      return
    }
    setMessage('论文问答引擎尚未接入。本阶段只完成文献库、PDF 预览和阅读状态。')
  }

  function selectSkill(index: number) {
    setSelectedSkillIndex(index)
    setActivePanel('skills')
    setMessage(`已选中技能「${skills[index].name}」。技能执行器将在后续阶段接入。`)
  }

  function runSkill() {
    setActivePanel('skills')
    setMessage(`「${selectedSkill.name}」尚未接入真实执行器。`)
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
              title="搜索技能"
              onClick={() => {
                setActivePanel('skills')
                setMessage('技能搜索将在 science-skills 注册表接入后启用。')
              }}
            >
              <Search size={15} aria-hidden="true" />
            </button>
          </div>
          <div className="skill-list">
            {skills.map((skill, index) => (
              <button
                className={`skill-row ${index === selectedSkillIndex ? 'selected' : ''}`}
                key={skill.name}
                type="button"
                onClick={() => selectSkill(index)}
              >
                <span>
                  <strong>{skill.name}</strong>
                  <small>
                    {skill.domain} | {skill.state}
                  </small>
                </span>
                <ChevronRight size={15} aria-hidden="true" />
              </button>
            ))}
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
            <span>模型尚未配置</span>
            <span>论文问答适配器待接入</span>
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
            <button type="button" onClick={runPaperQa}>
              <Sparkles size={16} aria-hidden="true" />
              发起论文问答
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
              <button type="button" onClick={() => selectPanel('skills')}>
                打开技能市场
              </button>
              <button type="button" onClick={runSkill}>
                运行当前技能
              </button>
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
                PaperQA 与科学技能执行尚未接入。本阶段保留真实文献、PDF 预览和阅读状态，
                后续会基于当前文献 ID 调用研究适配器。
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
            <h3>真实 PDF 预览</h3>
            <p>右侧使用 PDF.js 渲染真实页面，并支持页码与缩放。</p>
          </article>
          <article>
            <p className="eyebrow">下一步</p>
            <h3>论文问答适配器</h3>
            <p>基于文献 ID 接入 PaperQA 索引、问答与引用跳转。</p>
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
          <button type="button" onClick={runSkill}>
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
