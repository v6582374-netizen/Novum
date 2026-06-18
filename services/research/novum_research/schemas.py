from __future__ import annotations

from typing import Any, Literal

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


class SkillInputSpec(NovumModel):
    name: str
    label: str
    type: Literal["text", "textarea", "file", "select", "number", "boolean"]
    required: bool
    default_value: str | int | float | bool | None = Field(alias="defaultValue")
    help: str | None = None


class ScienceSkill(NovumModel):
    id: str
    name: str
    description: str
    domain: str
    source: Literal["science-skills"]
    source_path: str = Field(alias="sourcePath")
    upstream_commit: str = Field(alias="upstreamCommit")
    required_inputs: list[SkillInputSpec] = Field(alias="requiredInputs")
    required_env: list[str] = Field(alias="requiredEnv")
    execution_mode: Literal["python", "prompt", "hybrid"] = Field(alias="executionMode")
    status: Literal["可用", "缺少依赖", "需要配置", "不可用"]
    updated_at: str = Field(alias="updatedAt")


class ListSkillsResponse(NovumModel):
    skills: list[ScienceSkill]


class SkillRunLog(NovumModel):
    timestamp: str
    level: Literal["info", "warning", "error"]
    message: str


class SkillRunOutput(NovumModel):
    id: str
    kind: Literal["markdown", "json", "file", "text"]
    title: str
    content: str
    file_path: str | None = Field(alias="filePath")


class SkillRun(NovumModel):
    id: str
    skill_id: str = Field(alias="skillId")
    skill_name: str = Field(alias="skillName")
    status: Literal["queued", "running", "succeeded", "failed"]
    started_at: str = Field(alias="startedAt")
    finished_at: str | None = Field(alias="finishedAt")
    error: str | None
    logs: list[SkillRunLog]
    outputs: list[SkillRunOutput]


class SkillRunContext(NovumModel):
    active_document_id: str | None = Field(alias="activeDocumentId")
    active_document_path: str | None = Field(alias="activeDocumentPath")
    selected_text: str | None = Field(alias="selectedText")
    provider: OpenAICompatibleProvider | None = None


class RunSkillRequest(NovumModel):
    inputs: dict[str, Any]
    context: SkillRunContext


class RunSkillResponse(NovumModel):
    run: SkillRun
