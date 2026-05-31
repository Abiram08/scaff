use std::time::Instant;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    Min,
    Medium,
    Max,
}

impl Mode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "min" => Mode::Min,
            "max" => Mode::Max,
            _ => Mode::Medium,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Min => "min",
            Mode::Medium => "medium",
            Mode::Max => "max",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModeConfig {
    pub mode: Mode,
    pub monthly_tokens: u64,
    pub monthly_cost: f64,
    pub rate_limit_per_minute: u64,
    pub concurrent_limit: u64,
    pub cache_ttl_days: u64,
    pub model: &'static str,
    pub temperature: f64,
    pub max_tokens: u64,
}

impl ModeConfig {
    pub fn for_mode(mode: Mode) -> Self {
        match mode {
            Mode::Min => ModeConfig {
                mode: Mode::Min,
                monthly_tokens: 1_000_000,
                monthly_cost: 0.25,
                rate_limit_per_minute: 6,
                concurrent_limit: 1,
                cache_ttl_days: 30,
                model: "gpt-4o-mini",
                temperature: 0.3,
                max_tokens: 2048,
            },
            Mode::Medium => ModeConfig {
                mode: Mode::Medium,
                monthly_tokens: 5_000_000,
                monthly_cost: 1.25,
                rate_limit_per_minute: 12,
                concurrent_limit: 2,
                cache_ttl_days: 14,
                model: "gpt-4o",
                temperature: 0.7,
                max_tokens: 4096,
            },
            Mode::Max => ModeConfig {
                mode: Mode::Max,
                monthly_tokens: 20_000_000,
                monthly_cost: 5.00,
                rate_limit_per_minute: 30,
                concurrent_limit: 5,
                cache_ttl_days: 7,
                model: "gpt-4o",
                temperature: 1.0,
                max_tokens: 4096,
            },
        }
    }
}

pub struct TokenBucket {
    capacity: f64,
    refill_rate: f64,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            capacity,
            refill_rate,
            tokens: capacity,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = Instant::now();
    }

    pub fn acquire(&mut self, tokens: f64) -> bool {
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    pub fn peek(&self) -> f64 {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        (self.tokens + elapsed * self.refill_rate).min(self.capacity)
    }

    pub fn utilization(&mut self) -> f64 {
        self.refill();
        if self.capacity <= 0.0 {
            return 0.0;
        }
        1.0 - (self.tokens / self.capacity)
    }
}

pub struct RateLimiter {
    pub mode: Mode,
    pub config: ModeConfig,
    bucket: TokenBucket,
    pub monthly_tokens_used: u64,
}

impl RateLimiter {
    pub fn new(mode: Mode) -> Self {
        let config = ModeConfig::for_mode(mode);
        let bucket = TokenBucket::new(
            config.rate_limit_per_minute as f64,
            config.rate_limit_per_minute as f64 / 60.0,
        );
        Self {
            mode,
            config,
            bucket,
            monthly_tokens_used: 0,
        }
    }

    pub fn with_usage(mode: Mode, monthly_tokens_used: u64) -> Self {
        let mut rl = Self::new(mode);
        rl.monthly_tokens_used = monthly_tokens_used;
        rl
    }

    pub fn check_budget(&self, estimated_tokens: u64) -> Result<(), String> {
        let remaining = self.config.monthly_tokens.saturating_sub(self.monthly_tokens_used);
        if estimated_tokens > remaining {
            return Err(format!(
                "Monthly budget exhausted. {} tokens remaining (need ~{}).\n\
                 Switch mode with: scaff mode min",
                remaining, estimated_tokens
            ));
        }
        Ok(())
    }

    pub fn check_rate_limit(&mut self) -> Result<(), String> {
        if !self.bucket.acquire(1.0) {
            return Err("Rate limited. Try again in a moment.".to_string());
        }
        Ok(())
    }

    pub fn record_request(&mut self, tokens: u64) {
        self.monthly_tokens_used += tokens;
    }

    pub fn remaining_budget(&self) -> u64 {
        self.config.monthly_tokens.saturating_sub(self.monthly_tokens_used)
    }

    pub fn budget_percent_used(&self) -> f64 {
        if self.config.monthly_tokens == 0 {
            return 0.0;
        }
        (self.monthly_tokens_used as f64 / self.config.monthly_tokens as f64) * 100.0
    }

    pub fn utilization_percent(&mut self) -> f64 {
        self.bucket.utilization() * 100.0
    }
}
