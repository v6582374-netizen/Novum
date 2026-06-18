# Novum Research Service

Local Python service for PaperQA-backed document QA and Science Skills registry
integration.

## Development

```sh
cd services/research
python3.11 -m venv .venv
source .venv/bin/activate
python -m pip install -e ".[test]"
python -m uvicorn novum_research.app:app --host 127.0.0.1 --port 51731
```

The Tauri app starts this service automatically in development when `index_document`
or `ask_document` is called, and also when the Science Skills registry is loaded.
If dependencies are missing, the desktop UI surfaces a Chinese recovery message
instead of fabricating an answer.

## Science Skills

The service reads `../../vendor/science-skills/skills/**/SKILL.md`, extracts
metadata, and exposes:

```text
GET  /skills
GET  /skills/{skillId}
POST /skills/{skillId}/run
GET  /skill-runs/{runId}
```

Phase 4 uses a controlled dry-run runner. It validates the selected skill,
checks dependency status, records logs, and returns structured output without
executing arbitrary upstream shell commands.
