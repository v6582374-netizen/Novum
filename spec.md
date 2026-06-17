# Novum Specification

## 1. Vision And Design Philosophy

Novum is a desktop research IDE for scientists, independent researchers, and technically strong builders who want to push long-running research programs forward. It is not a general chatbot shell and not a lightweight PDF reader. It is a dense, tool-first environment where agents assist the user while the user remains the scientific operator.

The product should feel closer to an IDE than to a consumer writing app. The interface should support frequent human-agent interaction, rapid invocation of specialized tools, local knowledge organization, and continuous inspection of source material. The first version focuses on long-term research mode: sustained literature reading, question answering over papers, skill-assisted scientific workflows, notes, citations, and iterative hypothesis development.

Short-term competition mode is a future product mode. It should not shape the MVP implementation except that the architecture should leave room for multiple modes later.

Core principles:

- Desktop first. macOS ships first; Windows follows after the core architecture stabilizes.
- Local first. User research data, PDFs, notes, indexes, conversations, and run logs are stored on the user's machine by default.
- Tool first. Built-in capabilities should expose strong open-source research tools through a coherent interface instead of hiding them behind generic chat.
- Agent as assistant, user as driver. The agent should accelerate reading, retrieval, synthesis, and tool execution, but the user should be able to inspect sources and intermediate states.
- Hardcore over decorative. The UI should prioritize density, keyboard access, clear panes, command workflows, and scientific traceability.

## 2. MVP Scope

The MVP implements long-term research mode only. The central workflow is a paper workbench:

1. The user imports or opens scientific PDFs.
2. Novum stores documents in a local library and creates local indexes.
3. The user reads the selected paper in a persistent right-side PDF preview pane.
4. The user asks questions over the active paper or a selected document set.
5. PaperQA returns answers with citations and source references.
6. Citation clicks navigate the PDF preview to the relevant page or passage when possible.
7. The user records notes and can continue the research thread across sessions.

The MVP should include:

- macOS desktop app.
- IDE-style shell with left navigation, central command/agent area, and right PDF preview.
- Local document library for PDFs and metadata.
- Real-time PDF preview on the far right.
- PaperQA-backed paper question answering.
- User-managed LLM provider configuration.
- A first version of the science skill market.
- GPT Researcher-inspired deep research task UI patterns, limited to research task configuration and progress display where useful.
- CLI/package-manager upgrade path for developers and technical users.

The MVP should not include:

- Short-term competition mode.
- Cloud account system.
- Cloud sync.
- Built-in model hosting.
- Team collaboration.
- Mobile or web versions.
- Billing, subscriptions, or managed credits.

## 3. Desktop And System Architecture

Novum should use Tauri + React for the desktop shell.

- Tauri owns the native desktop app, filesystem access, secure local commands, packaging, updater integration, and OS-specific capabilities.
- React owns the IDE UI, panes, command palette, skill market, task screens, PDF preview container, and interaction state.
- A local Python runtime or service layer owns integrations with Python-first research tools such as PaperQA, GPT Researcher, and science-skills.
- A typed bridge between the frontend and local backend should expose stable commands for document operations, indexing, QA, skill discovery, skill execution, and run logs.

Preferred high-level layout:

- Left pane: workspace navigation, document library, projects, saved searches, and skill market entry.
- Center pane: command palette, active research thread, PaperQA answers, task progress, notes, and tool output.
- Right pane: real-time PDF preview, always available when a paper is selected.

The app should be built as a local-first system. All research artifacts should be stored under an application data directory, with project-level folders supported later if needed.

Initial local data categories:

- PDF files or references to imported files.
- Extracted metadata.
- Text extraction artifacts.
- Vector/search indexes.
- Conversation history.
- Notes.
- Tool execution logs.
- Skill registry cache.
- User provider settings.

## 4. Open-Source Integrations

Novum's built-in power should come from carefully embedded open-source projects. The implementation should preserve upstream provenance, license files, version pins, and local patch records.

### PaperQA

Source: <https://github.com/Future-House/paper-qa>

PaperQA is the first core research engine. It should power document-grounded question answering, citation-backed answers, and multi-paper retrieval where supported.

MVP behavior:

- Index imported PDFs for QA.
- Ask questions over the active paper.
- Ask questions over a selected document set.
- Return answers with citations.
- Map citations back to PDF pages or passages when possible.
- Store answer history in the local research thread.

### GPT Researcher

Source: <https://github.com/assafelovic/gpt-researcher>

GPT Researcher should influence Novum's research task UX. Its web UI is a useful reference for task setup, visible progress, source gathering, and final report flow.

MVP behavior:

- Borrow product patterns for research task configuration and progress display.
- Expose a simple deep research task surface only after the paper workbench flow is stable.
- Keep generated reports inspectable, source-linked, and saved locally.

### Science Skills

Source: <https://github.com/google-deepmind/science-skills>

Science-skills should become a built-in skill market for scientific workflows. Users should not have to browse raw repository folders. The UI should surface human-facing skill metadata and invocation controls.

MVP behavior:

- Parse and display skill names, descriptions, domains, requirements, and usage hints from `SKILL.md`.
- Hide implementation-only folders such as `scripts/` and `references/` from the normal user interface.
- Allow users to invoke skills by clicking in the skill market or through the command palette.
- Show required inputs, permissions, run status, output, and logs.
- Keep execution local unless a skill explicitly requires a network/API call and the user has configured credentials.

### Warp UI Reference

Source: <https://github.com/warpdotdev/warp>

Novum should reuse the Warp terminal style as a design reference: dense developer tooling, command-first interaction, pane-based workflows, restrained surfaces, excellent keyboard ergonomics, and terminal/IDE visual language.

