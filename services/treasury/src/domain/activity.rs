//! Maps indexed account events into user-facing activity entries.

use crate::domain::repository::AccountEventRow;

/// A user-visible activity row before proto serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityView {
    pub kind: String,
    pub direction: String,
    pub counterparty: String,
    pub chain_id: u64,
    pub amount_wei: String,
    pub status: String,
    pub tx_hash: String,
    pub occurred_at: String,
}

/// Convert one indexed event into zero or one activity views for `user`.
pub fn map_event_to_activity(
    row: &AccountEventRow,
    user: &str,
    relay_status: Option<&str>,
) -> Option<ActivityView> {
    let user_lower = user.to_lowercase();
    let occurred_at = row
        .block_time_unix
        .map(|ts| format!("{ts}"))
        .unwrap_or_default();

    match row.event_kind.as_str() {
        "deposited"
            if row
                .address_to
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(&user_lower)) =>
        {
            Some(ActivityView {
                kind: "deposit".into(),
                direction: "incoming".into(),
                counterparty: String::new(),
                chain_id: row.chain_id as u64,
                amount_wei: row.amount_wei.clone(),
                status: "completed".into(),
                tx_hash: row.tx_hash.clone(),
                occurred_at,
            })
        }
        "withdrawn"
            if row
                .address_from
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(&user_lower)) =>
        {
            Some(ActivityView {
                kind: "withdrawal".into(),
                direction: "outgoing".into(),
                counterparty: String::new(),
                chain_id: row.chain_id as u64,
                amount_wei: row.amount_wei.clone(),
                status: "completed".into(),
                tx_hash: row.tx_hash.clone(),
                occurred_at,
            })
        }
        "transfer" => {
            let from = row.address_from.as_deref()?;
            let to = row.address_to.as_deref()?;
            let (direction, counterparty) = if from.eq_ignore_ascii_case(&user_lower) {
                ("outgoing", to.to_string())
            } else if to.eq_ignore_ascii_case(&user_lower) {
                ("incoming", from.to_string())
            } else {
                return None;
            };
            Some(ActivityView {
                kind: "transfer".into(),
                direction: direction.into(),
                counterparty,
                chain_id: row.chain_id as u64,
                amount_wei: row.amount_wei.clone(),
                status: "completed".into(),
                tx_hash: row.tx_hash.clone(),
                occurred_at,
            })
        }
        "hot_path_initiated"
            if row
                .address_from
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(&user_lower)) =>
        {
            let status = normalize_relay_status(relay_status);
            Some(ActivityView {
                kind: "transfer".into(),
                direction: "outgoing".into(),
                counterparty: row.address_to.clone().unwrap_or_default(),
                chain_id: row.chain_id as u64,
                amount_wei: row.amount_wei.clone(),
                status,
                tx_hash: row.tx_hash.clone(),
                occurred_at,
            })
        }
        "hot_path_released"
            if row
                .address_to
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(&user_lower)) =>
        {
            Some(ActivityView {
                kind: "transfer".into(),
                direction: "incoming".into(),
                counterparty: String::new(),
                chain_id: row.chain_id as u64,
                amount_wei: row.amount_wei.clone(),
                status: "completed".into(),
                tx_hash: row.tx_hash.clone(),
                occurred_at,
            })
        }
        _ => None,
    }
}

fn normalize_relay_status(relay_status: Option<&str>) -> String {
    match relay_status {
        Some("completed") => "completed".into(),
        Some("failed") | Some("rejected_inactive_chain") | Some("rejected_insufficient_depth") => {
            "failed".into()
        }
        Some(other) if other.starts_with("rejected") => "failed".into(),
        Some("pending") | None => "pending".into(),
        Some(other) => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: &str, from: Option<&str>, to: Option<&str>, amount: &str) -> AccountEventRow {
        AccountEventRow {
            chain_id: 84532,
            tx_hash: "0xtx".into(),
            log_index: 0,
            event_kind: kind.into(),
            address_from: from.map(str::to_string),
            address_to: to.map(str::to_string),
            amount_wei: amount.into(),
            block_number: 100,
            block_time_unix: Some(1_700_000_000),
            correlation: None,
        }
    }

    #[test]
    fn maps_deposit_for_recipient() {
        let view = map_event_to_activity(
            &row("deposited", None, Some("0xBob"), "5000"),
            "0xBob",
            None,
        )
        .unwrap();
        assert_eq!(view.kind, "deposit");
        assert_eq!(view.direction, "incoming");
    }

    #[test]
    fn maps_same_chain_transfer_both_ways() {
        let outgoing = map_event_to_activity(
            &row("transfer", Some("0xBob"), Some("0xAlice"), "500"),
            "0xBob",
            None,
        )
        .unwrap();
        assert_eq!(outgoing.direction, "outgoing");
        assert_eq!(outgoing.counterparty, "0xAlice");

        let incoming = map_event_to_activity(
            &row("transfer", Some("0xBob"), Some("0xAlice"), "500"),
            "0xAlice",
            None,
        )
        .unwrap();
        assert_eq!(incoming.direction, "incoming");
    }

    #[test]
    fn hot_path_sender_shows_pending_without_relay() {
        let view = map_event_to_activity(
            &row(
                "hot_path_initiated",
                Some("0xBob"),
                Some("0xCharlie"),
                "1000",
            ),
            "0xBob",
            None,
        )
        .unwrap();
        assert_eq!(view.status, "pending");
        assert_eq!(view.direction, "outgoing");
    }

    #[test]
    fn hot_path_recipient_shows_completed() {
        let view = map_event_to_activity(
            &row("hot_path_released", None, Some("0xCharlie"), "1000"),
            "0xCharlie",
            None,
        )
        .unwrap();
        assert_eq!(view.status, "completed");
        assert_eq!(view.direction, "incoming");
    }

    #[test]
    fn maps_withdrawal_for_sender() {
        let view = map_event_to_activity(
            &row("withdrawn", Some("0xBob"), None, "2000"),
            "0xBob",
            None,
        )
        .unwrap();
        assert_eq!(view.kind, "withdrawal");
        assert_eq!(view.direction, "outgoing");
    }

    #[test]
    fn excludes_unrelated_event_kinds() {
        assert!(map_event_to_activity(
            &row(
                "hot_path_initiated",
                Some("0xBob"),
                Some("0xCharlie"),
                "1000"
            ),
            "0xCharlie",
            None,
        )
        .is_none());
    }
}
