from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field


class NovumModel(BaseModel):
    model_config = ConfigDict(populate_by_name=True)


class OpenAICompatibleProvider(NovumModel):
    base_url: str = Field(alias="baseUrl")
    model: str
    api_key: str = Field(alias="apiKey")


class ResearchHealth(NovumModel):
    ok: bool
    service_version: str = Field(alias="serviceVersion")
    paperqa_available: bool = Field(alias="paperqaAvailable")


class IndexDocumentRequest(NovumModel):
    document_id: str = Field(alias="documentId")
    pdf_path: str = Field(alias="pdfPath")
    index_path: str = Field(alias="indexPath")
    provider: OpenAICompatibleProvider


class AskDocumentRequest(NovumModel):
    document_id: str = Field(alias="documentId")
    pdf_path: str = Field(alias="pdfPath")
    index_path: str = Field(alias="indexPath")
    question: str
    provider: OpenAICompatibleProvider


class QaCitation(NovumModel):
    id: str
    document_id: str = Field(alias="documentId")
    title: str
    page: int | None
    excerpt: str
    source_label: str = Field(alias="sourceLabel")
    confidence: float | None = None


class AskDocumentResponse(NovumModel):
    run_id: str = Field(alias="runId")
    answer: str
    citations: list[QaCitation]


class ResearchRunLog(NovumModel):
    timestamp: str
    level: Literal["info", "warning", "error"]
    message: str


class ResearchRun(NovumModel):
    id: str
    kind: Literal["index_document", "ask_document"]
    status: Literal["queued", "running", "succeeded", "failed"]
    document_id: str = Field(alias="documentId")
    started_at: str = Field(alias="startedAt")
    finished_at: str | None = Field(alias="finishedAt")
    error: str | None
    logs: list[ResearchRunLog]


class IndexDocumentResponse(NovumModel):
    run: ResearchRun
