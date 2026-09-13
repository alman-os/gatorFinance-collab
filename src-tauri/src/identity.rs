use sha2::{Digest, Sha256};

use crate::models::ParsedStatement;

pub const IDENTITY_VERSION: u32 = 1;

pub fn canonical_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

pub fn institution_name(statement: &ParsedStatement) -> String {
    match statement.organization.as_deref().map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("BG") => "Banco General".to_string(),
        Some(value) if !value.is_empty() => value.to_string(),
        _ => "Bank account".to_string(),
    }
}

pub fn identity_fingerprint(statement: &ParsedStatement) -> Option<String> {
    fingerprint(
        statement.bank_id.as_deref(),
        statement.financial_institution_id.as_deref(),
        statement.organization.as_deref(),
        statement.account_id.as_deref(),
    )
}

pub fn fingerprint(
    bank_id: Option<&str>,
    fid: Option<&str>,
    organization: Option<&str>,
    account_id: Option<&str>,
) -> Option<String> {
    let account = canonical_identifier(account_id?);
    if account.is_empty() {
        return None;
    }
    let institution = bank_id
        .or(fid)
        .or(organization)
        .map(canonical_identifier)
        .filter(|value| !value.is_empty())?;
    let input = format!("v{IDENTITY_VERSION}|{institution}|{account}");
    Some(format!("{:x}", Sha256::digest(input.as_bytes())))
}

pub fn masked_identifier(account_id: Option<&str>) -> Option<String> {
    let normalized = canonical_identifier(account_id?);
    if normalized.is_empty() {
        return None;
    }
    let suffix = normalized.chars().rev().take(4).collect::<String>();
    Some(format!("•••• {}", suffix.chars().rev().collect::<String>()))
}

pub fn suggested_nickname(statement: &ParsedStatement) -> String {
    let institution = institution_name(statement);
    let account_type = statement
        .account_type
        .as_deref()
        .map(|value| match value.to_ascii_uppercase().as_str() {
            "CHECKING" => "Checking",
            "SAVINGS" => "Savings",
            "MONEYMRKT" => "Money market",
            "CREDITLINE" => "Credit line",
            _ => "Account",
        })
        .unwrap_or("Account");
    match masked_identifier(statement.account_id.as_deref()) {
        Some(masked) => format!("{institution} {account_type} {masked}"),
        None => format!("{institution} {account_type}"),
    }
}

pub fn payload_fingerprint(
    posted_at: &chrono::NaiveDateTime,
    amount_minor: i64,
    refnum: Option<&str>,
    memo: &str,
) -> String {
    let memo = memo.split_whitespace().collect::<Vec<_>>().join(" ");
    let input = format!(
        "{}|{}|{}|{}",
        posted_at.format("%Y-%m-%dT%H:%M:%S"),
        amount_minor,
        refnum.unwrap_or_default().trim(),
        memo.trim()
    );
    format!("{:x}", Sha256::digest(input.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_identity_ignores_formatting_but_preserves_leading_zeroes() {
        assert_eq!(canonical_identifier("00-123 45"), "0012345");
        assert_eq!(
            fingerprint(Some("04"), None, None, Some("00-123 45")),
            fingerprint(Some("04"), None, None, Some("0012345"))
        );
        assert_eq!(fingerprint(None, None, None, Some("0012345")), None);
    }
}
