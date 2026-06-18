from novum_research.schemas import OpenAICompatibleProvider, QaCitation


def test_provider_accepts_frontend_aliases() -> None:
    provider = OpenAICompatibleProvider(
        baseUrl="https://api.example.com/v1",
        model="openai/gpt-4o-mini",
        apiKey="secret",
    )

    assert provider.base_url == "https://api.example.com/v1"
    assert provider.api_key == "secret"


def test_citation_serializes_frontend_aliases() -> None:
    citation = QaCitation(
        id="c1",
        documentId="doc1",
        title="Paper",
        page=3,
        excerpt="source text",
        sourceLabel="p.3",
    )

    payload = citation.model_dump(by_alias=True)

    assert payload["documentId"] == "doc1"
    assert payload["sourceLabel"] == "p.3"
