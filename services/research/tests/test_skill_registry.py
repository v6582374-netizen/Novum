from novum_research.schemas import RunSkillRequest, SkillRunContext
from novum_research.skill_registry import get_skill, list_skills
from novum_research.skill_runner import run_skill


def test_science_skills_snapshot_is_parsed() -> None:
    skills = list_skills()

    assert len(skills) >= 30
    assert any(skill.id == "uniprot_database" for skill in skills)


def test_frontmatter_name_and_description_are_used() -> None:
    skill = get_skill("uniprot_database")

    assert skill.name == "uniprot-database"
    assert "UniProtKB" in skill.description
    assert skill.source_path.endswith("skills/uniprot_database/SKILL.md")


def test_required_env_is_detected() -> None:
    skill = get_skill("alphagenome_single_variant_analysis")

    assert "ALPHAGENOME_API_KEY" in skill.required_env


def test_dry_run_skill_execution_succeeds_for_uv() -> None:
    run = run_skill(
        "uv",
        RunSkillRequest(
            inputs={"task": "检查 uv 是否可用"},
            context=SkillRunContext(
                activeDocumentId=None,
                activeDocumentPath=None,
                selectedText=None,
                provider=None,
            ),
        ),
    )

    assert run.status == "succeeded"
    assert run.outputs
