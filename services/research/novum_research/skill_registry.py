from __future__ import annotations

import os
import re
import shutil
from datetime import UTC, datetime
from pathlib import Path

from .schemas import ScienceSkill, SkillInputSpec

UPSTREAM_COMMIT = "33557e0f1faf0f281d255940de58935c61b2143b"


def repo_root() -> Path:
    configured = os.environ.get("NOVUM_REPO_ROOT")
    if configured:
        return Path(configured).expanduser().resolve()
    return Path(__file__).resolve().parents[3]


def science_skills_root() -> Path:
    return repo_root().joinpath("vendor", "science-skills")


def skills_root() -> Path:
    return science_skills_root().joinpath("skills")


def skill_markdown_path(skill_id: str) -> Path:
    safe_id = _normalize_skill_id(skill_id)
    path = skills_root().joinpath(safe_id, "SKILL.md")
    if not path.is_file():
        raise KeyError(skill_id)
    return path


def read_skill_markdown(skill_id: str) -> str:
    return skill_markdown_path(skill_id).read_text(encoding="utf-8")


def list_skills() -> list[ScienceSkill]:
    root = skills_root()
    if not root.is_dir():
        return []

    skills: list[ScienceSkill] = []
    for path in sorted(root.glob("*/SKILL.md")):
        skills.append(_parse_skill(path))
    return sorted(skills, key=lambda skill: (skill.domain, skill.name.lower()))


def get_skill(skill_id: str) -> ScienceSkill:
    safe_id = _normalize_skill_id(skill_id)
    for skill in list_skills():
        if skill.id == safe_id:
            return skill
    raise KeyError(skill_id)


def _normalize_skill_id(value: str) -> str:
    normalized = re.sub(r"[^a-zA-Z0-9_-]+", "-", value.strip()).strip("-").lower()
    if not normalized:
        raise KeyError(value)
    return normalized


def _parse_skill(path: Path) -> ScienceSkill:
    text = path.read_text(encoding="utf-8")
    metadata, body = _parse_frontmatter(text)
    skill_id = path.parent.name
    markdown_title = _first_heading(body) or skill_id.replace("_", " ").replace("-", " ").title()
    name = metadata.get("name") or markdown_title
    description = metadata.get("description") or _first_paragraph(body) or markdown_title
    required_env = _extract_required_env(text)
    mentions_uv = _mentions_uv(text)
    has_scripts = path.parent.joinpath("scripts").is_dir()
    execution_mode = _execution_mode(has_scripts, required_env)
    status = _status_for_skill(skill_id, mentions_uv, required_env)
    updated_at = datetime.fromtimestamp(path.stat().st_mtime, UTC).isoformat()

    return ScienceSkill(
        id=skill_id,
        name=name.strip(),
        description=_compact_text(description),
        domain=_infer_domain(skill_id, text),
        source="science-skills",
        sourcePath=str(path.relative_to(repo_root())),
        upstreamCommit=UPSTREAM_COMMIT,
        requiredInputs=[
            SkillInputSpec(
                name="task",
                label="任务上下文",
                type="textarea",
                required=True,
                defaultValue=None,
                help="描述你希望该技能处理的问题、对象或当前研究任务。",
            )
        ],
        requiredEnv=required_env,
        executionMode=execution_mode,
        status=status,
        updatedAt=updated_at,
    )


def _parse_frontmatter(text: str) -> tuple[dict[str, str], str]:
    if not text.startswith("---"):
        return {}, text

    lines = text.splitlines()
    closing_index: int | None = None
    for index, line in enumerate(lines[1:], start=1):
        if line.strip() == "---":
            closing_index = index
            break
    if closing_index is None:
        return {}, text

    metadata: dict[str, str] = {}
    current_key: str | None = None
    current_style = "plain"
    current_lines: list[str] = []

    def flush() -> None:
        nonlocal current_key, current_lines
        if current_key is None:
            return
        separator = "\n" if current_style == "literal" else " "
        metadata[current_key] = _compact_text(separator.join(current_lines))
        current_key = None
        current_lines = []

    for line in lines[1:closing_index]:
        if line and not line.startswith((" ", "\t")) and ":" in line:
            flush()
            key, raw_value = line.split(":", 1)
            current_key = key.strip()
            value = raw_value.strip()
            if value in {">", ">-"}:
                current_style = "folded"
                current_lines = []
            elif value in {"|", "|-"}:
                current_style = "literal"
                current_lines = []
            else:
                current_style = "plain"
                current_lines = [value.strip('"').strip("'")]
            continue

        if current_key is not None:
            current_lines.append(line.strip())
    flush()

    return metadata, "\n".join(lines[closing_index + 1 :])


def _first_heading(markdown: str) -> str | None:
    for line in markdown.splitlines():
        if line.startswith("# "):
            return line.removeprefix("# ").strip()
    return None


def _first_paragraph(markdown: str) -> str | None:
    lines: list[str] = []
    for line in markdown.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or stripped.startswith("---"):
            if lines:
                break
            continue
        if stripped.startswith(("```", "- ", "* ", "1.")):
            if lines:
                break
            continue
        lines.append(stripped)
    return " ".join(lines) if lines else None


def _compact_text(value: str) -> str:
    return re.sub(r"\s+", " ", value).strip()


def _extract_required_env(text: str) -> list[str]:
    matches = re.findall(
        r"\b[A-Z][A-Z0-9_]*(?:API_KEY|ACCESS_TOKEN|TOKEN|SECRET)\b",
        text,
    )
    ignored = {"API_KEY", "ACCESS_TOKEN", "TOKEN", "SECRET"}
    return sorted({match for match in matches if match not in ignored})


def _mentions_uv(text: str) -> bool:
    return bool(re.search(r"(^|\W)`?uv`?(\W|$)", text, flags=re.IGNORECASE))


def _execution_mode(has_scripts: bool, required_env: list[str]) -> str:
    if has_scripts and required_env:
        return "hybrid"
    if has_scripts:
        return "python"
    return "prompt"


def _status_for_skill(skill_id: str, mentions_uv: bool, required_env: list[str]) -> str:
    if skill_id == "uv":
        return "可用"
    if mentions_uv and shutil.which("uv") is None:
        return "缺少依赖"
    missing_env = [name for name in required_env if not os.environ.get(name)]
    if missing_env:
        return "需要配置"
    return "可用"


def _infer_domain(skill_id: str, text: str) -> str:
    probe = f"{skill_id} {text[:1200]}".lower()
    if any(term in probe for term in ("literature", "pubmed", "arxiv", "biorxiv", "openalex")):
        return "文献检索"
    if any(term in probe for term in ("protein", "uniprot", "pdb", "alphafold", "foldseek")):
        return "蛋白质"
    if any(
        term in probe
        for term in (
            "variant",
            "genome",
            "genomic",
            "gnomad",
            "clinvar",
            "dbsnp",
            "alphagenome",
            "encode",
            "gtex",
            "jaspar",
            "ucsc",
            "unibind",
            "ensembl",
        )
    ):
        return "基因组学"
    if any(term in probe for term in ("chembl", "pubchem", "chemistry", "compound")):
        return "化学"
    if any(term in probe for term in ("clinical", "openfda", "trial", "disease")):
        return "临床医学"
    if any(term in probe for term in ("ontology", "go ", "reactome", "interpro", "string")):
        return "生物知识库"
    if any(term in probe for term in ("uv", "workflow", "scienceskillscommon")):
        return "基础工具"
    return "科学技能"
