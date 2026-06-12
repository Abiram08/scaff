"""Tests for corpus store."""
import pytest
import tempfile
from pathlib import Path
from relay.corpus.store import CorpusStore, CorpusChunk


@pytest.fixture
def temp_db():
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        yield db_path


@pytest.mark.asyncio
async def test_store_init(temp_db):
    store = CorpusStore(str(temp_db))
    await store.init_db()
    count = await store.count()
    assert count == 0


@pytest.mark.asyncio
async def test_store_insert_and_count(temp_db):
    store = CorpusStore(str(temp_db))
    await store.init_db()

    chunks = [
        CorpusChunk(id="test_1", url="http://a.com", title="A", section=None, content="Content A"),
        CorpusChunk(id="test_2", url="http://b.com", title="B", section="Sec", content="Content B"),
    ]
    await store.insert_chunks(chunks)

    count = await store.count()
    assert count == 2


@pytest.mark.asyncio
async def test_store_search_fts(temp_db):
    store = CorpusStore(str(temp_db))
    await store.init_db()

    chunks = [
        CorpusChunk(id="test_1", url="http://a.com", title="Canary Deployment", section=None, content="Canary is a deployment strategy"),
        CorpusChunk(id="test_2", url="http://b.com", title="Blue Green", section=None, content="Blue green is another strategy"),
    ]
    await store.insert_chunks(chunks)

    results = await store.search_fts("canary", limit=5)
    assert len(results) >= 1
    assert any("canary" in r.title.lower() for r in results)


@pytest.mark.asyncio
async def test_store_get_all(temp_db):
    store = CorpusStore(str(temp_db))
    await store.init_db()

    chunks = [
        CorpusChunk(id="test_1", url="http://a.com", title="A", section=None, content="Content"),
    ]
    await store.insert_chunks(chunks)

    all_chunks = await store.get_all_chunks()
    assert len(all_chunks) == 1


@pytest.mark.asyncio
async def test_store_clear(temp_db):
    store = CorpusStore(str(temp_db))
    await store.init_db()

    chunks = [CorpusChunk(id="test_1", url="http://a.com", title="A", section=None, content="Content")]
    await store.insert_chunks(chunks)

    await store.clear()
    count = await store.count()
    assert count == 0
