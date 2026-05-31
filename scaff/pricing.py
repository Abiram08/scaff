"""Pricing calculator for OpenAI API calls."""

from enum import Enum


class PricingModel(Enum):
    """OpenAI pricing models"""
    GPT_4O_INPUT = 0.0025  # $2.50 per 1M
    GPT_4O_OUTPUT = 0.010  # $10.00 per 1M
    GPT_4O_MINI_INPUT = 0.00015  # $0.15 per 1M
    GPT_4O_MINI_OUTPUT = 0.0006  # $0.60 per 1M


class PricingCalculator:
    """Calculate costs for API calls"""
    
    @staticmethod
    def calculate_request_cost(
        model: str,
        input_tokens: int,
        output_tokens: int,
    ) -> float:
        """Calculate cost for a request"""
        if "mini" in model.lower():
            input_cost = (input_tokens / 1_000_000) * PricingModel.GPT_4O_MINI_INPUT.value
            output_cost = (output_tokens / 1_000_000) * PricingModel.GPT_4O_MINI_OUTPUT.value
        else:
            input_cost = (input_tokens / 1_000_000) * PricingModel.GPT_4O_INPUT.value
            output_cost = (output_tokens / 1_000_000) * PricingModel.GPT_4O_OUTPUT.value
        
        total = input_cost + output_cost
        return round(total, 6)  # Keep more decimals for small amounts
    
    @staticmethod
    def calculate_monthly_agents(budget: float, model: str = "gpt-4o") -> int:
        """Estimate how many agents can be generated with a budget"""
        avg_input_tokens = 500
        avg_output_tokens = 1000
        
        cost_per_agent = PricingCalculator.calculate_request_cost(
            model, avg_input_tokens, avg_output_tokens
        )
        
        # Return int, minimum 1 if cost < budget
        if cost_per_agent <= 0:
            return int(budget * 1000)  # Default estimate
        
        agents = int(budget / cost_per_agent)
        return max(1, agents) if budget > 0 else 0
