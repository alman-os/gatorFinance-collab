use chrono::NaiveDateTime;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppResult, message};
use crate::identity::{
    IDENTITY_VERSION, canonical_identifier, fingerprint, identity_fingerprint, institution_name,
    masked_identifier, payload_fingerprint, suggested_nickname,
};
use crate::models::{
    AccountProfile, AccountUpdate, ImportFileReceipt, ImportSummary, MigrationStatus,
    NewAccountInput, ParsedStatement, RawTransaction, SmartList, SmartListInput, TransactionRecord,
    TransactionType, TransactionUpdate,
};
use crate::ofx::{normalize, parse_ofx_date};

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
}

impl Database {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn connect(&self) -> AppResult<Connection> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(&self.path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.execute_batch(SCHEMA)?;
        migrate_schema(&mut connection)?;
        Ok(connection)
    }

    pub fn initialize(
        &self,
        legacy_path: &Path,
        migrate_legacy: bool,
    ) -> AppResult<MigrationStatus> {
        let connection = self.connect()?;
        let existing: i64 =
            connection.query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?;
        let already_migrated: Option<String> = connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'legacy_migrated'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let legacy_found = legacy_path.join("raw").is_dir();
        drop(connection);

        if existing > 0 || already_migrated.is_some() || !legacy_found || !migrate_legacy {
            return Ok(MigrationStatus {
                legacy_found,
                migration_performed: false,
                imported_transactions: 0,
                message: if legacy_found && existing > 0 {
                    "Current database already contains transactions; legacy data was left untouched.".to_string()
                } else if legacy_found && already_migrated.is_some() {
                    "Legacy migration was already checked.".to_string()
                } else if legacy_found {
                    "Legacy beta data was found and left untouched. Import current OFX files to start a clean ledger.".to_string()
                } else {
                    "No legacy database found.".to_string()
                },
            });
        }

        let mut imported = 0usize;
        let mut files = fs::read_dir(legacy_path.join("raw"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            let value: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
            if let Some(statement) = legacy_statement(&value) {
                imported += self.import_statement(&statement)?.new_transactions;
            }
        }

        let connection = self.connect()?;
        connection.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES('legacy_migrated', datetime('now'))",
            [],
        )?;
        self.apply_legacy_overrides(legacy_path)?;

        Ok(MigrationStatus {
            legacy_found: true,
            migration_performed: true,
            imported_transactions: imported,
            message: format!(
                "Migrated {imported} legacy transactions. Original files were preserved."
            ),
        })
    }

