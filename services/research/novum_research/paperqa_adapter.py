from __future__ import annotations

import asyncio
import os
import pickle
import uuid
from pathlib import Path
from typing import Any

from .schemas import OpenAICompatibleProvider, QaCitation


def is_paperqa_available() -> bool:
    try:
        import paperqa  # noqa: F401
    except Exception:
        return False
    return True


def _paperqa_import_error() -> RuntimeError:
    return RuntimeError(
        "PaperQA 运行时不可用。请在 services/research 中执行 "
        'python -m pip install -e ".[test]" 后重试。'
    )


def _prepare_provider_environment(provider: OpenAICompatibleProvider) -> None:
    os.environ["OPENAI_API_KEY"] = provider.api_key
    os.environ["OPENAI_BASE_URL"] = provider.base_url
    os.environ["OPENAI_API_BASE"] = provider.base_url


def _build_settings(provider: OpenAICompatibleProvider) -> Any:
    try:
        from paperqa import Settings
    except Exception as error:
        raise _paperqa_import_error() from error

    _prepare_provider_environment(provider)

    try:
        return Settings(
            llm=provider.model,
            summary_llm=provider.model,
            embedding="sparse",
            temperature=0,
        )
    except TypeError:
        settings = Settings()
        for field, value in {
            "llm": provider.model,
            "summary_llm": provider.model,
            "embedding": "sparse",
            "temperature": 0,
        }.items():
            if hasattr(settings, field):
                setattr(settings, field, value)
        return settings


def _docs_file(index_path: str) -> Path:
    path = Path(index_path)
    path.mkdir(parents=True, exist_ok=True)
    return path.joinpath("paperqa-docs.pkl")


def _load_docs(index_path: str) -> Any:
    docs_path = _docs_file(index_path)
    if not docs_path.exists():
        raise RuntimeError("当前文献尚未建立 PaperQA 索引，请先索引后再提问。")

    with docs_path.open("rb") as file:
        return pickle.load(file)


def _save_docs(index_path: str, docs: Any) -> None:
    docs_path = _docs_file(index_path)
    with docs_path.open("wb") as file:
        pickle.dump(docs, file)


async def index_document(pdf_path: str, index_path: str, provider: OpenAICompatibleProvider) -> None:
    try:
        from paperqa import Docs
    except Exception as error:
        raise _paperqa_import_error() from error

    if not Path(pdf_path).is_file():
        raise RuntimeError("PDF 文件不存在，无法建立索引。")

    settings = _build_settings(provider)
    docs = Docs()

    if hasattr(docs, "aadd"):
        await docs.aadd(pdf_path, settings=settings)
    else:
        await asyncio.to_thread(docs.add, pdf_path, settings=settings)

    _save_docs(index_path, docs)


def _stringify_answer(session: Any) -> str:
    for field in ("answer", "formatted_answer", "response"):
        value = getattr(session, field, None)
        if isinstance(value, str) and value.strip():
            return value.strip()

    if isinstance(session, str):
        return session.strip()

    return str(session).strip()


def _iter_contexts(session: Any) -> list[Any]:
    for field in ("contexts", "context", "sources", "references"):
        value = getattr(session, field, None)
        if not value:
            continue
        if isinstance(value, dict):
            return list(value.values())
        if isinstance(value, list | tuple | set):
            return list(value)
    return []


def _read_attr(value: Any, names: tuple[str, ...]) -> Any:
    for name in names:
        if isinstance(value, dict) and name in value:
            return value[name]
        if hasattr(value, name):
            return getattr(value, name)
    return None


def _citation_from_context(document_id: str, title: str, context: Any) -> QaCitation:
    text = _read_attr(context, ("text", "excerpt", "context", "summary")) or str(context)
    page = _read_attr(context, ("page", "page_number", "pages"))
    if isinstance(page, list | tuple):
        page = page[0] if page else None
    try:
        page = int(page) if page is not None else None
    except (TypeError, ValueError):
        page = None

    source_label = (
        _read_attr(context, ("source", "docname", "name", "citation", "key"))
        or title
    )
    confidence = _read_attr(context, ("score", "confidence"))
    try:
        confidence = float(confidence) if confidence is not None else None
    except (TypeError, ValueError):
        confidence = None

    return QaCitation(
        id=str(uuid.uuid4()),
        documentId=document_id,
        title=title,
        page=page,
        excerpt=str(text).strip()[:1200],
        sourceLabel=str(source_label),
        confidence=confidence,
    )


async def ask_document(
    document_id: str,
    title: str,
    question: str,
    index_path: str,
    provider: OpenAICompatibleProvider,
) -> tuple[str, list[QaCitation]]:
    docs = _load_docs(index_path)
    settings = _build_settings(provider)

    if hasattr(docs, "aquery"):
        session = await docs.aquery(question, settings=settings)
    else:
        session = await asyncio.to_thread(docs.query, question, settings=settings)

    answer = _stringify_answer(session)
    citations = [
        _citation_from_context(document_id, title, context)
        for context in _iter_contexts(session)
    ]

    return answer, citations
