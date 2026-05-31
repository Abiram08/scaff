"""Unit tests for token management modules."""

import json
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

from scaff import cache as scache
from scaff.pricing import PricingCalculator
from scaff.request_enforcer import Mode, ModeRegistry, RequestEnforcer
from scaff.session_manager import MemoryMonitor, MemoryProfile, SessionManager
from scaff.token_counter import TokenCounter
from scaff.token_tracker import TokenTracker


class TestTokenCounter(unittest.TestCase):
    """Test token counter functionality."""
    
    def setUp(self):
        self.counter = TokenCounter()
    
    def test_count_tokens_short_text(self):
        """Test token counting for short text."""
        text = "Hello, world!"
        tokens = self.counter.count_tokens(text)
        # ~4.5 chars per token for this text
        self.assertGreater(tokens, 0)
        self.assertLess(tokens, 10)
    
    def test_count_tokens_empty(self):
        """Test token counting for empty string."""
        tokens = self.counter.count_tokens("")
        self.assertEqual(tokens, 0)
    
    def test_estimate_response_tokens(self):
        """Test response token estimation."""
        input_tokens = 100
        max_tokens = 500
        output = self.counter.estimate_response_tokens(input_tokens, max_tokens)
        self.assertGreater(output, 0)
        self.assertLessEqual(output, max_tokens)
    
    def test_count_openai_request(self):
        """Test counting tokens in OpenAI request format."""
        messages = [
            {"role": "user", "content": "Hello, how are you?"},
            {"role": "assistant", "content": "I'm doing well, thank you!"},
        ]
        tokens = self.counter.count_openai_request(messages)
        self.assertGreater(tokens, 0)


class TestRequestEnforcer(unittest.TestCase):
    """Test request enforcer functionality."""
    
    def setUp(self):
        self.enforcer = RequestEnforcer()
    
    def test_mode_enum(self):
        """Test mode enum values."""
        self.assertEqual(Mode.MIN.value, "min")
        self.assertEqual(Mode.MEDIUM.value, "medium")
        self.assertEqual(Mode.MAX.value, "max")
    
    def test_get_config_min(self):
        """Test MIN mode configuration."""
        config = ModeRegistry.get_config(Mode.MIN)
        self.assertEqual(config.monthly_tokens, 1_000_000)
        self.assertEqual(config.monthly_cost, 0.25)
    
    def test_get_config_medium(self):
        """Test MEDIUM mode configuration."""
        config = ModeRegistry.get_config(Mode.MEDIUM)
        self.assertEqual(config.monthly_tokens, 5_000_000)
        self.assertEqual(config.monthly_cost, 1.25)
    
    def test_get_config_max(self):
        """Test MAX mode configuration."""
        config = ModeRegistry.get_config(Mode.MAX)
        self.assertEqual(config.monthly_tokens, 20_000_000)
        self.assertEqual(config.monthly_cost, 5.00)
    
    def test_check_budget_ok(self):
        """Test budget check when within limit."""
        self.enforcer.check_budget(1000)  # Should not raise
    
    def test_check_rate_limit(self):
        """Test rate limit check."""
        self.enforcer.check_rate_limit()  # Should not raise
    
    def test_record_request(self):
        """Test recording a request."""
        initial_tokens = self.enforcer.tokens_used
        self.enforcer.record_request(100)
        self.assertEqual(self.enforcer.tokens_used, initial_tokens + 100)


class TestTokenTracker(unittest.TestCase):
    """Test token tracker functionality."""
    
    def setUp(self):
        self.tmp_dir = tempfile.TemporaryDirectory()
        self.tracker = TokenTracker(
            data_file=Path(self.tmp_dir.name) / "test_tokens.json"
        )
    
    def tearDown(self):
        self.tmp_dir.cleanup()
    
    def test_start_session(self):
        """Test starting a session."""
        session_id = self.tracker.start_session("gpt-4o")
        self.assertIsNotNone(session_id)
    
    def test_record_request(self):
        """Test recording a request."""
        session_id = self.tracker.start_session("gpt-4o")
        self.tracker.record_request(
            session_id=session_id,
            model="gpt-4o",
            input_tokens=100,
            output_tokens=50,
            cost=0.05,
        )
        # Need to end session to save to file
        self.tracker.end_session()
        stats = self.tracker.get_monthly_stats()
        self.assertEqual(stats['total_tokens'], 150)
    
    def test_monthly_stats(self):
        """Test monthly statistics."""
        session_id = self.tracker.start_session("gpt-4o")
        self.tracker.record_request(
            session_id=session_id,
            model="gpt-4o",
            input_tokens=100,
            output_tokens=50,
            cost=0.05,
        )
        # Need to end session to save to file
        self.tracker.end_session()
        stats = self.tracker.get_monthly_stats()
        self.assertGreater(stats['total_tokens'], 0)
        self.assertGreater(stats['total_cost'], 0)