    fn apply_legacy_overrides(&self, legacy_path: &Path) -> AppResult<()> {
        let path = legacy_path.join("overrides.json");
        if !path.is_file() {
            return Ok(());
        }
        let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
        let Some(overrides) = value.as_object() else {
            return Ok(());
        };
        let mut connection = self.connect()?;
        let tx = connection.transaction()?;
        for (hash, item) in overrides {
            let tags = item
                .get("tags_user")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new()));
            tx.execute(
                "UPDATE transactions SET category_user=?1, vendor_user=?2, notes_user=?3, tags_json=?4, is_excluded=?5 WHERE dedupe_hash=?6",
                params![
                    item.get("category_user").and_then(Value::as_str),
                    item.get("vendor_user").and_then(Value::as_str),
                    item.get("notes_user").and_then(Value::as_str),
                    tags.to_string(),
                    item.get("is_excluded").and_then(Value::as_bool).unwrap_or(false) as i32,
                    hash,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn import_statement(&self, statement: &ParsedStatement) -> AppResult<ImportSummary> {
        let account = self.ensure_account_for_statement(statement, None)?;
        let receipt = self.import_statement_for_account(
            statement,
            &account.id,
            &uuid::Uuid::new_v4().to_string(),
        )?;
        Ok(ImportSummary {
            files_processed: 1,
            files_failed: 0,
            new_transactions: receipt.new_transactions,
            duplicate_transactions: receipt.duplicate_transactions,
            errors: if receipt.conflict_transactions > 0 {
                vec![format!(
                    "{} transaction revisions require review",
                    receipt.conflict_transactions
                )]
            } else {
                Vec::new()
            },
        })
    }

    pub fn list_accounts(&self, include_archived: bool) -> AppResult<Vec<AccountProfile>> {
        let connection = self.connect()?;
        let sql = if include_archived {
            "SELECT id, nickname, institution_name, display_account_type, source_account_type, currency, masked_identifier, created_at, updated_at, archived_at IS NOT NULL FROM account_profiles ORDER BY archived_at IS NOT NULL, nickname COLLATE NOCASE"
        } else {
            "SELECT id, nickname, institution_name, display_account_type, source_account_type, currency, masked_identifier, created_at, updated_at, 0 FROM account_profiles WHERE archived_at IS NULL ORDER BY nickname COLLATE NOCASE"
        };
        let mut statement = connection.prepare(sql)?;
        let rows = statement.query_map([], row_to_account)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn account_by_id(&self, id: &str) -> AppResult<Option<AccountProfile>> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, nickname, institution_name, display_account_type, source_account_type, currency, masked_identifier, created_at, updated_at, archived_at IS NOT NULL FROM account_profiles WHERE id=?1",
                [id],
                row_to_account,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn match_account(&self, statement: &ParsedStatement) -> AppResult<Option<AccountProfile>> {
        let Some(identity) = identity_fingerprint(statement) else {
            return Ok(None);
        };
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT p.id, p.nickname, p.institution_name, p.display_account_type, p.source_account_type, p.currency, p.masked_identifier, p.created_at, p.updated_at, p.archived_at IS NOT NULL FROM account_profiles p JOIN account_identities i ON i.account_profile_id=p.id WHERE i.identity_fingerprint=?1",
                [identity],
                row_to_account,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn ensure_account_for_statement(
        &self,
        statement: &ParsedStatement,
        input: Option<&NewAccountInput>,
    ) -> AppResult<AccountProfile> {
        if let Some(account) = self.match_account(statement)? {
            return Ok(account);
        }
        let nickname = input
            .map(|value| value.nickname.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| suggested_nickname(statement));
        let display_type = input
            .and_then(|value| clean_optional(value.display_account_type.as_deref()))
            .or_else(|| statement.account_type.clone());
        let id = uuid::Uuid::new_v4().to_string();
        let institution = institution_name(statement);
        let masked = masked_identifier(statement.account_id.as_deref());
        let identity = identity_fingerprint(statement);
        let mut connection = self.connect()?;
        let tx = connection.transaction()?;
        tx.execute(
            "INSERT INTO account_profiles(id, nickname, institution_name, display_account_type, source_account_type, currency, masked_identifier, created_at, updated_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'), datetime('now'))",
            params![id, nickname, institution, display_type, statement.account_type, statement.currency, masked],
        )?;
        if let Some(identity) = identity {
            tx.execute(
                "INSERT INTO account_identities(id, account_profile_id, identity_version, institution_key, account_key, identity_fingerprint, last_seen_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    id,
                    IDENTITY_VERSION,
                    canonical_identifier(statement.bank_id.as_deref().or(statement.financial_institution_id.as_deref()).or(statement.organization.as_deref()).unwrap_or_default()),
                    canonical_identifier(statement.account_id.as_deref().unwrap_or_default()),
                    identity,
                ],
            )?;
        }
        tx.commit()?;
        self.account_by_id(&id)?
            .ok_or_else(|| message("Account profile could not be created"))
    }

    pub fn update_account(&self, update: &AccountUpdate) -> AppResult<AccountProfile> {
        let nickname = update.nickname.trim();
        if nickname.is_empty() {
            return Err(message("Account nickname is required"));
        }
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE account_profiles SET nickname=?1, display_account_type=?2, archived_at=CASE WHEN ?3 THEN COALESCE(archived_at, datetime('now')) ELSE NULL END, updated_at=datetime('now') WHERE id=?4",
            params![nickname, clean_optional(update.display_account_type.as_deref()), update.archived, update.id],
        )?;
        if changed == 0 {
            return Err(message("Account profile no longer exists"));
        }
        self.account_by_id(&update.id)?
            .ok_or_else(|| message("Account profile no longer exists"))
    }

    pub fn observe_account_statement(
        &self,
        account_profile_id: &str,
        statement: &ParsedStatement,
    ) -> AppResult<()> {
        let connection = self.connect()?;
        connection.execute(
            "UPDATE account_profiles SET source_account_type=COALESCE(?1, source_account_type), updated_at=datetime('now') WHERE id=?2",
            params![statement.account_type, account_profile_id],
        )?;
        if let Some(identity) = identity_fingerprint(statement) {
            connection.execute(
                "UPDATE account_identities SET last_seen_at=datetime('now') WHERE account_profile_id=?1 AND identity_fingerprint=?2",
                params![account_profile_id, identity],
            )?;
        }
        Ok(())
    }

    pub fn import_statement_for_account(
        &self,
        statement: &ParsedStatement,
        account_profile_id: &str,
        batch_id: &str,
    ) -> AppResult<ImportFileReceipt> {
        let mut connection = self.connect()?;
        let tx = connection.transaction()?;
        let mut new_transactions = 0usize;
        let mut duplicate_transactions = 0usize;
        let mut conflict_transactions = 0usize;

        for raw in &statement.transactions {
            let payload = payload_fingerprint(
                &raw.posted_at,
                raw.amount_minor,
                raw.refnum.as_deref(),
                &raw.memo,
            );
            let existing: Option<(String, Option<String>)> = tx
                .query_row(
                    "SELECT dedupe_hash, payload_fingerprint FROM raw_transactions WHERE account_profile_id=?1 AND fitid=?2",
                    params![account_profile_id, raw.fitid],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((dedupe_hash, prior_payload)) = existing {
                if prior_payload
                    .as_deref()
                    .map_or(true, |value| value == payload)
                {
                    duplicate_transactions += 1;
                    tx.execute(
                        "UPDATE raw_transactions SET payload_fingerprint=COALESCE(payload_fingerprint, ?1) WHERE dedupe_hash=?2",
                        params![payload, dedupe_hash],
                    )?;
                } else {
                    conflict_transactions += 1;
                    tx.execute(
                        "INSERT INTO import_conflicts(id, import_batch_id, account_profile_id, provider_transaction_id, existing_dedupe_hash, incoming_payload_fingerprint, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
                        params![uuid::Uuid::new_v4().to_string(), batch_id, account_profile_id, raw.fitid, dedupe_hash, payload],
                    )?;
                }
                continue;
            }
            let stored_hash = tx
                .query_row(
                    "SELECT account_profile_id FROM raw_transactions WHERE dedupe_hash=?1",
                    [&raw.dedupe_hash],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .filter(|owner| owner.as_deref() != Some(account_profile_id))
                .and_then(|_| fingerprint(None, None, Some(account_profile_id), Some(&raw.fitid)))
                .unwrap_or_else(|| raw.dedupe_hash.clone());
            let inserted = insert_raw(&tx, raw, &stored_hash, account_profile_id, &payload)?;
            if inserted == 0 {
                duplicate_transactions += 1;
                continue;
            }
            let mut normalized = normalize(raw);
            normalized.dedupe_hash = stored_hash;
            insert_normalized(&tx, &normalized, account_profile_id)?;
            new_transactions += 1;
        }

        tx.execute(
            "INSERT INTO imports(source_file, imported_at, total_transactions, new_transactions, duplicate_transactions, bank_id, account_label, currency, period_start, period_end, account_profile_id, conflict_transactions, import_batch_id) VALUES(?1, datetime('now'), ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                statement.source_file,
                statement.transactions.len() as i64,
                new_transactions as i64,
                duplicate_transactions as i64,
                statement.bank_id,
                masked_identifier(statement.account_id.as_deref()),
                statement.currency,
                statement.period_start.map(format_datetime),
                statement.period_end.map(format_datetime),
                account_profile_id,
                conflict_transactions as i64,
                batch_id,
            ],
        )?;
        tx.commit()?;

        Ok(ImportFileReceipt {
            source_file: statement.source_file.clone(),
            account_profile_id: account_profile_id.to_string(),
            new_transactions,
            duplicate_transactions,
            conflict_transactions,
            total_transactions: statement.transactions.len(),
        })
    }

    pub fn list_transactions(&self) -> AppResult<Vec<TransactionRecord>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT dedupe_hash, unique_id, posted_at, date_raw, amount_minor, currency, transaction_type, category_auto, category_confidence, is_recurring, vendor_raw, vendor_clean, vendor_normalized, card_mask, category_user, vendor_user, notes_user, tags_json, is_excluded, source_file, account_label, account_profile_id FROM transactions ORDER BY posted_at DESC, dedupe_hash",
        )?;
        let rows = statement.query_map([], row_to_transaction)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_transaction(&self, update: &TransactionUpdate) -> AppResult<()> {
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE transactions SET category_user=?1, vendor_user=?2, notes_user=?3, tags_json=?4, is_excluded=?5, updated_at=datetime('now') WHERE dedupe_hash=?6",
            params![
                clean_optional(update.category.as_deref()),
                clean_optional(update.vendor.as_deref()),
                clean_optional(update.notes.as_deref()),
                serde_json::to_string(&update.tags)?,
                update.is_excluded as i32,
                update.dedupe_hash,
            ],
        )?;
        if changed == 0 {
            return Err(message("Transaction no longer exists"));
        }
        Ok(())
    }

    pub fn list_smart_lists(&self) -> AppResult<Vec<SmartList>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, name, rules_json, created_at, updated_at FROM smart_lists ORDER BY name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            let rules_json: String = row.get(2)?;
            let rules = serde_json::from_str(&rules_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(SmartList {
                id: row.get(0)?,
                name: row.get(1)?,
                rules,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn save_smart_list(&self, input: &SmartListInput) -> AppResult<SmartList> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(message("Smart list name is required"));
        }
        if name.chars().count() > 80 || input.rules.source_query.chars().count() > 200 {
            return Err(message("Smart list name or source query is too long"));
        }
        if !matches!(input.rules.direction.as_str(), "all" | "credit" | "debit") {
            return Err(message("Invalid smart list direction"));
        }
        if !matches!(
            input.rules.source_match.as_str(),
            "contains" | "exact" | "starts_with"
        ) {
            return Err(message("Invalid source matching mode"));
        }
        if let (Some(min), Some(max)) = (input.rules.min_amount_minor, input.rules.max_amount_minor)
        {
            if min > max {
                return Err(message("Minimum amount cannot exceed maximum amount"));
            }
        }
        if input.rules.min_amount_minor.is_some_and(|value| value < 0)
            || input.rules.max_amount_minor.is_some_and(|value| value < 0)
        {
            return Err(message("Smart list amount bounds cannot be negative"));
        }
        for value in [
            input.rules.date_start.as_deref(),
            input.rules.date_end.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map_err(|_| message("Smart list dates must use YYYY-MM-DD"))?;
        }
        let id = input
            .id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO smart_lists(id, name, rules_json, created_at, updated_at) VALUES(?1, ?2, ?3, datetime('now'), datetime('now')) ON CONFLICT(id) DO UPDATE SET name=excluded.name, rules_json=excluded.rules_json, updated_at=datetime('now')",
            params![id, name, serde_json::to_string(&input.rules)?],
        )?;
        self.list_smart_lists()?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| message("Smart list could not be saved"))
    }