License boundary:

- The Warp repository is AGPL-3.0 overall.
- Novum may reuse only components or code that are explicitly available under permissive licensing, such as MIT-licensed UI framework portions where applicable.
- Otherwise, Novum should reimplement the visual and interaction language without copying AGPL-covered source.
- The spec's phrase "Warp style" means visual and UX alignment, not unrestricted source reuse.

## 5. Source Embedding And Upgrade Strategy

The selected upstream projects should be embedded as source snapshots rather than pure external services. This gives Novum tighter product integration and a smoother offline/local-first story.

Required source management rules:

- Each embedded project must include its upstream URL, commit SHA or release tag, license file, and date imported.
- Local changes must be isolated in clearly documented patch files or adapter layers.
- Avoid editing upstream code directly unless there is no practical alternative.
- If upstream code must be modified, record the reason and expected rebase cost.
- Maintain a simple upgrade checklist for refreshing each embedded source snapshot.

Recommended repository structure once implementation begins:

```text
vendor/
  paper-qa/
  gpt-researcher/
  science-skills/
licenses/
  paper-qa/
  gpt-researcher/
  science-skills/
patches/
  paper-qa/
  gpt-researcher/
  science-skills/
apps/
  desktop/
services/
  research/
```

The frontend should not depend on raw upstream internals. It should call Novum-owned adapters so that upstream upgrades do not force UI rewrites.

## 6. Core Product Modules

### Paper Workbench

The paper workbench is the first-class MVP surface.

Capabilities:

- Import PDF.
- Select active paper.
- Preview PDF in the right pane.
- Extract text and metadata.
- Build local QA/search indexes.
- Ask document-grounded questions.
- Display citation-backed answers.
- Click citations to navigate the PDF preview.
- Save notes and research thread state.

### PDF Preview

The PDF preview must be a persistent right-side pane, not a modal or secondary screen. It should update as the user selects papers, clicks citations, changes search results, or follows agent references.

Capabilities:

- Page navigation.
- Zoom controls.
- Search within document.
- Citation jump.
- Highlight active citation/page.
- Preserve reading position per document.

### Agent And Command Surface

The central pane should support high-frequency human-agent interaction.

Capabilities:

- Command palette for built-in actions.
- Research thread view.
- Tool invocation status.
- Intermediate outputs.
- Answer source display.
- Error states with actionable recovery.

The agent should be framed as an operator assistant. It should propose, retrieve, summarize, compare, and invoke tools, while keeping source evidence visible.

### Skill Market

The skill market is a structured browser and launcher for embedded science skills.

Capabilities:

- Browse skills.
- Search and filter by domain.
- Inspect required inputs and capabilities.
- Invoke skill by click.
- Invoke skill from command palette.
- Show run logs and outputs.
- Hide raw `scripts/` and `references/` implementation details.

### Provider Settings

Novum should not operate a hosted model service in the MVP. Users provide their own API keys.

Capabilities:

- Configure supported LLM providers.
- Store credentials locally using OS-appropriate secure storage.
- Validate provider connectivity.
- Select default model/provider for research tasks.
- Allow per-task override later.

Initial target providers:

- OpenAI-compatible APIs.
- Anthropic.
- Gemini.
- Local provider hooks can be added later, but local models are not the default MVP path.

## 7. UI And Interaction Requirements

Novum should look and feel like a serious developer/scientist tool.

UI requirements:

- Desktop-only layout for MVP.
- Three-pane IDE shell.
- Dense but readable controls.
- Command palette as a primary interaction path.
- Keyboard-first navigation.
- Dark theme first, with room for light theme later.
- Pane resizing.
- Persistent right-side PDF preview.
- Tool outputs and citations should be inspectable, not hidden behind opaque chat bubbles.
- Avoid marketing-style cards, oversized hero sections, decorative gradients, and low-density consumer UI.

The first screen after setup should be the actual research workspace, not a landing page.

## 8. Distribution And Upgrade

Novum should support developer-friendly command-line upgrades.

macOS first:

- Publish a signed and notarized macOS build.
- Provide Homebrew distribution as the first package-manager path.
- The expected upgrade command is:

```sh
brew upgrade novum
```

Windows later:

- Support winget and/or Scoop after the macOS MVP stabilizes.

The app may also include an in-app update notification later, but command-line/package-manager upgrade is the primary requirement for the target audience.

## 9. Acceptance Criteria

The MVP is acceptable when the following scenarios work end to end:

- A user installs Novum on macOS and opens the desktop app.
- A user imports a PDF and sees it in the right-side preview pane.
- A user asks a question about the active PDF and receives a citation-backed answer from PaperQA.
- A user clicks a citation and the PDF preview navigates to the relevant page or passage when possible.
- A user configures their own LLM provider credentials locally.
- A user opens the skill market, selects a science skill, supplies required inputs, runs it, and sees output/logs.
- A user can update Novum through the macOS package-manager flow.

Engineering acceptance:

- Embedded upstream projects have recorded URL, version, license, and import date.
- Frontend code calls Novum-owned adapters rather than raw upstream internals.
- Local data paths are documented.
- Provider credentials are not stored in plaintext application files.
- Failures from indexing, QA, skill execution, and provider calls produce visible, actionable errors.

## 10. Future Directions

Future versions may add:

- Short-term competition mode.
- Cloud sync.
- Team workspaces.
- Report authoring and export.
- Deeper GPT Researcher integration.
- Local model-first workflows.
- Windows parity.
- Plugin ecosystem beyond bundled science skills.
- Dataset and code execution workspaces.
- Experiment tracking and reproducibility tools.

