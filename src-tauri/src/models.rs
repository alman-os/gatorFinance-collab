use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ENGINE_VERSION: &str = "0.4.0-alpha.1";
pub const ARTIFACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Credit,
    Debit,
}

#[derive(Debug, Clone)]
pub struct RawTransaction {
    pub dedupe_hash: String,
    pub unique_id: String,
    pub fitid: String,
    pub refnum: Option<String>,
    pub date_raw: String,
    pub posted_at: NaiveDateTime,
    pub amount_raw: String,
    pub amount_minor: i64,
    pub trntype: String,
    pub memo: String,
    pub source_file: String,
    pub account_id: Option<String>,
    pub bank_id: Option<String>,
    pub currency: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ParsedStatement {
    pub source_file: String,
    pub organization: Option<String>,
    pub financial_institution_id: Option<String>,
    pub bank_id: Option<String>,
    pub account_id: Option<String>,
    pub account_type: Option<String>,
    pub currency: String,
    pub period_start: Option<NaiveDateTime>,
    pub period_end: Option<NaiveDateTime>,
    pub ledger_balance_minor: Option<i64>,
    pub transactions: Vec<RawTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRecord {
    pub dedupe_hash: String,
    pub unique_id: String,
    pub date: NaiveDateTime,
    pub date_raw: String,
    pub amount_minor: i64,
    pub currency: String,
    pub transaction_type: TransactionType,
    pub category_auto: String,
    pub category_confidence: f64,
    pub is_recurring: Option<bool>,
    pub vendor_raw: String,
    pub vendor_clean: String,
    pub vendor_normalized: String,
    pub card_mask: Option<String>,
    pub category_user: Option<String>,
    pub vendor_user: Option<String>,
    pub notes_user: Option<String>,
    pub tags_user: Vec<String>,
    pub is_excluded: bool,
    pub source_file: String,
    pub account_label: Option<String>,
    pub account_profile_id: Option<String>,
}

impl TransactionRecord {
    pub fn category_effective(&self) -> &str {
        self.category_user.as_deref().unwrap_or(&self.category_auto)
    }

    pub fn is_transfer(&self) -> bool {
        matches!(self.category_effective(), "transfer_in" | "transfer_out")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlySnapshot {
    pub year: i32,
    pub month: u32,
    pub total_credits_minor: i64,
    pub total_debits_minor: i64,
    pub net_flow_minor: i64,
    pub recurring_credits_minor: i64,
    pub recurring_debits_minor: i64,
    pub transaction_count: usize,
    pub commitment_ratio: f64,
    pub category_totals_minor: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthComponents {
    pub net_flow_score: f64,
    pub stability_score: f64,
    pub trend_score: f64,
    pub savings_score: f64,
    pub volatility_penalty: f64,
    pub overcommitment_penalty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResult {
    pub engine_version: String,
    pub score: f64,
    pub grade: String,
    pub components: HealthComponents,
    pub avg_monthly_net_flow_minor: i64,
    pub avg_monthly_income_minor: i64,
    pub avg_monthly_expenses_minor: i64,
    pub trend_direction: String,
    pub trend_slope_minor: f64,
    pub volatility: f64,
    pub months_analyzed: usize,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub monthly_snapshots: Vec<MonthlySnapshot>,
    pub best_month: Option<String>,
    pub worst_month: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTotal {
    pub category: String,
    pub amount_minor: i64,
    pub count: usize,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub database: String,
    pub library: String,
    pub legacy_database: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationStatus {
    pub legacy_found: bool,
    pub migration_performed: bool,
    pub imported_transactions: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub artifact_id: String,
    pub title: String,
    pub created_at: String,
    pub period_label: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub score: f64,
    pub grade: String,
    pub path: String,
    pub payload_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScan {
    pub items: Vec<LibraryItem>,
    pub skipped_invalid: usize,
    pub duplicate_payloads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub health: HealthResult,
    pub transactions: Vec<TransactionRecord>,
    pub smart_list_transactions: Vec<TransactionRecord>,
    pub categories: Vec<CategoryTotal>,
    pub transaction_count: usize,
    pub account_count: usize,
    pub currency: String,
    pub accounts: Vec<AccountProfile>,
    pub selected_account_ids: Vec<String>,
    pub smart_lists: Vec<SmartList>,
    pub library: LibraryScan,
    pub paths: AppPaths,
    pub migration: MigrationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub files_processed: usize,
    pub files_failed: usize,
    pub new_transactions: usize,
    pub duplicate_transactions: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub id: String,
    pub nickname: String,
    pub institution_name: Option<String>,
    pub display_account_type: Option<String>,
    pub source_account_type: Option<String>,
    pub currency: String,
    pub masked_identifier: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdate {
    pub id: String,
    pub nickname: String,
    pub display_account_type: Option<String>,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedStatement {
    pub detected_id: String,
    pub source_file: String,
    pub institution_name: Option<String>,
    pub masked_identifier: Option<String>,
    pub source_account_type: Option<String>,
    pub currency: String,
    pub period_start: Option<NaiveDateTime>,
    pub period_end: Option<NaiveDateTime>,
    pub transaction_count: usize,
    pub matched_account_id: Option<String>,
    pub identity_reliable: bool,
    pub suggested_nickname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlan {
    pub plan_id: String,
    pub statements: Vec<DetectedStatement>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccountInput {
    pub nickname: String,
    pub display_account_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAssignment {
    pub detected_id: String,
    pub account_profile_id: Option<String>,
    pub new_account: Option<NewAccountInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileReceipt {
    pub source_file: String,
    pub account_profile_id: String,
    pub new_transactions: usize,
    pub duplicate_transactions: usize,
    pub conflict_transactions: usize,
    pub total_transactions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReceipt {
    pub batch_id: String,
    pub files: Vec<ImportFileReceipt>,
    pub errors: Vec<String>,
    pub new_transactions: usize,
    pub duplicate_transactions: usize,
    pub conflict_transactions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartListRules {
    pub source_query: String,
    pub source_match: String,
    pub direction: String,
    pub account_profile_ids: Vec<String>,
    pub category: Option<String>,
    pub tag: Option<String>,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    pub min_amount_minor: Option<i64>,
    pub max_amount_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartList {
    pub id: String,
    pub name: String,
    pub rules: SmartListRules,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartListInput {
    pub id: Option<String>,
    pub name: String,
    pub rules: SmartListRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionUpdate {
    pub dedupe_hash: String,
    pub category: Option<String>,
    pub vendor: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub is_excluded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPrivacy {
    pub contains_account_identifiers: bool,
    pub contains_transaction_details: bool,
    pub safe_for_ai_share: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPayload {
    pub health: HealthResult,
    pub categories: Vec<CategoryTotal>,
    pub transaction_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatorArtifact {
    pub schema_version: u32,
    pub artifact_id: String,
    pub source_app: String,
    pub engine_version: String,
    pub title: String,
    pub created_at: String,
    pub payload_type: String,
    pub currency: String,
    pub period: String,
    pub privacy: ReportPrivacy,
    pub payload: ReportPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactActionResult {
    pub message: String,
    pub path: Option<String>,
    pub item: Option<LibraryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharePayload {
    pub url: String,
    pub text: String,
}