    pub fn delete_smart_list(&self, id: &str) -> AppResult<()> {
        let connection = self.connect()?;
        let changed = connection.execute("DELETE FROM smart_lists WHERE id=?1", [id])?;
        if changed == 0 {
            return Err(message("Smart list no longer exists"));
        }
        Ok(())
    }

    pub fn start_import_batch(&self, id: &str) -> AppResult<()> {
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO import_batches(id, started_at, status) VALUES(?1, datetime('now'), 'running')",
            [id],
        )?;
        Ok(())
    }

    pub fn finish_import_batch(&self, id: &str, status: &str) -> AppResult<()> {
        let connection = self.connect()?;
        connection.execute(
            "UPDATE import_batches SET finished_at=datetime('now'), status=?1 WHERE id=?2",
            params![status, id],
        )?;
        Ok(())
    }
}

fn clean_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn format_datetime(value: NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn parse_datetime(value: &str) -> rusqlite::Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn insert_raw(
    tx: &Transaction<'_>,
    raw: &RawTransaction,
    stored_hash: &str,
    account_profile_id: &str,
    payload: &str,
) -> AppResult<usize> {
    Ok(tx.execute(
        "INSERT OR IGNORE INTO raw_transactions(dedupe_hash, unique_id, fitid, refnum, date_raw, posted_at, amount_raw, amount_minor, trntype, memo, source_file, account_id, bank_id, currency, imported_at, account_profile_id, payload_fingerprint) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, datetime('now'), ?15, ?16)",
        params![
            stored_hash,
            raw.unique_id,
            raw.fitid,
            raw.refnum,
            raw.date_raw,
            format_datetime(raw.posted_at),
            raw.amount_raw,
            raw.amount_minor,
            raw.trntype,
            raw.memo,
            raw.source_file,
            raw.account_id,
            raw.bank_id,
            raw.currency,
            account_profile_id,
            payload,
        ],
    )?)
}

fn insert_normalized(
    tx: &Transaction<'_>,
    item: &TransactionRecord,
    account_profile_id: &str,
) -> AppResult<()> {
    tx.execute(
        "INSERT INTO transactions(dedupe_hash, unique_id, posted_at, date_raw, amount_minor, currency, transaction_type, category_auto, category_confidence, is_recurring, vendor_raw, vendor_clean, vendor_normalized, card_mask, tags_json, is_excluded, source_file, account_label, updated_at, account_profile_id) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, '[]', 0, ?15, ?16, datetime('now'), ?17)",
        params![
            item.dedupe_hash,
            item.unique_id,
            format_datetime(item.date),
            item.date_raw,
            item.amount_minor,
            item.currency,
            match item.transaction_type { TransactionType::Credit => "credit", TransactionType::Debit => "debit" },
            item.category_auto,
            item.category_confidence,
            item.is_recurring.map(i32::from),
            item.vendor_raw,
            item.vendor_clean,
            item.vendor_normalized,
            item.card_mask,
            item.source_file,
            item.account_label,
            account_profile_id,
        ],
    )?;
    Ok(())
}

fn row_to_transaction(row: &rusqlite::Row<'_>) -> rusqlite::Result<TransactionRecord> {
    let transaction_type: String = row.get(6)?;
    let tags_json: String = row.get(17)?;
    Ok(TransactionRecord {
        dedupe_hash: row.get(0)?,
        unique_id: row.get(1)?,
        date: parse_datetime(&row.get::<_, String>(2)?)?,
        date_raw: row.get(3)?,
        amount_minor: row.get(4)?,
        currency: row.get(5)?,
        transaction_type: if transaction_type == "credit" {
            TransactionType::Credit
        } else {
            TransactionType::Debit
        },
        category_auto: row.get(7)?,
        category_confidence: row.get(8)?,
        is_recurring: row.get::<_, Option<i32>>(9)?.map(|value| value != 0),
        vendor_raw: row.get(10)?,
        vendor_clean: row.get(11)?,
        vendor_normalized: row.get(12)?,
        card_mask: row.get(13)?,
        category_user: row.get(14)?,
        vendor_user: row.get(15)?,
        notes_user: row.get(16)?,
        tags_user: serde_json::from_str(&tags_json).unwrap_or_default(),
        is_excluded: row.get::<_, i32>(18)? != 0,
        source_file: row.get(19)?,
        account_label: row.get(20)?,
        account_profile_id: row.get(21)?,
    })
}

fn row_to_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<AccountProfile> {
    Ok(AccountProfile {
        id: row.get(0)?,
        nickname: row.get(1)?,
        institution_name: row.get(2)?,
        display_account_type: row.get(3)?,
        source_account_type: row.get(4)?,
        currency: row.get(5)?,
        masked_identifier: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        archived: row.get::<_, i32>(9)? != 0,
    })
}

fn legacy_statement(value: &Value) -> Option<ParsedStatement> {
    let source_file = value.get("source_file")?.as_str()?.to_string();
    let bank_id = value
        .get("bank_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let account_id = value
        .get("account_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let account_type = value
        .get("account_type")
        .and_then(Value::as_str)
        .map(str::to_string);
    let currency = value
        .get("currency")
        .and_then(Value::as_str)
        .unwrap_or("USD")
        .to_string();
    let mut transactions = Vec::new();
    for item in value.get("transactions")?.as_array()? {
        let fitid = item.get("fitid")?.as_str()?.to_string();
        let date_raw = item.get("dtposted")?.as_str()?.to_string();
        let posted_at = parse_ofx_date(&date_raw)?;
        let amount = item.get("trnamt")?.as_f64()?;
        let amount_raw = if amount.fract() == 0.0 {
            format!("{amount:.1}")
        } else {
            amount.to_string()
        };
        transactions.push(RawTransaction {
            dedupe_hash: item.get("dedupe_hash")?.as_str()?.to_string(),
            unique_id: item.get("unique_id")?.as_str()?.to_string(),
            fitid,
            refnum: item
                .get("refnum")
                .and_then(Value::as_str)
                .map(str::to_string),
            date_raw,
            posted_at,
            amount_raw,
            amount_minor: (amount * 100.0).round() as i64,
            trntype: item
                .get("trntype")
                .and_then(Value::as_str)
                .unwrap_or("OTHER")
                .to_string(),
            memo: item
                .get("memo")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            source_file: source_file.clone(),
            account_id: account_id.clone(),
            bank_id: bank_id.clone(),
            currency: currency.clone(),
        });
    }
    Some(ParsedStatement {
        source_file,
        organization: None,
        financial_institution_id: None,
        bank_id,
        account_id,
        account_type,
        currency,
        period_start: value
            .get("period_start")
            .and_then(Value::as_str)
            .and_then(|date| date.get(..19))
            .and_then(|date| NaiveDateTime::parse_from_str(date, "%Y-%m-%dT%H:%M:%S").ok()),
        period_end: value
            .get("period_end")
            .and_then(Value::as_str)
            .and_then(|date| date.get(..19))
            .and_then(|date| NaiveDateTime::parse_from_str(date, "%Y-%m-%dT%H:%M:%S").ok()),
        ledger_balance_minor: value
            .get("ledger_balance")
            .and_then(Value::as_f64)
            .map(|amount| (amount * 100.0).round() as i64),
        transactions,
    })
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> AppResult<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for candidate in columns {
        if candidate? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn migrate_schema(connection: &mut Connection) -> AppResult<()> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version >= 2 {
        return Ok(());
    }

    connection.execute_batch(PROFILE_SCHEMA)?;
    for (table, column, definition) in [
        ("imports", "account_profile_id", "TEXT"),
        (
            "imports",
            "conflict_transactions",
            "INTEGER NOT NULL DEFAULT 0",
        ),
        ("imports", "import_batch_id", "TEXT"),
        ("raw_transactions", "account_profile_id", "TEXT"),
        ("raw_transactions", "payload_fingerprint", "TEXT"),
        ("transactions", "account_profile_id", "TEXT"),
    ] {
        if !column_exists(connection, table, column)? {
            connection.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN {column} {definition};"
            ))?;
        }
    }

    backfill_account_profiles(connection)?;
    connection.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_raw_profile_fitid ON raw_transactions(account_profile_id, fitid);
         CREATE INDEX IF NOT EXISTS idx_transactions_account ON transactions(account_profile_id, posted_at DESC);
         CREATE INDEX IF NOT EXISTS idx_imports_account ON imports(account_profile_id, imported_at DESC);
         PRAGMA user_version=2;",
    )?;
    Ok(())
}

fn backfill_account_profiles(connection: &mut Connection) -> AppResult<()> {
    let identities = {
        let mut statement = connection.prepare(
            "SELECT DISTINCT bank_id, account_id, currency FROM raw_transactions WHERE account_profile_id IS NULL",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let tx = connection.transaction()?;
    for (bank_id, account_id, currency) in identities {
        let identity = fingerprint(bank_id.as_deref(), None, None, account_id.as_deref());
        let existing_id = if let Some(value) = identity.as_deref() {
            tx.query_row(
                "SELECT account_profile_id FROM account_identities WHERE identity_fingerprint=?1",
                [value],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        } else {
            None
        };
        let profile_id = existing_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let masked = masked_identifier(account_id.as_deref());
        let fallback_name = masked
            .as_deref()
            .map(|value| format!("Imported account {value}"))
            .unwrap_or_else(|| format!("Unassigned {currency} imports"));
        tx.execute(
            "INSERT OR IGNORE INTO account_profiles(id, nickname, institution_name, currency, masked_identifier, created_at, updated_at) VALUES(?1, ?2, ?3, ?4, ?5, datetime('now'), datetime('now'))",
            params![profile_id, fallback_name, bank_id, currency, masked],
        )?;
        if let Some(identity) = identity {
            tx.execute(
                "INSERT OR IGNORE INTO account_identities(id, account_profile_id, identity_version, institution_key, account_key, identity_fingerprint, last_seen_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
                params![uuid::Uuid::new_v4().to_string(), profile_id, IDENTITY_VERSION, canonical_identifier(bank_id.as_deref().unwrap_or_default()), canonical_identifier(account_id.as_deref().unwrap_or_default()), identity],
            )?;
        }
        tx.execute(
            "UPDATE raw_transactions SET account_profile_id=?1 WHERE account_profile_id IS NULL AND bank_id IS ?2 AND account_id IS ?3 AND currency=?4",
            params![profile_id, bank_id, account_id, currency],
        )?;
        tx.execute(
            "UPDATE transactions SET account_profile_id=?1 WHERE account_profile_id IS NULL AND dedupe_hash IN (SELECT dedupe_hash FROM raw_transactions WHERE account_profile_id=?1)",
            [profile_id],
        )?;
    }

    let rows = {
        let mut statement = tx.prepare(
            "SELECT dedupe_hash, posted_at, amount_minor, refnum, memo FROM raw_transactions WHERE payload_fingerprint IS NULL",
        )?;
        let values = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        values.collect::<Result<Vec<_>, _>>()?
    };
    for (dedupe_hash, posted_at, amount_minor, refnum, memo) in rows {
        if let Ok(date) = NaiveDateTime::parse_from_str(&posted_at, "%Y-%m-%dT%H:%M:%S") {
            tx.execute(
                "UPDATE raw_transactions SET payload_fingerprint=?1 WHERE dedupe_hash=?2",
                params![
                    payload_fingerprint(&date, amount_minor, refnum.as_deref(), &memo),
                    dedupe_hash
                ],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS imports (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_file TEXT NOT NULL,
  imported_at TEXT NOT NULL,
  total_transactions INTEGER NOT NULL,
  new_transactions INTEGER NOT NULL,
  duplicate_transactions INTEGER NOT NULL,
  bank_id TEXT,
  account_label TEXT,
  currency TEXT NOT NULL,
  period_start TEXT,
  period_end TEXT
);
CREATE TABLE IF NOT EXISTS raw_transactions (
  dedupe_hash TEXT PRIMARY KEY,
  unique_id TEXT NOT NULL,
  fitid TEXT NOT NULL,
  refnum TEXT,
  date_raw TEXT NOT NULL,
  posted_at TEXT NOT NULL,
  amount_raw TEXT NOT NULL,
  amount_minor INTEGER NOT NULL,
  trntype TEXT NOT NULL,
  memo TEXT NOT NULL,
  source_file TEXT NOT NULL,
  account_id TEXT,
  bank_id TEXT,
  currency TEXT NOT NULL,
  imported_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS transactions (
  dedupe_hash TEXT PRIMARY KEY REFERENCES raw_transactions(dedupe_hash),
  unique_id TEXT NOT NULL,
  posted_at TEXT NOT NULL,
  date_raw TEXT NOT NULL,
  amount_minor INTEGER NOT NULL,
  currency TEXT NOT NULL,
  transaction_type TEXT NOT NULL,
  category_auto TEXT NOT NULL,
  category_confidence REAL NOT NULL,
  is_recurring INTEGER,
  vendor_raw TEXT NOT NULL,
  vendor_clean TEXT NOT NULL,
  vendor_normalized TEXT NOT NULL,
  card_mask TEXT,
  category_user TEXT,
  vendor_user TEXT,
  notes_user TEXT,
  tags_json TEXT NOT NULL DEFAULT '[]',
  is_excluded INTEGER NOT NULL DEFAULT 0,
  source_file TEXT NOT NULL,
  account_label TEXT,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_transactions_posted_at ON transactions(posted_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category_auto, category_user);
"#;

const PROFILE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS account_profiles (
  id TEXT PRIMARY KEY,
  nickname TEXT NOT NULL,
  institution_name TEXT,
  display_account_type TEXT,
  source_account_type TEXT,
  currency TEXT NOT NULL,
  masked_identifier TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE IF NOT EXISTS account_identities (
  id TEXT PRIMARY KEY,
  account_profile_id TEXT NOT NULL REFERENCES account_profiles(id),
  identity_version INTEGER NOT NULL,
  institution_key TEXT NOT NULL,
  account_key TEXT NOT NULL,
  identity_fingerprint TEXT NOT NULL UNIQUE,
  last_seen_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS import_batches (
  id TEXT PRIMARY KEY,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  status TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS import_conflicts (
  id TEXT PRIMARY KEY,
  import_batch_id TEXT NOT NULL,
  account_profile_id TEXT NOT NULL REFERENCES account_profiles(id),
  provider_transaction_id TEXT NOT NULL,
  existing_dedupe_hash TEXT NOT NULL,
  incoming_payload_fingerprint TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS smart_lists (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  rules_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SmartListRules;
    use crate::ofx::parse_ofx_content;

    const FIXTURE: &str = include_str!("../../tests/fixtures/sample_statement.ofx");

    #[test]
    fn fresh_database_does_not_import_legacy_beta_data_by_default() {
        let root = std::env::temp_dir().join(format!(
            "gatorfinance-no-auto-migration-test-{}",
            uuid::Uuid::new_v4()
        ));
        let legacy = root.join("legacy");
        fs::create_dir_all(legacy.join("raw")).unwrap();
        let database = Database::new(root.join("test.sqlite3"));

        let status = database.initialize(&legacy, false).unwrap();

        assert!(status.legacy_found);
        assert!(!status.migration_performed);
        assert_eq!(status.imported_transactions, 0);
        assert!(database.list_transactions().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_is_transactional_and_deduplicated() {
        let root =
            std::env::temp_dir().join(format!("gatorfinance-db-test-{}", uuid::Uuid::new_v4()));
        let database = Database::new(root.join("test.sqlite3"));
        let statement = parse_ofx_content(FIXTURE, "sample_statement.ofx").unwrap();
        let first = database.import_statement(&statement).unwrap();
        let second = database.import_statement(&statement).unwrap();
        assert_eq!(first.new_transactions, 4);
        assert_eq!(second.duplicate_transactions, 4);
        assert_eq!(database.list_transactions().unwrap().len(), 4);
        let connection = database.connect().unwrap();
        let import_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM imports", [], |row| row.get(0))
            .unwrap();
        assert_eq!(import_count, 2, "duplicate-only imports remain auditable");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn revised_provider_transaction_is_held_as_a_conflict() {
        let root = std::env::temp_dir().join(format!(
            "gatorfinance-conflict-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = Database::new(root.join("test.sqlite3"));
        let statement = parse_ofx_content(FIXTURE, "sample_statement.ofx").unwrap();
        let account = database
            .ensure_account_for_statement(&statement, None)
            .unwrap();
        database
            .import_statement_for_account(&statement, &account.id, "first")
            .unwrap();
        let mut revised = statement.clone();
        revised.transactions[0].memo.push_str(" REVISED");
        let receipt = database
            .import_statement_for_account(&revised, &account.id, "second")
            .unwrap();
        assert_eq!(receipt.conflict_transactions, 1);
        assert_eq!(receipt.duplicate_transactions, 3);
        assert_eq!(receipt.new_transactions, 0);
        assert_eq!(database.list_transactions().unwrap().len(), 4);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn identical_provider_ids_can_belong_to_two_manual_accounts() {
        let root = std::env::temp_dir().join(format!(
            "gatorfinance-account-scope-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = Database::new(root.join("test.sqlite3"));
        let mut statement = parse_ofx_content(FIXTURE, "sample_statement.ofx").unwrap();
        statement.account_id = None;
        for transaction in &mut statement.transactions {
            transaction.account_id = None;
        }
        let first_account = database
            .ensure_account_for_statement(
                &statement,
                Some(&NewAccountInput {
                    nickname: "Manual one".to_string(),
                    display_account_type: None,
                }),
            )
            .unwrap();
        let second_account = database
            .ensure_account_for_statement(
                &statement,
                Some(&NewAccountInput {
                    nickname: "Manual two".to_string(),
                    display_account_type: None,
                }),
            )
            .unwrap();
        database
            .import_statement_for_account(&statement, &first_account.id, "first")
            .unwrap();
        let receipt = database
            .import_statement_for_account(&statement, &second_account.id, "second")
            .unwrap();
        assert_eq!(receipt.new_transactions, 4);
        assert_eq!(database.list_transactions().unwrap().len(), 8);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn smart_list_rules_round_trip_and_validate_amounts() {
        let root = std::env::temp_dir().join(format!(
            "gatorfinance-smart-list-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database = Database::new(root.join("test.sqlite3"));
        let rules = SmartListRules {
            source_query: "Amazon".to_string(),
            source_match: "contains".to_string(),
            direction: "debit".to_string(),
            account_profile_ids: Vec::new(),
            category: None,
            tag: None,
            date_start: Some("2026-01-01".to_string()),
            date_end: Some("2026-12-31".to_string()),
            min_amount_minor: Some(100),
            max_amount_minor: Some(50_000),
        };
        let saved = database
            .save_smart_list(&SmartListInput {
                id: None,
                name: "Amazon 2026".to_string(),
                rules: rules.clone(),
            })
            .unwrap();
        assert_eq!(saved.rules.source_query, "Amazon");
        assert_eq!(database.list_smart_lists().unwrap().len(), 1);

        let mut invalid = rules;
        invalid.min_amount_minor = Some(-1);
        assert!(
            database
                .save_smart_list(&SmartListInput {
                    id: None,
                    name: "Invalid".to_string(),
                    rules: invalid,
                })
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }
}
