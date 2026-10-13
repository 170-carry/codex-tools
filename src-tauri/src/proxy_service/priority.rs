//! Stable account-group priority, independent of sequential session affinity.
use super::{ProxyCandidate, StoredAccount};
use std::collections::HashMap;

pub(super) fn account_priorities(
    accounts: &mut [StoredAccount],
    order: &[String],
) -> HashMap<String, usize> {
    accounts.sort_by(|left, right| {
        left.added_at
            .cmp(&right.added_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    // 构建一次索引，避免每个请求按账号数量重复线性查找优先级。
    let mut priorities = HashMap::new();
    for key in order {
        let next = priorities.len();
        priorities.entry(key.clone()).or_insert(next);
    }
    for account in accounts.iter() {
        let next = priorities.len();
        priorities.entry(account.account_key()).or_insert(next);
    }
    priorities
}

pub(super) fn sort_candidates(candidates: &mut [ProxyCandidate], now: i64) {
    // 优先级模式每次重新按用户排序选择；已恢复的高优先级账号可立即接回后续请求。
    // 与逐个模式分开，不能被上一个会话命中的低优先级账号永久占用。
    candidates.sort_by_key(|candidate| {
        (
            candidate.auth_refresh_blocked || priority_quota_exhausted(candidate, now),
            candidate.priority,
            candidate.account_key.clone(),
        )
    });
}

fn priority_quota_exhausted(candidate: &ProxyCandidate, now: i64) -> bool {
    candidate.usage.as_ref().is_some_and(|usage| {
        [usage.five_hour.as_ref(), usage.one_week.as_ref()]
            .into_iter()
            .flatten()
            .any(|window| {
                window.used_percent.is_finite()
                    && window.used_percent >= 100.0
                    && window.reset_at.map(|reset| reset > now).unwrap_or(true)
            })
    })
}
