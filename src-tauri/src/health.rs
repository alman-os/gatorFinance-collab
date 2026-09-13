use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;

use crate::models::{
    ENGINE_VERSION, HealthComponents, HealthResult, MonthlySnapshot, TransactionRecord,
    TransactionType,
};

#[derive(Clone, Copy)]
struct Config {
    w_net_flow: f64,
    w_stability: f64,
    w_trend: f64,
    w_savings: f64,
    w_volatility: f64,
    w_overcommitment: f64,
    volatility_threshold: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            w_net_flow: 0.30,
            w_stability: 0.20,
            w_trend: 0.20,
            w_savings: 0.10,
            w_volatility: 0.10,
            w_overcommitment: 0.10,
            volatility_threshold: 0.50,
        }
    }
}

fn sigmoid(value: f64, steepness: f64) -> f64 {
    1.0 / (1.0 + (-steepness * value).exp())
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn sample_stdev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let average = mean(values);
    let variance = values
        .iter()
        .map(|value| (value - average).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    variance.sqrt()
}

fn trend_slope(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let x_mean = (values.len() - 1) as f64 / 2.0;
    let y_mean = mean(values);
    let numerator = values
        .iter()
        .enumerate()
        .map(|(index, value)| (index as f64 - x_mean) * (value - y_mean))
        .sum::<f64>();
    let denominator = (0..values.len())
        .map(|index| (index as f64 - x_mean).powi(2))
        .sum::<f64>();
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

fn grade(score: f64) -> String {
    match score {
        value if value >= 90.0 => "A",
        value if value >= 80.0 => "B",
        value if value >= 70.0 => "C",
        value if value >= 60.0 => "D",
        _ => "F",
    }
    .to_string()
}

fn next_month((year, month): (i32, u32)) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

fn snapshots(transactions: &[TransactionRecord], corrected: bool) -> Vec<MonthlySnapshot> {
    let mut grouped: BTreeMap<(i32, u32), MonthlySnapshot> = BTreeMap::new();
    for transaction in transactions {
        if transaction.is_excluded || (corrected && transaction.is_transfer()) {
            continue;
        }
        let key = (transaction.date.year(), transaction.date.month());
        let snapshot = grouped.entry(key).or_insert_with(|| MonthlySnapshot {
            year: key.0,
            month: key.1,
            total_credits_minor: 0,
            total_debits_minor: 0,
            net_flow_minor: 0,
            recurring_credits_minor: 0,
            recurring_debits_minor: 0,
            transaction_count: 0,
            commitment_ratio: 0.0,
            category_totals_minor: BTreeMap::new(),
        });
        snapshot.transaction_count += 1;
        match transaction.transaction_type {
            TransactionType::Credit => {
                snapshot.total_credits_minor += transaction.amount_minor;
                if transaction.is_recurring == Some(true) {
                    snapshot.recurring_credits_minor += transaction.amount_minor;
                }
            }
            TransactionType::Debit => {
                snapshot.total_debits_minor += transaction.amount_minor;
                if transaction.is_recurring == Some(true) {
                    snapshot.recurring_debits_minor += transaction.amount_minor;
                }
            }
        }
        *snapshot
            .category_totals_minor
            .entry(transaction.category_effective().to_string())
            .or_insert(0) += transaction.amount_minor;
    }

    if corrected {
        if let (Some(first), Some(last)) = (
            grouped.keys().next().copied(),
            grouped.keys().next_back().copied(),
        ) {
            let mut cursor = first;
            while cursor <= last {
                grouped.entry(cursor).or_insert_with(|| MonthlySnapshot {
                    year: cursor.0,
                    month: cursor.1,
                    total_credits_minor: 0,
                    total_debits_minor: 0,
                    net_flow_minor: 0,
                    recurring_credits_minor: 0,
                    recurring_debits_minor: 0,
                    transaction_count: 0,
                    commitment_ratio: 0.0,
                    category_totals_minor: BTreeMap::new(),
                });
                cursor = next_month(cursor);
            }
        }
    }

    grouped
        .into_values()
        .map(|mut snapshot| {
            snapshot.net_flow_minor = snapshot.total_credits_minor - snapshot.total_debits_minor;
            snapshot.commitment_ratio = if snapshot.total_credits_minor > 0 {
                snapshot.recurring_debits_minor as f64 / snapshot.total_credits_minor as f64
            } else {
                1.0
            };
            snapshot
        })
        .collect()
}

fn calculate(transactions: &[TransactionRecord], corrected: bool) -> HealthResult {
    let config = Config::default();
    let snapshots = snapshots(transactions, corrected);
    let engine_version = if corrected {
        ENGINE_VERSION
    } else {
        "0.3.0-parity"
    };

    if snapshots.is_empty() {
        return HealthResult {
            engine_version: engine_version.to_string(),
            score: 50.0,
            grade: "C".to_string(),
            components: HealthComponents {
                net_flow_score: 0.5,
                stability_score: 0.5,
                trend_score: 0.5,
                savings_score: 0.5,
                volatility_penalty: 0.0,
                overcommitment_penalty: 0.0,
            },
            avg_monthly_net_flow_minor: 0,
            avg_monthly_income_minor: 0,
            avg_monthly_expenses_minor: 0,
            trend_direction: "insufficient_data".to_string(),
            trend_slope_minor: 0.0,
            volatility: 0.0,
            months_analyzed: 0,
            period_start: None,
            period_end: None,
            monthly_snapshots: Vec::new(),
            best_month: None,
            worst_month: None,
        };
    }

    let incomes = snapshots
        .iter()
        .map(|item| item.total_credits_minor as f64)
        .collect::<Vec<_>>();
    let expenses = snapshots
        .iter()
        .map(|item| item.total_debits_minor as f64)
        .collect::<Vec<_>>();
    let net_flows = snapshots
        .iter()
        .map(|item| item.net_flow_minor as f64)
        .collect::<Vec<_>>();
    let stabilities = snapshots
        .iter()
        .map(|item| (item.recurring_credits_minor - item.recurring_debits_minor) as f64)
        .collect::<Vec<_>>();
    let commitments = snapshots
        .iter()
        .map(|item| item.commitment_ratio)
        .collect::<Vec<_>>();
    let avg_income = mean(&incomes);
    let avg_expenses = mean(&expenses);
    let avg_net_flow = mean(&net_flows);
    let avg_stability = mean(&stabilities);

    let net_flow_score = sigmoid(
        if avg_income > 0.0 {
            avg_net_flow / avg_income
        } else {
            0.0
        },
        5.0,
    );
    let stability_score = sigmoid(
        if avg_income > 0.0 {
            avg_stability / avg_income
        } else {
            0.0
        },
        5.0,
    );
    let slope = trend_slope(&net_flows);
    let normalized_slope = if avg_income > 0.0 {
        slope / avg_income
    } else {
        0.0
    };
    let trend_score = if snapshots.len() >= 2 {
        sigmoid(normalized_slope, 10.0)
    } else {
        0.5
    };
    let trend_threshold = if corrected {
        avg_income.abs() * 0.01
    } else {
        0.01
    };
    let trend_direction = if snapshots.len() < 2 {
        "insufficient_data"
    } else if slope > trend_threshold {
        "improving"
    } else if slope < -trend_threshold {
        "declining"
    } else {
        "stable"
    };
    let savings_score = if avg_income > 0.0 && avg_net_flow > 0.0 {
        ((avg_net_flow / avg_income) * 2.0).min(1.0)
    } else {
        0.0
    };
    let volatility = if avg_expenses > 0.0 {
        sample_stdev(&expenses) / avg_expenses
    } else {
        0.0
    };
    let volatility_penalty = if volatility > config.volatility_threshold {
        ((volatility - config.volatility_threshold) * 2.0).min(1.0)
    } else {
        0.0
    };
    let average_commitment = mean(&commitments);
    let overcommitment_penalty = if average_commitment > 0.5 {
        ((average_commitment - 0.5) * 2.0).min(1.0)
    } else {
        0.0
    };
    let raw_score = config.w_net_flow * net_flow_score
        + config.w_stability * stability_score
        + config.w_trend * trend_score
        + config.w_savings * savings_score
        - config.w_volatility * volatility_penalty
        - config.w_overcommitment * overcommitment_penalty;
    let score = ((raw_score + 0.2) * 100.0).clamp(0.0, 100.0);

    let best_month = snapshots
        .iter()
        .max_by_key(|item| item.net_flow_minor)
        .map(|item| format!("{}-{:02}", item.year, item.month));
    let worst_month = snapshots
        .iter()
        .min_by_key(|item| item.net_flow_minor)
        .map(|item| format!("{}-{:02}", item.year, item.month));
    let first = snapshots.first().expect("non-empty snapshots");
    let last = snapshots.last().expect("non-empty snapshots");

    HealthResult {
        engine_version: engine_version.to_string(),
        score,
        grade: grade(score),
        components: HealthComponents {
            net_flow_score,
            stability_score,
            trend_score,
            savings_score,
            volatility_penalty,
            overcommitment_penalty,
        },
        avg_monthly_net_flow_minor: avg_net_flow.round() as i64,
        avg_monthly_income_minor: avg_income.round() as i64,
        avg_monthly_expenses_minor: avg_expenses.round() as i64,
        trend_direction: trend_direction.to_string(),
        trend_slope_minor: slope,
        volatility,
        months_analyzed: snapshots.len(),
        period_start: NaiveDate::from_ymd_opt(first.year, first.month, 1),
        period_end: NaiveDate::from_ymd_opt(last.year, last.month, 1),
        monthly_snapshots: snapshots,
        best_month,
        worst_month,
    }
}

pub fn calculate_health(transactions: &[TransactionRecord]) -> HealthResult {
    calculate(transactions, true)
}

#[cfg(test)]
pub fn calculate_health_v03(transactions: &[TransactionRecord]) -> HealthResult {
    calculate(transactions, false)
}

#[cfg(test)]
mod tests {
    use crate::ofx::{normalize, parse_ofx_content};

    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/sample_statement.ofx");

    #[test]
    fn v03_score_matches_python_golden_value() {
        let statement = parse_ofx_content(FIXTURE, "sample_statement.ofx").unwrap();
        let transactions = statement
            .transactions
            .iter()
            .map(normalize)
            .collect::<Vec<_>>();
        let result = calculate_health_v03(&transactions);
        assert!((result.score - 79.5).abs() < 0.1);
        assert_eq!(result.grade, "C");
        assert_eq!(result.months_analyzed, 1);
    }

    #[test]
    fn corrected_engine_excludes_internal_transfers() {
        let statement = parse_ofx_content(FIXTURE, "sample_statement.ofx").unwrap();
        let transactions = statement
            .transactions
            .iter()
            .map(normalize)
            .collect::<Vec<_>>();
        let result = calculate_health(&transactions);
        assert_eq!(result.avg_monthly_income_minor, 0);
        assert!(result.avg_monthly_expenses_minor > 0);
    }
}
