# Novum Research Service

Phase 3 local Python service for PaperQA-backed indexing and document QA.

## Development

```sh
cd services/research
python3.11 -m venv .venv
source .venv/bin/activate
python -m pip install -e ".[test]"
python -m uvicorn novum_research.app:app --host 127.0.0.1 --port 51731
```

The Tauri app starts this service automatically in development when `index_document`
or `ask_document` is called. If dependencies are missing, the desktop UI surfaces a
Chinese recovery message instead of fabricating an answer.
