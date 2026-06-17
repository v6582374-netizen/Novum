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

const papers = [
  {
    title: 'Scaling Laws for Scientific Discovery',
    venue: 'Frontier Notes',
    pages: 28,
    status: 'Indexed',
  },
  {
    title: 'Mechanistic Interpretability In Protein Models',
    venue: 'Reading Queue',
    pages: 42,
    status: 'Queued',
  },
  {
    title: 'Closed-Loop Hypothesis Generation',
    venue: 'Methods',
    pages: 19,
    status: 'Indexed',
  },
]

const skills = [
  {
    name: 'AlphaGenome Lookup',
    domain: 'Genomics',
    state: 'Ready',
  },
  {
    name: 'UniProt Evidence Pull',
    domain: 'Biology',
    state: 'Needs input',
  },
  {
    name: 'AFDB Structure Search',
    domain: 'Proteins',
    state: 'Ready',
  },
]

const answerSources = ['p. 4', 'p. 11', 'p. 18']

function App() {
  return (
    <main className="app-shell">
      <aside className="sidebar" aria-label="Research workspace navigation">
        <div className="brand-row">
          <div className="brand-mark">N</div>
          <div>
            <p className="eyebrow">Novum</p>
            <h1>Research IDE</h1>
          </div>
        </div>

        <button className="command-button" type="button">
          <Command size={16} aria-hidden="true" />
          <span>Command palette</span>
          <kbd>Cmd K</kbd>
        </button>

        <nav className="nav-stack">
          <a className="nav-item active" href="#library">
            <Library size={16} aria-hidden="true" />
            Library
          </a>
          <a className="nav-item" href="#paperqa">
            <BrainCircuit size={16} aria-hidden="true" />
            PaperQA
          </a>
          <a className="nav-item" href="#skills">
            <FlaskConical size={16} aria-hidden="true" />
            Skill market
          </a>
          <a className="nav-item" href="#settings">
            <Settings size={16} aria-hidden="true" />
            Providers
          </a>
        </nav>

        <section className="sidebar-section" id="library">
          <div className="section-heading">
            <span>Active papers</span>
            <button className="icon-button" type="button" title="Import PDF">
              <Upload size={15} aria-hidden="true" />
            </button>
          </div>
          <div className="paper-list">
            {papers.map((paper, index) => (
              <button
                className={`paper-row ${index === 0 ? 'selected' : ''}`}
                key={paper.title}
                type="button"
              >
                <BookOpen size={15} aria-hidden="true" />
                <span>
                  <strong>{paper.title}</strong>
                  <small>
                    {paper.venue} | {paper.pages} pages | {paper.status}
                  </small>
                </span>
              </button>
            ))}
          </div>
        </section>

        <section className="sidebar-section" id="skills">
          <div className="section-heading">
            <span>Science skills</span>
            <button className="icon-button" type="button" title="Search skills">
              <Search size={15} aria-hidden="true" />
            </button>
          </div>
          <div className="skill-list">
            {skills.map((skill) => (
              <button className="skill-row" key={skill.name} type="button">
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

      <section className="workspace" aria-label="Paper workbench">
        <header className="workspace-topbar">
          <div>
            <p className="eyebrow">Long-term research mode</p>
            <h2>Paper workbench</h2>
          </div>
          <div className="status-strip" aria-label="Local runtime status">
            <span>Local library online</span>
            <span>Provider not configured</span>
            <span>PaperQA adapter planned</span>
          </div>
        </header>

        <section className="query-panel" id="paperqa">
          <div className="query-input">
            <FileSearch size={18} aria-hidden="true" />
            <span>
              Ask about the active paper, selected corpus, or current citation.
            </span>
            <button type="button">
              <Sparkles size={16} aria-hidden="true" />
              Ask PaperQA
            </button>
          </div>
        </section>

        <section className="answer-panel">
          <div className="answer-header">
            <MessageSquareText size={18} aria-hidden="true" />
            <div>
              <p className="eyebrow">Draft answer surface</p>
              <h3>What is the main experimental bottleneck?</h3>
            </div>
          </div>
          <p>
            Novum will keep agent output source-grounded. Answers from PaperQA
            should show citations inline and let the user jump the PDF preview
            to the relevant page or passage.
          </p>
          <div className="source-row" aria-label="Answer citations">
            {answerSources.map((source) => (
              <button type="button" key={source}>
                {source}
              </button>
            ))}
          </div>
        </section>

        <section className="task-board" aria-label="Research task status">
          <article>
            <p className="eyebrow">Next integration</p>
            <h3>PaperQA adapter</h3>
            <p>Index PDFs, ask active-paper questions, and return citations.</p>
          </article>
          <article>
            <p className="eyebrow">Skill market</p>
            <h3>Science skills registry</h3>
            <p>Expose SKILL.md metadata while hiding scripts and references.</p>
          </article>
          <article>
            <p className="eyebrow">Distribution</p>
            <h3>Homebrew upgrade path</h3>
            <p>Keep CLI-friendly updates central to the macOS release plan.</p>
          </article>
        </section>
      </section>

      <aside className="pdf-pane" aria-label="PDF preview">
        <header className="pdf-toolbar">
          <div>
            <p className="eyebrow">PDF preview</p>
            <h2>Scaling Laws for Scientific Discovery</h2>
          </div>
          <div className="toolbar-actions">
            <button className="icon-button" type="button" title="Zoom out">
              <ZoomOut size={15} aria-hidden="true" />
            </button>
            <button className="icon-button" type="button" title="Zoom in">
              <ZoomIn size={15} aria-hidden="true" />
            </button>
            <button className="icon-button" type="button" title="Focus PDF">
              <Maximize2 size={15} aria-hidden="true" />
            </button>
          </div>
        </header>

        <div className="pdf-stage">
          <div className="pdf-page">
            <div className="page-meta">
              <span>Page 4 of 28</span>
              <span>Active citation</span>
            </div>
            <h3>Experimental Bottlenecks</h3>
            <p>
              A persistent limitation is the delay between hypothesis
              generation, literature validation, and targeted experiment
              design.
            </p>
            <p className="highlight">
              Citation-linked reading will keep the agent accountable to the
              exact paper passage under discussion.
            </p>
            <p>
              The preview pane should stay visible while the user asks
              questions, invokes skills, and compares results.
            </p>
          </div>
        </div>

        <footer className="pdf-footer">
          <button type="button">
            <PanelRight size={15} aria-hidden="true" />
            Sync citation
          </button>
          <button type="button">
            <Play size={15} aria-hidden="true" />
            Run skill
          </button>
        </footer>
      </aside>
    </main>
  )
}

export default App
