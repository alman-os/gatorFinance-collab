use chrono::{NaiveDate, NaiveDateTime};
use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

use crate::categorizer::{categorize, clean_vendor, detect_recurring};
use crate::error::{AppResult, message};
use crate::models::{ParsedStatement, RawTransaction, TransactionRecord};

static TRANSACTION_BLOCK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<STMTTRN>(.*?)</STMTTRN>").expect("valid transaction regex"));
static STATEMENT_BLOCK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<STMTRS>(.*?)</STMTRS>").expect("valid statement regex"));

fn field_regex(field: &str) -> Regex {
    Regex::new(&format!(r"(?i)<{field}>([^<\r\n]+)")).expect("valid OFX field regex")
}

fn field(content: &str, name: &str) -> Option<String> {
    field_regex(name)
        .captures(content)
        .and_then(|captures| captures.get(1))
        .map(|value| decode_entities(value.as_str().trim()))
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

pub fn parse_ofx_date(value: &str) -> Option<NaiveDateTime> {
    let trimmed = value.trim();
    if trimmed.len() >= 14 {
        NaiveDateTime::parse_from_str(&trimmed[..14], "%Y%m%d%H%M%S").ok()
    } else if trimmed.len() >= 8 {
        NaiveDate::parse_from_str(&trimmed[..8], "%Y%m%d")
            .ok()
            .and_then(|date| date.and_hms_opt(0, 0, 0))
    } else {
        None
    }
}

fn amount_minor(value: &str) -> AppResult<(i64, f64)> {
    let parsed = value
        .trim()
        .parse::<f64>()
        .map_err(|_| message(format!("Invalid OFX amount: {value}")))?;
    Ok(((parsed * 100.0).round() as i64, parsed))
}

fn python_float_string(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn dedupe_hash(
    bank_id: Option<&str>,
    account_id: Option<&str>,
    fitid: &str,
    date_raw: &str,
    amount: f64,
) -> String {
    let combined = format!(
        "{}|{}|{}|{}|{}",
        bank_id.unwrap_or_default(),
        account_id.unwrap_or_default(),
        fitid,
        date_raw,
        python_float_string(amount)
    );
    let digest = Sha256::digest(combined.as_bytes());
    format!("{digest:x}")[..16].to_string()
}

fn account_label(account_id: Option<&str>) -> Option<String> {
    account_id.map(|value| {
        let tail = value.chars().rev().take(4).collect::<String>();
        format!("•••• {}", tail.chars().rev().collect::<String>())
    })
}

pub fn parse_ofx_statements(path: &Path) -> AppResult<Vec<ParsedStatement>> {
    let bytes = fs::read(path)?;
    let content = match String::from_utf8(bytes.clone()) {
        Ok(value) => value,
        Err(_) => bytes.into_iter().map(char::from).collect(),
    };
    let source_file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("statement.ofx");
    let organization = field(&content, "ORG");
    let financial_institution_id = field(&content, "FID");
    let blocks = STATEMENT_BLOCK
        .captures_iter(&content)
        .filter_map(|captures| captures.get(1).map(|value| value.as_str()))
        .collect::<Vec<_>>();
    if blocks.is_empty() {
        return Ok(vec![parse_ofx_content(&content, source_file)?]);
    }
    blocks
        .into_iter()
        .map(|block| {
            let mut statement = parse_ofx_content(block, source_file)?;
            statement.organization = organization.clone();
            statement.financial_institution_id = financial_institution_id.clone();
            Ok(statement)
        })
        .collect()
}

pub fn parse_ofx_content(content: &str, source_file: &str) -> AppResult<ParsedStatement> {
    let organization = field(content, "ORG");
    let financial_institution_id = field(content, "FID");
    let bank_id = field(content, "BANKID");
    let account_id = field(content, "ACCTID");
    let account_type = field(content, "ACCTTYPE");
    let currency = field(content, "CURDEF").unwrap_or_else(|| "USD".to_string());
    let mut period_start = field(content, "DTSTART").and_then(|value| parse_ofx_date(&value));
    let mut period_end = field(content, "DTEND").and_then(|value| parse_ofx_date(&value));
    let ledger_balance_minor = field(content, "BALAMT")
        .and_then(|value| amount_minor(&value).ok().map(|(minor, _)| minor));

    let mut transactions = Vec::new();
    for captures in TRANSACTION_BLOCK.captures_iter(content) {
        let block = captures
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default();
        let Some(fitid) = field(block, "FITID") else {
            continue;
        };
        let Some(date_raw) = field(block, "DTPOSTED") else {
            continue;
        };
        let Some(posted_at) = parse_ofx_date(&date_raw) else {
            continue;
        };
        let Some(amount_raw) = field(block, "TRNAMT") else {
            continue;
        };
        let Ok((minor, amount_value)) = amount_minor(&amount_raw) else {
            continue;
        };
        let memo = field(block, "MEMO").unwrap_or_default();
        let hash = dedupe_hash(
            bank_id.as_deref(),
            account_id.as_deref(),
            &fitid,
            &date_raw,
            amount_value,
        );
        let unique_id = bank_id
            .as_deref()
            .map(|bank| format!("{bank}-{fitid}"))
            .unwrap_or_else(|| fitid.clone());

        transactions.push(RawTransaction {
            dedupe_hash: hash,
            unique_id,
            fitid,
            refnum: field(block, "REFNUM"),
            date_raw,
            posted_at,
            amount_raw,
            amount_minor: minor,
            trntype: field(block, "TRNTYPE").unwrap_or_else(|| "OTHER".to_string()),
            memo,
            source_file: source_file.to_string(),
            account_id: account_id.clone(),
            bank_id: bank_id.clone(),
            currency: currency.clone(),
        });
    }

    if transactions.is_empty() {
        return Err(message(
            "No valid transactions were found in the OFX statement",
        ));
    }

    if period_start.is_none() || period_end.is_none() || period_start == period_end {
        period_start = transactions
            .iter()
            .map(|transaction| transaction.posted_at)
            .min();
        period_end = transactions
            .iter()
            .map(|transaction| transaction.posted_at)
            .max();
    }

    Ok(ParsedStatement {
        source_file: source_file.to_string(),
        organization,
        financial_institution_id,
        bank_id,
        account_id,
        account_type,
        currency,
        period_start,
        period_end,
        ledger_balance_minor,
        transactions,
    })
}

pub fn normalize(raw: &RawTransaction) -> TransactionRecord {
    let (transaction_type, category_auto, category_confidence) =
        categorize(&raw.memo, raw.amount_minor);
    let (vendor_clean, card_mask, vendor_normalized) = clean_vendor(&raw.memo);
    TransactionRecord {
        dedupe_hash: raw.dedupe_hash.clone(),
        unique_id: raw.unique_id.clone(),
        date: raw.posted_at,
        date_raw: raw.date_raw.clone(),
        amount_minor: raw.amount_minor.unsigned_abs() as i64,
        currency: raw.currency.clone(),
        transaction_type,
        category_auto,
        category_confidence,
        is_recurring: detect_recurring(&raw.memo),
        vendor_raw: raw.memo.clone(),
        vendor_clean,
        vendor_normalized,
        card_mask,
        category_user: None,
        vendor_user: None,
        notes_user: None,
        tags_user: Vec::new(),
        is_excluded: false,
        source_file: raw.source_file.clone(),
        account_label: account_label(raw.account_id.as_deref()),
        account_profile_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/sample_statement.ofx");

    #[test]
    fn matches_python_fixture_identity_and_period() {
        let statement = parse_ofx_content(FIXTURE, "sample_statement.ofx").unwrap();
        assert_eq!(statement.transactions.len(), 4);
        assert_eq!(statement.transactions[0].dedupe_hash, "153e56a462c82f94");
        assert_eq!(statement.transactions[1].dedupe_hash, "45cf8386147594f0");
        assert_eq!(
            statement
                .period_start
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            "2025-04-03T10:15:00"
        );
        assert_eq!(
            statement
                .period_end
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            "2025-04-20T17:12:00"
        );
    }

    #[test]
    fn normalization_matches_python_categories() {
        let statement = parse_ofx_content(FIXTURE, "sample_statement.ofx").unwrap();
        let categories = statement
            .transactions
            .iter()
            .map(normalize)
            .map(|transaction| transaction.category_auto)
            .collect::<Vec<_>>();
        assert_eq!(
            categories,
            ["food_dining", "subscription", "transfer_in", "groceries"]
        );
    }

    #[test]
    fn parses_each_statement_account_independently() {
        let block = STATEMENT_BLOCK
            .captures(FIXTURE)
            .and_then(|captures| captures.get(1).map(|value| value.as_str()))
            .unwrap();
        let second = block.replace("00-00-00-000000-0", "00-00-00-999999-9");
        let content = format!(
            "<OFX><ORG>GATOR<FID>001<STMTRS>{block}</STMTRS><STMTRS>{second}</STMTRS></OFX>"
        );
        let root = std::env::temp_dir().join(format!(
            "gatorfinance-multi-ofx-test-{}.ofx",
            uuid::Uuid::new_v4()
        ));
        fs::write(&root, content).unwrap();
        let statements = parse_ofx_statements(&root).unwrap();
        assert_eq!(statements.len(), 2);
        assert_ne!(statements[0].account_id, statements[1].account_id);
        assert_eq!(statements[0].transactions.len(), 4);
        assert_eq!(statements[1].transactions.len(), 4);
        assert_eq!(statements[0].organization.as_deref(), Some("GATOR"));
        let _ = fs::remove_file(root);
    }
}
