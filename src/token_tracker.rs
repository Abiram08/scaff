use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRecord {
    pub session_id: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenTrackerData {
    pub sessions: HashMap<String, Vec<TokenRecord>>,
    pub records: Vec<TokenRecord>,
    pub current_session_id: String,
}

impl Default for TokenTrackerData {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            records: Vec::new(),
            current_session_id: Uuid::new_v4().to_string(),
        }
    }
}

pub struct TokenTracker {
    data: TokenTrackerData,
    path: PathBuf,
}

impl TokenTracker {
    pub fn new() -> Self {
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        let path = base.join(".scaff").join("token_tracker.json");
        let data = if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            TokenTrackerData::default()
        };
        Self { data, path }
    }

    fn save(&self) {
        if let Ok(content) = serde_json::to_string_pretty(&self.data) {
            if let Some(dir) = self.path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&self.path, content);
        }
    }

    pub fn record_request(&mut self, model: &str, input_tokens: u64, output_tokens: u64, cost: f64) {
        let record = TokenRecord {
            session_id: self.data.current_session_id.clone(),
            model: model.to_string(),
            input_tokens,
            output_tokens,
            cost,
            timestamp: Utc::now().to_rfc3339(),
        };
        self.data
            .sessions
            .entry(self.data.current_session_id.clone())
            .or_default()
            .push(record.clone());
        self.data.records.push(record);
        self.save();
    }

    pub fn get_monthly_stats(&self) -> MonthlyStats {
        let now = Utc::now();
        let mut total_tokens = 0u64;
        let mut total_cost = 0.0;
        let mut session_count = 0u64;
        let mut sessions: Vec<&Vec<TokenRecord>> = Vec::new();

        for (_sid, records) in &self.data.sessions {
            let mut session_tokens = 0u64;
            let mut session_cost = 0.0;
            let mut has_current_month = false;
            for r in records {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&r.timestamp) {
                    if dt.year() == now.year() && dt.month() == now.month() {
                        session_tokens += r.input_tokens + r.output_tokens;
                        session_cost += r.cost;
                        has_current_month = true;
                    }
                }
            }
            if has_current_month && session_tokens > 0 {
                total_tokens += session_tokens;
                total_cost += session_cost;
                session_count += 1;
                sessions.push(records);
            }
        }

        let days_elapsed = now.day().max(1) as f64;
        let days_in_month = 30.0;
        let projected_monthly = if days_elapsed > 0.0 {
            (total_tokens as f64 / days_elapsed) * days_in_month
        } else {
            0.0
        };
        let projected_cost = if days_elapsed > 0.0 {
            (total_cost / days_elapsed) * days_in_month
        } else {
            0.0
        };

        MonthlyStats {
            total_tokens,
            total_cost: (total_cost * 100.0).round() / 100.0,
            session_count,
            projected_monthly: projected_monthly.round() as u64,
            projected_cost: (projected_cost * 100.0).round() / 100.0,
            daily_burn: if days_elapsed > 0.0 {
                (total_tokens as f64 / days_elapsed).round() as u64
            } else {
                0
            },
            daily_cost: if days_elapsed > 0.0 {
                (total_cost / days_elapsed * 100.0).round() / 100.0
            } else {
                0.0
            },
        }
    }

    pub fn get_monthly_stats_for(&self, year: i32, month: u32) -> MonthlyStats {
        let mut total_tokens = 0u64;
        let mut total_cost = 0.0;
        let mut session_count = 0u64;

        for (_sid, records) in &self.data.sessions {
            let mut s_tokens = 0u64;
            let mut s_cost = 0.0;
            let mut in_month = false;
            for r in records {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&r.timestamp) {
                    if dt.year() == year && dt.month() == month {
                        s_tokens += r.input_tokens + r.output_tokens;
                        s_cost += r.cost;
                        in_month = true;
                    }
                }
            }
            if in_month && s_tokens > 0 {
                total_tokens += s_tokens;
                total_cost += s_cost;
                session_count += 1;
            }
        }

        MonthlyStats {
            total_tokens,
            total_cost: (total_cost * 100.0).round() / 100.0,
            session_count,
            projected_monthly: 0,
            projected_cost: 0.0,
            daily_burn: 0,
            daily_cost: 0.0,
        }
    }

    pub fn get_all_time_stats(&self) -> AllTimeStats {
        let mut total_tokens = 0u64;
        let mut total_cost = 0.0;
        let mut total_sessions = 0u64;

        for (_, records) in &self.data.sessions {
            let mut session_tokens = 0u64;
            let mut session_cost = 0.0;
            for r in records {
                session_tokens += r.input_tokens + r.output_tokens;
                session_cost += r.cost;
            }
            if session_tokens > 0 {
                total_tokens += session_tokens;
                total_cost += session_cost;
                total_sessions += 1;
            }
        }

        let avg_cost = if total_sessions > 0 {
            total_cost / total_sessions as f64
        } else {
            0.0
        };

        AllTimeStats {
            total_tokens,
            total_cost: (total_cost * 100.0).round() / 100.0,
            total_sessions,
            average_cost_per_session: (avg_cost * 10000.0).round() / 10000.0,
        }
    }

    pub fn export_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.data)
            .map_err(|e| format!("Failed to serialize token data: {e}"))
    }

    pub fn export_csv(&self) -> Result<String, String> {
        let mut csv = String::from("session_id,model,input_tokens,output_tokens,total_tokens,cost,timestamp\n");
        for r in &self.data.records {
            csv.push_str(&format!(
                "{},{},{},{},{},{:.6},{}\n",
                r.session_id,
                r.model,
                r.input_tokens,
                r.output_tokens,
                r.input_tokens + r.output_tokens,
                r.cost,
                r.timestamp,
            ));
        }
        Ok(csv)
    }

    pub fn export_csv_for_session(&self, session_id: &str) -> Result<String, String> {
        let records = self.data.sessions.get(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;
        let mut csv = String::from("model,input_tokens,output_tokens,total_tokens,cost,timestamp\n");
        for r in records {
            csv.push_str(&format!(
                "{},{},{},{},{:.6},{}\n",
                r.model,
                r.input_tokens,
                r.output_tokens,
                r.input_tokens + r.output_tokens,
                r.cost,
                r.timestamp,
            ));
        }
        Ok(csv)
    }

    pub fn monthly_tokens_used(&self) -> u64 {
        let now = Utc::now();
        let mut total = 0u64;
        for r in &self.data.records {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&r.timestamp) {
                if dt.year() == now.year() && dt.month() == now.month() {
                    total += r.input_tokens + r.output_tokens;
                }
            }
        }
        total
    }

    pub fn session_id(&self) -> &str {
        &self.data.current_session_id
    }
}

#[derive(Debug)]
pub struct MonthlyStats {
    pub total_tokens: u64,
    pub total_cost: f64,
    pub session_count: u64,
    pub projected_monthly: u64,
    pub projected_cost: f64,
    pub daily_burn: u64,
    pub daily_cost: f64,
}

#[derive(Debug)]
pub struct AllTimeStats {
    pub total_tokens: u64,
    pub total_cost: f64,
    pub total_sessions: u64,
    pub average_cost_per_session: f64,
}
