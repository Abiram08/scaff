"""Tests for Relay pipeline types."""
import pytest
from relay.types import (
    Confidence, Stance, SourceKind, SourceTag, SessionStatus,
    Session, Finding, Claim, Source, SubQuestion,
)


def test_confidence_from_score():
    assert Confidence.from_score(0.9, False) == Confidence.HIGH
    assert Confidence.from_score(0.7, False) == Confidence.MEDIUM
    assert Confidence.from_score(0.3, False) == Confidence.LOW
    assert Confidence.from_score(0.9, True) == Confidence.CONTESTED


def test_confidence_emoji():
    assert Confidence.HIGH.emoji() == "✅"
    assert Confidence.MEDIUM.emoji() == "⚠️"
    assert Confidence.LOW.emoji() == "🔴"
    assert Confidence.CONTESTED.emoji() == "⚡"


def test_session_create():
    session = Session.create("What is Harness CD?")
    assert session.id.startswith("sess-")
    assert session.query == "What is Harness CD?"
    assert session.status == SessionStatus.PLANNING


def test_session_update_status():
    session = Session.create("test")
    session.update_status(SessionStatus.SEARCHING)
    assert session.status == SessionStatus.SEARCHING


def test_session_add_finding():
    session = Session.create("test")
    finding = Finding(
        index=1,
        total=1,
        confidence=Confidence.HIGH,
        content="Test finding",
        sources=[],
    )
    session.add_finding(finding)
    assert len(session.findings) == 1


def test_session_to_dict():
    session = Session.create("test")
    d = session.to_dict()
    assert "id" in d
    assert d["query"] == "test"
    assert d["status"] == "planning"


def test_claim_creation():
    claim = Claim(
        id=0,
        text="Test claim",
        source_ids=[0, 1],
        confidence=Confidence.HIGH,
    )
    assert claim.text == "Test claim"
    assert len(claim.source_ids) == 2


def test_source_creation():
    source = Source(
        id=0,
        title="Test Source",
        url="http://example.com",
        kind=SourceKind.WEB,
    )
    assert source.title == "Test Source"
    assert source.kind == SourceKind.WEB


def test_sub_question_creation():
    sq = SubQuestion(
        id="sq_001",
        question="How does X work?",
        source_tag=SourceTag.BOTH,
    )
    assert sq.source_tag == SourceTag.BOTH
