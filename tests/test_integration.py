"""Integration tests for all scaff modules working together."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch, MagicMock

from scaff.analytics import Analytics
from scaff import cache as scache
from scaff.pricing import PricingCalculator
from scaff.request_enforcer import Mode, RequestEnforcer
from scaff.session_manager import SessionManager, MemoryProfile
from scaff.state_manager import StateManager, SessionState
from scaff.telemetry import Telemetry
from scaff.token_counter import TokenCounter
from scaff.token_tracker import TokenTracker


class TestIntegration(unittest.TestCase):
    """Integration tests for all modules working together"""
    
    def setUp(self):
        """Set up test fixtures"""
        self.temp_dir = tempfile.TemporaryDirectory()
        self.temp_path = Path(self.temp_dir.name)
    
    def tearDown(self):
        """Clean up"""
        scache.CACHE_DIR = getattr(self, '_orig_cache_dir', scache.CACHE_DIR)
        self.temp_dir.cleanup()
    
    def test_full_agent_generation_workflow(self):
        """Test complete agent generation workflow"""
        # Initialize all components
        session_mgr = SessionManager(MemoryProfile.BALANCED)
        token_counter = TokenCounter()
        request_enforcer = RequestEnforcer(Mode.MEDIUM)
        token_tracker = TokenTracker(data_file=self.temp_path / "tokens.json")
        cache_path = Path(self.temp_path / "cache")
        cache_path.mkdir(parents=True, exist_ok=True)
        self._orig_cache_dir = scache.CACHE_DIR
        scache.CACHE_DIR = cache_path
        state_mgr = StateManager(state_dir=self.temp_path)
        telemetry = Telemetry(telemetry_dir=self.temp_path / "telemetry")
        analytics = Analytics(analytics_dir=self.temp_path / "analytics")
        
        # Simulate an agent generation request
        description = "Create an AI agent that summarizes emails"
        
        # Token counting
        input_tokens = token_counter.count_tokens(description)
        self.assertGreater(input_tokens, 0)
        
        # Check request limits
        try:
            request_enforcer.check_budget(input_tokens + 1000)
        except ValueError:
            self.fail("Request enforcer should allow request within budget")
        
        # Start session
        session_id = token_tracker.start_session("gpt-4o")
        self.assertIsNotNone(session_id)
        
        # Record API call
        output_tokens = 1500
        cost = PricingCalculator.calculate_request_cost(
            "gpt-4o", input_tokens, output_tokens
        )
        
        token_tracker.record_request(
            session_id=session_id,
            model="gpt-4o",
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            cost=cost,
        )
        
        request_enforcer.record_request(input_tokens + output_tokens)
        
        # Log telemetry
        telemetry.log_event(
            "agent_generation",
            {
                "session_id": session_id,
                "description": description,
                "model": "gpt-4o",
            }
        )
        
        telemetry.record_performance(
            "agent_generation",
            duration_ms=5000,
            success=True,
        )
        
        analytics.log_api_call(
            model="gpt-4o",
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            latency_ms=5000,
            cost_usd=cost,
            success=True,
        )
        
        # End session
        token_tracker.end_session()
        
        # Update state
        state_mgr.increment("total_agents_generated")
        state_mgr.set("last_session_id", session_id)
        
        # Verify results
        monthly_stats = token_tracker.get_monthly_stats()
        self.assertGreater(monthly_stats['total_tokens'], 0)
        self.assertEqual(monthly_stats['session_count'], 1)
        
        # Verify cache is available
        cache_description = "test agent"
        cache_value = '{"agent": "test"}'
        scache.set(cache_description, "gpt-4o", cache_value)
        retrieved = scache.get(cache_description, "gpt-4o")
        self.assertEqual(retrieved, cache_value)
        
        # Verify state
        all_state = state_mgr.get_all()
        self.assertEqual(all_state['total_agents_generated'], 1)
        self.assertEqual(all_state['last_session_id'], session_id)
        
        # Verify analytics
        daily_stats = analytics.get_daily_stats()
        self.assertEqual(daily_stats['api_calls'], 1)
        self.assertEqual(daily_stats['successful_calls'], 1)
    
    def test_memory_limiting(self):
        """Test memory limiting across operations"""
        session_mgr = SessionManager(MemoryProfile.CONSERVATIVE)
        
        # Check memory limit
        result = session_mgr.check_memory_limit()
        self.assertIn('current_mb', result)
        self.assertIn('exceeds_limit', result)
        
        # Should not exceed conservative limit (256MB)
        self.assertFalse(result['exceeds_limit'])
    
    def test_rate_limiting(self):
        """Test rate limiting enforcement"""
        enforcer = RequestEnforcer(Mode.MIN)
        
        # MIN mode allows 6 req/min
        # Record requests
        for i in range(6):
            enforcer.record_request(100)
        
        # Check budget still valid
        self.assertLess(enforcer.tokens_used, enforcer.config.monthly_tokens)
        
        # Verify rate limit tokens consumed
        self.assertEqual(enforcer.tokens_used, 600)
    
    def test_budget_tracking(self):
        """Test budget tracking across requests"""
        tracker = TokenTracker(data_file=self.temp_path / "tokens.json")
        
        session_id = tracker.start_session("gpt-4o")
        
        # Record multiple requests
        for i in range(3):
            tracker.record_request(
                session_id=session_id,
                model="gpt-4o",
                input_tokens=100,
                output_tokens=200,
                cost=0.01,
            )
        
        tracker.end_session()
        
        stats = tracker.get_monthly_stats()
        self.assertEqual(stats['total_tokens'], 900)  # 3 * 300
    
    def test_error_tracking(self):
        """Test error tracking"""
        telemetry = Telemetry(telemetry_dir=self.temp_path / "telemetry")
        
        telemetry.report_error(
            error_type="APIError",
            error_message="Rate limit exceeded",
            context={"model": "gpt-4o"},
        )
        
        telemetry.report_error(
            error_type="ValidationError",
            error_message="Invalid configuration",
        )
        
        summary = telemetry.get_error_summary()
        self.assertEqual(summary['total_errors'], 2)
        self.assertEqual(len(summary['error_types']), 2)
    
    def test_performance_metrics(self):
        """Test performance tracking"""
        telemetry = Telemetry(telemetry_dir=self.temp_path / "telemetry")
        
        telemetry.record_performance(
            "token_counting",
            duration_ms=50,
            success=True,
        )
        
        telemetry.record_performance(
            "api_call",
            duration_ms=5000,
            success=True,
        )
        
        telemetry.record_performance(
            "file_writing",
            duration_ms=100,
            success=True,
        )
        
        stats = telemetry.get_performance_stats()
        self.assertEqual(len(stats), 3)
        self.assertEqual(stats['token_counting']['count'], 1)
    
    def test_session_state_persistence(self):
        """Test session state persistence"""
        session_id = "test-session-123"
        session_state = SessionState(
            session_id,
            state_dir=self.temp_path / "sessions"
        )
        
        # Set values
        session_state.set("agent_name", "EmailSummarizer")
        session_state.set("model", "gpt-4o")
        session_state.set("tokens_used", 1500)
        
        # Create new instance and verify persistence
        session_state2 = SessionState(
            session_id,
            state_dir=self.temp_path / "sessions"
        )
        
        self.assertEqual(session_state2.get("agent_name"), "EmailSummarizer")
        self.assertEqual(session_state2.get("model"), "gpt-4o")
        self.assertEqual(session_state2.get("tokens_used"), 1500)
    
    def test_cache_basic_operations(self):
        """Test basic cache operations"""
        test_dir = Path(self.temp_path / "test_cache")
        test_dir.mkdir(parents=True, exist_ok=True)
        orig_dir = scache.CACHE_DIR
        scache.CACHE_DIR = test_dir
        
        try:
            # Cache miss (not in cache)
            result1 = scache.get("test_key", "gpt-4o")
            self.assertIsNone(result1)
            
            # Cache set
            scache.set("test_key", "gpt-4o", "response_value")
            
            # Cache hit
            result2 = scache.get("test_key", "gpt-4o")
            self.assertEqual(result2, "response_value")
            
            # Cache miss - different description
            result3 = scache.get("nonexistent", "gpt-4o")
            self.assertIsNone(result3)
        finally:
            scache.CACHE_DIR = orig_dir
    
    def test_model_pricing_comparison(self):
        """Test pricing calculations across models"""
        gpt4o_cost = PricingCalculator.calculate_request_cost(
            "gpt-4o",
            input_tokens=1000,
            output_tokens=1000,
        )
        
        mini_cost = PricingCalculator.calculate_request_cost(
            "gpt-4o-mini",
            input_tokens=1000,
            output_tokens=1000,
        )
        
        # Mini should be cheaper
        self.assertLess(mini_cost, gpt4o_cost)
        
        # Monthly agents estimate
        mini_agents = PricingCalculator.calculate_monthly_agents(
            budget=5.0,
            model="gpt-4o-mini",
        )
        
        self.assertGreater(mini_agents, 0)
    
    def test_config_persistence(self):
        """Test configuration persistence"""
        state_mgr = StateManager(state_dir=self.temp_path)
        
        # Set configuration
        state_mgr.set("current_mode", "max")
        state_mgr.set("cache_enabled", False)
        state_mgr.set("verbose_mode", True)
        
        # Create new instance
        state_mgr2 = StateManager(state_dir=self.temp_path)
        
        # Verify persistence
        self.assertEqual(state_mgr2.get("current_mode"), "max")
        self.assertEqual(state_mgr2.get("cache_enabled"), False)
        self.assertEqual(state_mgr2.get("verbose_mode"), True)


if __name__ == "__main__":
    unittest.main()
