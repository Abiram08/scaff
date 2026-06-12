"""Tests for web search module."""
import pytest
from relay.web import generate_search_queries


def test_generate_search_queries_basic():
    queries = generate_search_queries("What is Harness CD?", ["How does canary work?"], 5)
    assert len(queries) >= 2
    assert "What is Harness CD?" in queries


def test_generate_search_queries_max_limit():
    sub_qs = [f"Question {i}" for i in range(10)]
    queries = generate_search_queries("Main question", sub_qs, 3)
    assert len(queries) <= 3


def test_generate_search_queries_deduplicates():
    sub_qs = ["What is Harness CD?"]
    queries = generate_search_queries("What is Harness CD?", sub_qs, 5)
    assert queries.count("What is Harness CD?") == 1


def test_generate_search_queries_extracts_terms():
    queries = generate_search_queries(
        "What is Harness Continuous Delivery?",
        [],
        5,
    )
    assert len(queries) >= 1