class TestSessionManager(unittest.TestCase):
    """Test session manager functionality."""
    
    def setUp(self):
        self.session = SessionManager(MemoryProfile.BALANCED)
    
    def test_memory_monitor(self):
        """Test memory monitoring."""
        memory_mb = MemoryMonitor.get_process_memory_mb()
        self.assertGreater(memory_mb, 0)
    
    def test_check_memory_limit(self):
        """Test memory limit checking."""
        result = self.session.check_memory_limit()
        self.assertIn('current_mb', result)
        self.assertIn('exceeds_limit', result)
    
    def test_session_stats(self):
        """Test session statistics."""
        stats = self.session.get_session_stats()
        self.assertIn('session_id', stats)
        self.assertIn('profile', stats)
        self.assertEqual(stats['profile'], 'balanced')
    
    def test_memory_profiles(self):
        """Test different memory profiles."""
        conservative = SessionManager(MemoryProfile.CONSERVATIVE)
        balanced = SessionManager(MemoryProfile.BALANCED)
        aggressive = SessionManager(MemoryProfile.AGGRESSIVE)
        
        self.assertLess(
            conservative.limits.max_memory_mb,
            balanced.limits.max_memory_mb
        )
        self.assertLess(
            balanced.limits.max_memory_mb,
            aggressive.limits.max_memory_mb
        )


class TestCache(unittest.TestCase):
    """Test production cache functionality."""
    
    def setUp(self):
        self.tmp_dir = tempfile.TemporaryDirectory()
        # Patch cache module's CACHE_DIR to use temp dir
        self._orig_cache_dir = scache.CACHE_DIR
        scache.CACHE_DIR = Path(self.tmp_dir.name) / "cache"
    
    def tearDown(self):
        scache.CACHE_DIR = self._orig_cache_dir
        self.tmp_dir.cleanup()
    
    def test_set_and_get(self):
        """Test setting and getting from cache."""
        scache.set("test description", "gpt-4o", '{"agent": "test"}')
        result = scache.get("test description", "gpt-4o")
        self.assertEqual(result, '{"agent": "test"}')
    
    def test_cache_clear(self):
        """Test clearing cache."""
        scache.set("desc1", "model1", "response1")
        scache.set("desc2", "model2", "response2")
        count = scache.clear()
        self.assertGreater(count, 0)
    
    def test_cache_status(self):
        """Test cache status reporting."""
        scache.set("desc", "model", "response")
        stats = scache.status()
        self.assertIn('entries', stats)
        self.assertIn('size_bytes', stats)
        self.assertGreater(stats['entries'], 0)


class TestPricingCalculator(unittest.TestCase):
    """Test pricing calculator functionality."""
    
    def test_calculate_request_cost_gpt4o(self):
        """Test cost calculation for GPT-4o."""
        cost = PricingCalculator.calculate_request_cost(
            "gpt-4o",
            input_tokens=1000,
            output_tokens=500,
        )
        self.assertGreater(cost, 0)
    
    def test_calculate_request_cost_mini(self):
        """Test cost calculation for GPT-4o-mini."""
        cost = PricingCalculator.calculate_request_cost(
            "gpt-4o-mini",
            input_tokens=10000,  # Larger amount to avoid rounding to 0
            output_tokens=5000,
        )
        self.assertGreater(cost, 0)
    
    def test_mini_cheaper_than_full(self):
        """Test that mini is cheaper than full model."""
        cost_full = PricingCalculator.calculate_request_cost(
            "gpt-4o",
            input_tokens=1000,
            output_tokens=500,
        )
        cost_mini = PricingCalculator.calculate_request_cost(
            "gpt-4o-mini",
            input_tokens=1000,
            output_tokens=500,
        )
        self.assertLess(cost_mini, cost_full)
    
    def test_calculate_monthly_agents(self):
        """Test calculating agents for a budget."""
        agents = PricingCalculator.calculate_monthly_agents(
            budget=1.0,
            model="gpt-4o-mini",
        )
        self.assertGreater(agents, 0)


if __name__ == "__main__":
    unittest.main()
