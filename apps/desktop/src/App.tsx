import { useMemo, useState } from 'react'
import {
  BookOpen,
  BrainCircuit,
  ChevronRight,
  Command,
  FileSearch,
  FlaskConical,
  Library,
  Maximize2,
  MessageSquareText,
  PanelRight,
  Play,
  Search,
  Settings,
  Sparkles,
  Upload,
  ZoomIn,
  ZoomOut,
} from 'lucide-react'
import './App.css'

type PanelId = 'library' | 'qa' | 'skills' | 'settings'

const papers = [
  {
    title: '科学发现中的尺度定律',
    venue: '前沿札记',
    pages: 28,
    status: '已索引',
    question: '当前论文的主要实验瓶颈是什么？',
    answer:
      '论文指出，关键瓶颈不在单次推理能力，而在假设生成、文献验证和实验设计之间的往返延迟。Novum 会把回答、引用和 PDF 页码保持同步，帮助研究者快速回到证据原文。',
    excerpt:
      '一个持续存在的限制，是从假设生成到文献验证，再到定向实验设计之间的延迟。',
  },
  {
    title: '蛋白质模型中的机制可解释性',
    venue: '阅读队列',
    pages: 42,
    status: '待索引',
    question: '这篇论文最值得优先追踪的证据是什么？',
    answer:
      '当前阅读优先级应放在模型内部表征与实验可观测结构之间的对应关系。后续接入论文问答引擎后，这里会展示跨段落引用和可跳转证据。',
    excerpt:
      '模型内部特征需要和可验证的结构、生物物理约束以及实验观测建立对应。',
  },
  {
    title: '闭环假设生成',
    venue: '方法库',
    pages: 19,
    status: '已索引',
    question: '闭环研究流程最需要降低哪类成本？',
    answer:
      '最需要降低的是从候选假设到下一步可执行实验之间的组织成本。Novum 的目标是让工具调用、证据定位和研究记录形成同一个本地闭环。',
    excerpt:
      '闭环系统的价值来自快速把候选假设转化为可验证的下一步动作。',
  },
]

const skills = [
  {
    name: 'AlphaGenome 检索',
    domain: '基因组学',
    state: '可运行',
  },
  {
    name: 'UniProt 证据提取',
    domain: '生物学',
    state: '需要输入',
  },
  {
    name: 'AFDB 结构搜索',
    domain: '蛋白质',
    state: '可运行',
  },
]

