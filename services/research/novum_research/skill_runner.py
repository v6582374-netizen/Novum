from __future__ import annotations

import re
import uuid
from datetime import UTC, datetime

from .schemas import (
    RunSkillRequest,
    SkillRun,
    SkillRunLog,
    SkillRunOutput,
)
from .skill_registry import get_skill

RUNS: dict[str, SkillRun] = {}


def _now() -> str:
    return datetime.now(UTC).isoformat()


def _log(level: str, message: str) -> SkillRunLog:
    return SkillRunLog(timestamp=_now(), level=level, message=_redact(message))


def _redact(value: str) -> str:
    patterns = [
        r"(api[_-]?key\s*[=:]\s*)[^\s,;]+",
        r"(authorization\s*:\s*bearer\s+)[^\s,;]+",
        r"([A-Z0-9_]*(?:API_KEY|TOKEN|SECRET)\s*[=:]\s*)[^\s,;]+",
    ]
    result = value
    for pattern in patterns:
        result = re.sub(pattern, r"\1[REDACTED]", result, flags=re.IGNORECASE)
    return result


def run_skill(skill_id: str, request: RunSkillRequest) -> SkillRun:
    skill = get_skill(skill_id)
    started_at = _now()
    run = SkillRun(
        id=str(uuid.uuid4()),
        skillId=skill.id,
        skillName=skill.name,
        status="running",
        startedAt=started_at,
        finishedAt=None,
        error=None,
        logs=[_log("info", "技能运行已进入受控 dry-run。")],
        outputs=[],
    )
    RUNS[run.id] = run

    task = str(request.inputs.get("task") or "").strip()
    if not task:
        return _finish_failed(run, "任务上下文不能为空。")

    if skill.status != "可用":
        if skill.status == "缺少依赖":
            return _finish_failed(run, "该技能缺少本地依赖。请先安装 uv 或按技能说明补齐依赖。")
        if skill.status == "需要配置":
            missing = "、".join(skill.required_env)
            return _finish_failed(run, f"该技能需要先配置环境变量：{missing}。")
        return _finish_failed(run, "该技能当前不可用。")

    output = _build_dry_run_output(skill_id=skill.id, request=request)
    run.outputs.append(
        SkillRunOutput(
            id=str(uuid.uuid4()),
            kind="markdown",
            title="技能 dry-run 执行计划",
            content=output,
            filePath=None,
        )
    )
    run.status = "succeeded"
    run.finished_at = _now()
    run.logs.append(_log("info", "技能 dry-run 已完成。"))
    RUNS[run.id] = run
    return run


def get_run(run_id: str) -> SkillRun | None:
    return RUNS.get(run_id)


def _finish_failed(run: SkillRun, message: str) -> SkillRun:
    run.status = "failed"
    run.finished_at = _now()
    run.error = message
    run.logs.append(_log("error", message))
    RUNS[run.id] = run
    return run


def _build_dry_run_output(skill_id: str, request: RunSkillRequest) -> str:
    skill = get_skill(skill_id)
    context = request.context
    active_document = context.active_document_id or "未绑定当前文献"
    active_document_path = context.active_document_path or "无"
    task = _redact(str(request.inputs.get("task") or "").strip())
    provider_state = "已提供" if context.provider else "未提供"

    return "\n".join(
        [
            f"# {skill.name}",
            "",
            "## dry-run 结果",
            "",
            "Novum 已完成技能选择、上下文绑定和依赖检查。为避免前端或用户输入触发任意 shell，本阶段不会直接执行上游脚本；后续真实执行必须经过白名单 runner。",
            "",
            "## 技能信息",
            "",
            f"- 技能 ID：`{skill.id}`",
            f"- 领域：{skill.domain}",
            f"- 来源：{skill.source_path}",
            f"- 执行模式：{skill.execution_mode}",
            f"- 状态：{skill.status}",
            "",
            "## 当前上下文",
            "",
            f"- 当前文献 ID：{active_document}",
            f"- 当前文献路径：{active_document_path}",
            f"- 模型 provider：{provider_state}",
            "",
            "## 用户任务",
            "",
            task,
            "",
            "## 下一步执行边界",
            "",
            "- 只能运行已登记的 `science-skills` 技能。",
            "- `scripts/` 和 `references/` 保留为运行依赖，但不作为普通 UI 文件树暴露。",
            "- 真实脚本执行前必须补齐参数 schema、依赖检查、日志脱敏和产物路径记录。",
        ]
    )
