use super::{max_option_i64, CodexTokenTotals, ConfirmedTokenDelta};
use serde::Serialize;
use std::collections::HashMap;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const TPS_MIN_RESPONSE_MS: i64 = 200;
const TPS_MAX_RESPONSE_MS: i64 = 10 * 60 * 1000;
const TPS_MIN_TOTAL_DURATION_MS: i64 = 1000;

/// Per-account usage attributed by the `creator_account_id` that Codex writes
/// into each rollout's `session_meta`. A session resumed after a switch stays
/// with the account that created it.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexAccountTokenUsage {
    pub(crate) account_id: String,
    pub(crate) last_24h: CodexTokenTotals,
    pub(crate) last_7d: CodexTokenTotals,
    pub(crate) last_30d: CodexTokenTotals,
    pub(crate) last_active_at: Option<i64>,
    pub(crate) response_count_24h: usize,
    /// Output tokens per second over model responses in the last 24 hours,
    /// measured from turn/previous-response boundary to the token_count event.
    pub(crate) output_tps_24h: Option<f64>,
    pub(crate) output_tps_7d: Option<f64>,
    pub(crate) latest_output_tps: Option<f64>,
    #[serde(skip)]
    tps_24h: TpsAccumulator,
    #[serde(skip)]
    tps_7d: TpsAccumulator,
    #[serde(skip)]
    latest_tps_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct TpsAccumulator {
    output_tokens: u64,
    duration_ms: i64,
}

impl TpsAccumulator {
    fn add(&mut self, output_tokens: u64, duration_ms: i64) {
        self.output_tokens = self.output_tokens.saturating_add(output_tokens);
        self.duration_ms = self.duration_ms.saturating_add(duration_ms);
    }

    fn rate(&self) -> Option<f64> {
        (self.duration_ms >= TPS_MIN_TOTAL_DURATION_MS && self.output_tokens > 0)
            .then(|| round_tps(self.output_tokens as f64 * 1000.0 / self.duration_ms as f64))
    }
}

impl CodexAccountTokenUsage {
    pub(super) fn new(account_id: String) -> Self {
        Self {
            account_id,
            ..Self::default()
        }
    }

    pub(super) fn observe(
        &mut self,
        event: &ConfirmedTokenDelta,
        last_24h_start: i64,
        last_7d_start: i64,
        last_30d_start: i64,
    ) {
        if event.timestamp < last_30d_start {
            return;
        }
        self.last_30d.add(&event.total);
        self.last_active_at = max_option_i64(self.last_active_at, Some(event.timestamp));
        if event.timestamp >= last_7d_start {
            self.last_7d.add(&event.total);
        }
        if event.timestamp >= last_24h_start {
            self.last_24h.add(&event.total);
            self.response_count_24h += 1;
        }
        let Some(duration_ms) = event.duration_ms else {
            return;
        };
        let output_tokens = event.total.output_tokens;
        if output_tokens == 0 {
            return;
        }
        if event.timestamp >= last_7d_start {
            self.tps_7d.add(output_tokens, duration_ms);
        }
        if event.timestamp >= last_24h_start {
            self.tps_24h.add(output_tokens, duration_ms);
        }
        if self
            .latest_tps_at
            .map(|at| event.timestamp >= at)
            .unwrap_or(true)
        {
            self.latest_tps_at = Some(event.timestamp);
            self.latest_output_tps = Some(round_tps(
                output_tokens as f64 * 1000.0 / duration_ms as f64,
            ));
        }
    }
}

fn round_tps(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

pub(super) fn finish_accounts(
    accounts: HashMap<String, CodexAccountTokenUsage>,
) -> Vec<CodexAccountTokenUsage> {
    let mut accounts = accounts
        .into_values()
        .filter(|account| account.last_30d.total_tokens > 0)
        .map(|mut account| {
            account.output_tps_24h = account.tps_24h.rate();
            account.output_tps_7d = account.tps_7d.rate();
            account
        })
        .collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.account_id.cmp(&right.account_id));
    accounts
}

