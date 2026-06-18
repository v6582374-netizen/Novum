from __future__ import annotations

import uuid
from datetime import UTC, datetime

from fastapi import FastAPI, HTTPException

from . import __version__
from .paperqa_adapter import ask_document as paperqa_ask_document
from .paperqa_adapter import index_document as paperqa_index_document
from .paperqa_adapter import is_paperqa_available
from .schemas import (
    AskDocumentRequest,
    AskDocumentResponse,
    IndexDocumentRequest,
    IndexDocumentResponse,
    ResearchHealth,
    ResearchRun,
    ResearchRunLog,
)

app = FastAPI(title="Novum Research Service", version=__version__)
RUNS: dict[str, ResearchRun] = {}


def _now() -> str:
    return datetime.now(UTC).isoformat()


def _log(level: str, message: str) -> ResearchRunLog:
    return ResearchRunLog(timestamp=_now(), level=level, message=message)


def _start_run(kind: str, document_id: str) -> ResearchRun:
    run = ResearchRun(
        id=str(uuid.uuid4()),
        kind=kind,
        status="running",
        documentId=document_id,
        startedAt=_now(),
        finishedAt=None,
        error=None,
        logs=[_log("info", "研究任务已开始。")],
    )
    RUNS[run.id] = run
    return run


def _finish_run(run: ResearchRun, status: str, error: str | None = None) -> ResearchRun:
    run.status = status
    run.finished_at = _now()
    run.error = error
    run.logs.append(_log("error" if error else "info", error or "研究任务已完成。"))
    RUNS[run.id] = run
    return run


def _raise_failed(run: ResearchRun, error: Exception) -> None:
    message = str(error) or "研究任务失败。"
    _finish_run(run, "failed", message)
    raise HTTPException(status_code=500, detail=message) from error


@app.get("/health", response_model=ResearchHealth)
async def health() -> ResearchHealth:
    return ResearchHealth(
        ok=True,
        serviceVersion=__version__,
        paperqaAvailable=is_paperqa_available(),
    )


@app.post("/documents/index", response_model=IndexDocumentResponse)
async def index_document(request: IndexDocumentRequest) -> IndexDocumentResponse:
    run = _start_run("index_document", request.document_id)
    try:
        await paperqa_index_document(
            request.pdf_path,
            request.index_path,
            request.provider,
        )
    except Exception as error:
        _raise_failed(run, error)

    _finish_run(run, "succeeded")
    return IndexDocumentResponse(run=run)


@app.post("/documents/ask", response_model=AskDocumentResponse)
async def ask_document(request: AskDocumentRequest) -> AskDocumentResponse:
    run = _start_run("ask_document", request.document_id)
    try:
        answer, citations = await paperqa_ask_document(
            document_id=request.document_id,
            title=request.document_id,
            question=request.question,
            index_path=request.index_path,
            provider=request.provider,
        )
    except Exception as error:
        _raise_failed(run, error)

    _finish_run(run, "succeeded")
    return AskDocumentResponse(runId=run.id, answer=answer, citations=citations)


@app.get("/runs/{run_id}", response_model=ResearchRun)
async def get_run(run_id: str) -> ResearchRun:
    run = RUNS.get(run_id)
    if not run:
        raise HTTPException(status_code=404, detail="找不到研究任务。")
    return run