const citations = [
  { label: '第 4 页', page: 4 },
  { label: '第 11 页', page: 11 },
  { label: '第 18 页', page: 18 },
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

function App() {
  const [activePanel, setActivePanel] = useState<PanelId>('library')
  const [selectedPaperIndex, setSelectedPaperIndex] = useState(0)
  const [selectedSkillIndex, setSelectedSkillIndex] = useState(0)
  const [selectedCitationIndex, setSelectedCitationIndex] = useState(0)
  const [zoom, setZoom] = useState(100)
  const [isPdfFocused, setIsPdfFocused] = useState(false)
  const [isCommandOpen, setIsCommandOpen] = useState(false)
  const [message, setMessage] = useState('已打开本地文献工作台。')

  const selectedPaper = papers[selectedPaperIndex]
  const selectedSkill = skills[selectedSkillIndex]
  const selectedCitation = citations[selectedCitationIndex]

  const panelTitle = useMemo(() => {
    if (activePanel === 'library') return '文献库'
    if (activePanel === 'qa') return '论文问答'
    if (activePanel === 'skills') return '技能市场'
    return '模型配置'
  }, [activePanel])

  function selectPanel(panel: PanelId) {
    setActivePanel(panel)
    setIsCommandOpen(false)
    setMessage(`已切换到「${panels.find((item) => item.id === panel)?.label}」。`)
  }

  function selectPaper(index: number) {
    setSelectedPaperIndex(index)
    setSelectedCitationIndex(0)
    setActivePanel('library')
    setMessage(`已选中文献「${papers[index].title}」。`)
  }

  function selectSkill(index: number) {
    setSelectedSkillIndex(index)
    setActivePanel('skills')
    setMessage(`已选中技能「${skills[index].name}」。`)
  }

  function runPaperQa() {
    setActivePanel('qa')
    setMessage(`已基于「${selectedPaper.title}」生成一条模拟论文问答。`)
  }

  function jumpToCitation(index: number) {
    setSelectedCitationIndex(index)
    setMessage(`已跳转到「${selectedPaper.title}」${citations[index].label}。`)
  }

  function adjustZoom(delta: number) {
    setZoom((current) => {
      const next = Math.min(140, Math.max(80, current + delta))
      setMessage(`PDF 缩放已调整为 ${next}%。`)
      return next
    })
  }

  function runSkill() {
    setActivePanel('skills')
    setMessage(`已准备运行「${selectedSkill.name}」，真实执行器将在后续接入。`)
  }

  return (
    <main className={`app-shell ${isPdfFocused ? 'pdf-focused' : ''}`}>
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
              onClick={() => {
                setActivePanel('library')
                setMessage('PDF 导入入口已触发，真实文件选择器将在下一阶段接入。')
              }}
            >
              <Upload size={15} aria-hidden="true" />
            </button>
          </div>
          <div className="paper-list">
            {papers.map((paper, index) => (
              <button
                className={`paper-row ${index === selectedPaperIndex ? 'selected' : ''}`}
                key={paper.title}
                type="button"
                onClick={() => selectPaper(index)}
              >
                <BookOpen size={15} aria-hidden="true" />
                <span>
                  <strong>{paper.title}</strong>
                  <small>
                    {paper.venue} | {paper.pages} 页 | {paper.status}
                  </small>
                </span>
              </button>
            ))}
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
                setMessage('技能搜索入口已触发。')
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
            <span>本地文献库已就绪</span>
            <span>模型尚未配置</span>
            <span>论文问答适配器待接入</span>
          </div>
        </header>

        <section className="query-panel" id="paperqa" aria-live="polite">
          <div className="query-input">
            <FileSearch size={18} aria-hidden="true" />
            <span>
              围绕当前文献、选中文献集或当前引用发起问题。
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
              <button type="button" onClick={runPaperQa}>
                对当前文献提问
              </button>
              <button type="button" onClick={() => selectPanel('skills')}>
                打开技能市场
              </button>
              <button type="button" onClick={runSkill}>
                运行当前技能
              </button>
              <button
                type="button"
                onClick={() => {
                  setActivePanel('settings')
                  setMessage('已打开模型配置，真实密钥存储将在后续接入。')
                }}
              >
                配置模型服务
              </button>
            </div>
          </section>
        ) : null}

        <section className="answer-panel">
          <div className="answer-header">
            <MessageSquareText size={18} aria-hidden="true" />
            <div>
              <p className="eyebrow">当前问答</p>
              <h3>{selectedPaper.question}</h3>
            </div>
          </div>
          <p>{selectedPaper.answer}</p>
          <div className="message-line" aria-live="polite">
            {message}
          </div>
          <div className="source-row" aria-label="回答引用">
            {citations.map((source, index) => (
              <button
                className={index === selectedCitationIndex ? 'selected' : ''}
                type="button"
                key={source.label}
                onClick={() => jumpToCitation(index)}
              >
                {source.label}
              </button>
            ))}
          </div>
        </section>

        <section className="task-board" aria-label="研究任务状态">
          <article>
            <p className="eyebrow">下一步集成</p>
            <h3>论文问答适配器</h3>
            <p>索引 PDF、回答当前文献问题，并返回可跳转引用。</p>
          </article>
          <article>
            <p className="eyebrow">技能市场</p>
            <h3>科学技能注册表</h3>
            <p>展示技能元信息，同时隐藏脚本和参考资料目录。</p>
          </article>
          <article>
            <p className="eyebrow">发布升级</p>
            <h3>Homebrew 升级通道</h3>
            <p>将命令行升级能力纳入 macOS 首发发布计划。</p>
          </article>
        </section>
      </section>

      <aside className="pdf-pane" aria-label="PDF 预览">
        <header className="pdf-toolbar">
          <div>
            <p className="eyebrow">PDF 预览</p>
            <h2>{selectedPaper.title}</h2>
          </div>
          <div className="toolbar-actions">
            <button
              className="icon-button"
              type="button"
              title="缩小"
              onClick={() => adjustZoom(-10)}
            >
              <ZoomOut size={15} aria-hidden="true" />
            </button>
            <button
              className="icon-button"
              type="button"
              title="放大"
              onClick={() => adjustZoom(10)}
            >
              <ZoomIn size={15} aria-hidden="true" />
            </button>
            <button
              className={`icon-button ${isPdfFocused ? 'active' : ''}`}
              type="button"
              title="聚焦 PDF"
              onClick={() => {
                setIsPdfFocused((value) => !value)
                setMessage('PDF 聚焦模式已切换。')
              }}
            >
              <Maximize2 size={15} aria-hidden="true" />
            </button>
          </div>
        </header>

        <div className="pdf-stage">
          <div className="pdf-page" style={{ transform: `scale(${zoom / 100})` }}>
            <div className="page-meta">
              <span>
                第 {selectedCitation.page} 页 / 共 {selectedPaper.pages} 页
              </span>
              <span>{zoom}%</span>
            </div>
            <h3>实验瓶颈</h3>
            <p>{selectedPaper.excerpt}</p>
            <p className="highlight">
              当前引用已同步到 {selectedCitation.label}。后续接入真实 PDF
              后，这里会定位到论文原文段落。
            </p>
            <p>
              右侧预览栏会在提问、运行技能和比较结果时保持可见，确保所有
              智能体输出都能回到证据来源。
            </p>
          </div>
        </div>

        <footer className="pdf-footer">
          <button type="button" onClick={() => jumpToCitation(selectedCitationIndex)}>
            <PanelRight size={15} aria-hidden="true" />
            同步引用
          </button>
          <button type="button" onClick={runSkill}>
            <Play size={15} aria-hidden="true" />
            运行技能
          </button>
        </footer>
      </aside>
    </main>
  )
}

export default App