pub(super) fn response_duration_ms(boundary: Option<i64>, timestamp: i64) -> Option<i64> {
    boundary
        .and_then(|start| timestamp.checked_sub(start))
        .filter(|duration| (TPS_MIN_RESPONSE_MS..=TPS_MAX_RESPONSE_MS).contains(duration))
}

/// A new user turn starts the clock for the next model response; idle time
/// before it must not count against throughput.
pub(super) fn is_response_boundary_line(line: &str) -> bool {
    [
        "\"type\":\"turn_context\"",
        "\"type\":\"user_message\"",
        "\"type\":\"task_started\"",
    ]
    .iter()
    .any(|pattern| line.contains(pattern))
}

pub(super) fn line_timestamp_ms(line: &str) -> Option<i64> {
    const KEY: &str = "\"timestamp\":\"";
    let start = line.find(KEY)? + KEY.len();
    let end = start + line[start..].find('"')?;
    let timestamp = OffsetDateTime::parse(&line[start..end], &Rfc3339).ok()?;
    i64::try_from(timestamp.unix_timestamp_nanos() / 1_000_000).ok()
}

#[cfg(test)]
mod tests {
    use super::super::{scan_codex_token_usage_roots, tests::unique_temp_dir};
    use super::*;
    use std::fs;

    fn cumulative_token_line(timestamp: &str, total: u64, output: u64) -> String {
        serde_json::json!({
            "timestamp": timestamp,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "total_token_usage": {
                        "input_tokens": total - output,
                        "output_tokens": output,
                        "total_tokens": total
                    }
                }
            }
        })
        .to_string()
    }

    #[test]
    fn attributes_usage_and_output_tps_to_the_creator_account() {
        let root = unique_temp_dir();
        let sessions = root.join("sessions").join("2026").join("04").join("28");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        let meta = serde_json::json!({
            "timestamp": "2026-04-28T05:00:00.000Z",
            "type": "session_meta",
            "payload": { "id": "tps-session", "creator_account_id": "acct-a" }
        })
        .to_string();
        let turn = |timestamp: &str| {
            serde_json::json!({
                "timestamp": timestamp,
                "type": "turn_context",
                "payload": { "turn_id": "t" }
            })
            .to_string()
        };
        fs::write(
            sessions.join("rollout-tps.jsonl"),
            [
                meta,
                turn("2026-04-28T06:00:00.000Z"),
                // 100 output tokens in 2s, then 300 more in 3s.
                cumulative_token_line("2026-04-28T06:00:02.000Z", 1_100, 100),
                cumulative_token_line("2026-04-28T06:00:05.000Z", 2_400, 400),
                // An hour of idle time before the next turn must not count.
                turn("2026-04-28T07:00:00.000Z"),
                cumulative_token_line("2026-04-28T07:00:04.000Z", 3_000, 600),
            ]
            .join("\n"),
        )
        .expect("write log");

        let snapshot = scan_codex_token_usage_roots(&[root.join("sessions")], 1_777_361_000);

        assert_eq!(snapshot.accounts.len(), 1);
        let account = &snapshot.accounts[0];
        assert_eq!(account.account_id, "acct-a");
        assert_eq!(account.last_24h.total_tokens, 3_000);
        assert_eq!(account.response_count_24h, 3);
        // 600 output tokens over 9 seconds of response time.
        assert_eq!(account.output_tps_24h, Some(66.7));
        assert_eq!(account.latest_output_tps, Some(50.0));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_missing_tiny_idle_and_reversed_durations() {
        assert_eq!(response_duration_ms(None, 1000), None);
        assert_eq!(response_duration_ms(Some(1000), 1199), None);
        assert_eq!(response_duration_ms(Some(1000), 1200), Some(200));
        assert_eq!(response_duration_ms(Some(1000), 601000), Some(600000));
        assert_eq!(response_duration_ms(Some(1000), 601001), None);
        assert_eq!(response_duration_ms(Some(1000), 999), None);
    }
}
