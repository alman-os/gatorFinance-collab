mod categorizer;
mod database;
mod error;
mod health;
mod identity;
mod library;
mod models;
mod ofx;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{Manager, State};

use crate::database::Database;
use crate::error::AppResult;
use crate::health::calculate_health;
use crate::library::OutputLibrary;
use crate::models::{
    AccountProfile, AccountUpdate, AppPaths, ArtifactActionResult, CategoryTotal,
    DashboardSnapshot, DetectedStatement, ImportAssignment, ImportPlan, ImportReceipt, LibraryScan,
    MigrationStatus, ParsedStatement, SharePayload, SmartList, SmartListInput, TransactionType,
    TransactionUpdate,
};

#[derive(Clone)]
struct PendingStatement {
    detected_id: String,
    statement: ParsedStatement,
}

#[derive(Clone)]
struct PendingImportPlan {
    statements: Vec<PendingStatement>,
}

struct AppState {
    database: Database,
    library: OutputLibrary,
    paths: AppPaths,
    migration: Mutex<MigrationStatus>,
    pending_imports: Mutex<HashMap<String, PendingImportPlan>>,
}

fn category_totals(transactions: &[models::TransactionRecord]) -> Vec<CategoryTotal> {
    let mut totals: BTreeMap<String, (i64, usize)> = BTreeMap::new();
    for transaction in transactions {
        if transaction.is_excluded
            || transaction.is_transfer()
            || transaction.transaction_type != TransactionType::Debit
        {
            continue;
        }
        let entry = totals
            .entry(transaction.category_effective().to_string())
            .or_insert((0, 0));
        entry.0 += transaction.amount_minor;
        entry.1 += 1;
    }
    let grand_total = totals.values().map(|(amount, _)| *amount).sum::<i64>();
    let mut values = totals
        .into_iter()
        .map(|(category, (amount_minor, count))| CategoryTotal {
            category,
            amount_minor,
            count,
            percentage: if grand_total > 0 {
                amount_minor as f64 / grand_total as f64 * 100.0
            } else {
                0.0
            },
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.amount_minor.cmp(&left.amount_minor));
    values
}

fn scoped_transactions(
    state: &AppState,
    account_ids: Option<&[String]>,
    requested_currency: Option<&str>,
) -> AppResult<(
    Vec<models::TransactionRecord>,
    Vec<models::TransactionRecord>,
    Vec<AccountProfile>,
    Vec<String>,
    String,
)> {
    let all_accounts = state.database.list_accounts(true)?;
    let active_accounts = all_accounts
        .iter()
        .filter(|account| !account.archived)
        .collect::<Vec<_>>();
    let currency = account_ids
        .and_then(|ids| ids.first())
        .and_then(|id| active_accounts.iter().find(|account| account.id == *id))
        .map(|account| account.currency.clone())
        .or_else(|| requested_currency.map(str::to_string))
        .or_else(|| {
            active_accounts
                .first()
                .map(|account| account.currency.clone())
        })
        .unwrap_or_else(|| "USD".to_string());

    let currency_account_ids = active_accounts
        .iter()
        .filter(|account| account.currency == currency)
        .map(|account| account.id.clone())
        .collect::<Vec<_>>();
    let mut selected_account_ids = account_ids
        .unwrap_or_default()
        .iter()
        .filter(|id| {
            active_accounts
                .iter()
                .any(|account| account.id == **id && account.currency == currency)
        })
        .cloned()
        .collect::<Vec<_>>();
    if selected_account_ids.is_empty() {
        selected_account_ids = currency_account_ids.clone();
    }

    let smart_list_transactions = state
        .database
        .list_transactions()?
        .into_iter()
        .filter(|transaction| {
            transaction.currency == currency
                && transaction
                    .account_profile_id
                    .as_ref()
                    .map(|id| currency_account_ids.contains(id))
                    .unwrap_or(currency_account_ids.is_empty())
        })
        .collect::<Vec<_>>();
    let transactions = smart_list_transactions
        .iter()
        .filter(|transaction| {
            transaction
                .account_profile_id
                .as_ref()
                .map(|id| selected_account_ids.contains(id))
                .unwrap_or(selected_account_ids.is_empty())
        })
        .cloned()
        .collect();
    Ok((
        transactions,
        smart_list_transactions,
        all_accounts,
        selected_account_ids,
        currency,
    ))
}

fn build_snapshot(
    state: &AppState,
    account_ids: Option<Vec<String>>,
    currency: Option<String>,
) -> AppResult<DashboardSnapshot> {
    let (transactions, smart_list_transactions, accounts, selected_account_ids, currency) =
        scoped_transactions(state, account_ids.as_deref(), currency.as_deref())?;
    let health = calculate_health(&transactions);
    let categories = category_totals(&transactions);
    let account_count = selected_account_ids.len();
    let smart_lists = state.database.list_smart_lists()?;
    let library = state.library.scan()?;
    let migration = state
        .migration
        .lock()
        .expect("migration lock poisoned")
        .clone();
    Ok(DashboardSnapshot {
        transaction_count: transactions.len(),
        health,
        transactions,
        smart_list_transactions,
        categories,
        account_count,
        currency,
        accounts,
        selected_account_ids,
        smart_lists,
        library,
        paths: state.paths.clone(),
        migration,
    })
}

#[tauri::command]
fn bootstrap(
    account_ids: Option<Vec<String>>,
    currency: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<DashboardSnapshot> {
    build_snapshot(&state, account_ids, currency)
}

#[tauri::command]
fn inspect_statements(paths: Vec<String>, state: State<'_, AppState>) -> AppResult<ImportPlan> {
    let mut pending = Vec::new();
    let mut detected = Vec::new();
    let mut errors = Vec::new();

    for raw_path in paths {
        let path = PathBuf::from(&raw_path);
        match crate::ofx::parse_ofx_statements(&path) {
            Ok(statements) => {
                for statement in statements {
                    let detected_id = uuid::Uuid::new_v4().to_string();
                    let matched = state.database.match_account(&statement)?;
                    detected.push(DetectedStatement {
                        detected_id: detected_id.clone(),
                        source_file: statement.source_file.clone(),
                        institution_name: Some(crate::identity::institution_name(&statement)),
                        masked_identifier: crate::identity::masked_identifier(
                            statement.account_id.as_deref(),
                        ),
                        source_account_type: statement.account_type.clone(),
                        currency: statement.currency.clone(),
                        period_start: statement.period_start,
                        period_end: statement.period_end,
                        transaction_count: statement.transactions.len(),
                        matched_account_id: matched.map(|account| account.id),
                        identity_reliable: crate::identity::identity_fingerprint(&statement)
                            .is_some(),
                        suggested_nickname: crate::identity::suggested_nickname(&statement),
                    });
                    pending.push(PendingStatement {
                        detected_id,
                        statement,
                    });
                }
            }
            Err(error) => {
                let file_name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("statement.ofx");
                errors.push(format!("{file_name}: {error}"));
            }
        }
    }

    if pending.is_empty() {
        return Err(crate::error::message(
            errors
                .first()
                .cloned()
                .unwrap_or_else(|| "No valid OFX statements were found".to_string()),
        ));
    }
    let plan_id = uuid::Uuid::new_v4().to_string();
    state
        .pending_imports
        .lock()
        .expect("pending import lock poisoned")
        .insert(
            plan_id.clone(),
            PendingImportPlan {
                statements: pending,
            },
        );
    Ok(ImportPlan {
        plan_id,
        statements: detected,
        errors,
    })
}

#[tauri::command]
fn commit_import(
    plan_id: String,
    assignments: Vec<ImportAssignment>,
    state: State<'_, AppState>,
) -> AppResult<ImportReceipt> {
    let plan = state
        .pending_imports
        .lock()
        .expect("pending import lock poisoned")
        .get(&plan_id)
        .cloned()
        .ok_or_else(|| {
            crate::error::message("This import preview expired. Choose the files again.")
        })?;
    let assignments = assignments
        .into_iter()
        .map(|assignment| (assignment.detected_id.clone(), assignment))
        .collect::<HashMap<_, _>>();
    let batch_id = uuid::Uuid::new_v4().to_string();
    state.database.start_import_batch(&batch_id)?;
    let mut receipt = ImportReceipt {
        batch_id: batch_id.clone(),
        files: Vec::new(),
        errors: Vec::new(),
        new_transactions: 0,
        duplicate_transactions: 0,
        conflict_transactions: 0,
    };

    for pending in plan.statements {
        let result = (|| -> AppResult<_> {
            let matched = state.database.match_account(&pending.statement)?;
            let account = if let Some(account) = matched {
                if account.archived {
                    return Err(crate::error::message(format!(
                        "{} matches archived account ‘{}’. Restore it in Settings before importing.",
                        pending.statement.source_file, account.nickname
                    )));
                }
                account
            } else {
                let assignment = assignments.get(&pending.detected_id).ok_or_else(|| {
                    crate::error::message(format!(
                        "Choose an account for {}",
                        pending.statement.source_file
                    ))
                })?;
                if crate::identity::identity_fingerprint(&pending.statement).is_some() {
                    let input = assignment.new_account.as_ref().ok_or_else(|| {
                        crate::error::message(format!(
                            "Create a profile for the newly detected account in {}",
                            pending.statement.source_file
                        ))
                    })?;
                    state
                        .database
                        .ensure_account_for_statement(&pending.statement, Some(input))?
                } else if let Some(account_id) = assignment.account_profile_id.as_deref() {
                    let account = state.database.account_by_id(account_id)?.ok_or_else(|| {
                        crate::error::message("The selected account profile no longer exists")
                    })?;
                    if account.currency != pending.statement.currency {
                        return Err(crate::error::message(format!(
                            "{} is {}, but the selected account uses {}",
                            pending.statement.source_file,
                            pending.statement.currency,
                            account.currency
                        )));
                    }
                    account
                } else {
                    let input = assignment.new_account.as_ref().ok_or_else(|| {
                        crate::error::message(format!(
                            "Choose or create an account for {}",
                            pending.statement.source_file
                        ))
                    })?;
                    state
                        .database
                        .ensure_account_for_statement(&pending.statement, Some(input))?
                }
            };
            if account.currency != pending.statement.currency {
                return Err(crate::error::message(format!(
                    "{} uses {}, but account ‘{}’ uses {}. Currency changes require a separate profile.",
                    pending.statement.source_file,
                    pending.statement.currency,
                    account.nickname,
                    account.currency
                )));
            }
            state
                .database
                .observe_account_statement(&account.id, &pending.statement)?;
            state
                .database
                .import_statement_for_account(&pending.statement, &account.id, &batch_id)
        })();

        match result {
            Ok(file) => {
                receipt.new_transactions += file.new_transactions;
                receipt.duplicate_transactions += file.duplicate_transactions;
                receipt.conflict_transactions += file.conflict_transactions;
                receipt.files.push(file);
            }
            Err(error) => receipt.errors.push(error.to_string()),
        }
    }
    let status = if receipt.errors.is_empty() {
        "complete"
    } else {
        "complete_with_errors"
    };
    state.database.finish_import_batch(&batch_id, status)?;
    state
        .pending_imports
        .lock()
        .expect("pending import lock poisoned")
        .remove(&plan_id);
    Ok(receipt)
}

#[tauri::command]
fn update_transaction(update: TransactionUpdate, state: State<'_, AppState>) -> AppResult<()> {
    state.database.update_transaction(&update)
}

#[tauri::command]
fn update_account(update: AccountUpdate, state: State<'_, AppState>) -> AppResult<AccountProfile> {
    state.database.update_account(&update)
}

#[tauri::command]
fn save_smart_list(input: SmartListInput, state: State<'_, AppState>) -> AppResult<SmartList> {
    state.database.save_smart_list(&input)
}

#[tauri::command]
fn delete_smart_list(id: String, state: State<'_, AppState>) -> AppResult<()> {
    state.database.delete_smart_list(&id)
}

#[tauri::command]
fn refresh_library(state: State<'_, AppState>) -> AppResult<LibraryScan> {
    state.library.scan()
}

#[tauri::command]
fn save_report(
    title: String,
    account_ids: Option<Vec<String>>,
    currency: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<ArtifactActionResult> {
    let (transactions, _, _, _, currency) =
        scoped_transactions(&state, account_ids.as_deref(), currency.as_deref())?;
    let health = calculate_health(&transactions);
    let categories = category_totals(&transactions);
    state
        .library
        .save_report(&title, &currency, health, categories, transactions.len())
}

#[tauri::command]
fn import_artifact(path: String, state: State<'_, AppState>) -> AppResult<ArtifactActionResult> {
    state.library.import_artifact(PathBuf::from(path).as_path())
}

#[tauri::command]
fn trash_artifact(path: String, state: State<'_, AppState>) -> AppResult<ArtifactActionResult> {
    state.library.trash_artifact(PathBuf::from(path).as_path())
}

#[tauri::command]
fn artifact_text(path: String, state: State<'_, AppState>) -> AppResult<String> {
    state.library.serialized(PathBuf::from(path).as_path())
}

#[tauri::command]
fn share_artifact(
    path: String,
    provider: String,
    state: State<'_, AppState>,
) -> AppResult<SharePayload> {
    state
        .library
        .share_payload(PathBuf::from(path).as_path(), &provider)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let documents = app.path().document_dir()?;
            let home = directories::BaseDirs::new()
                .map(|dirs| dirs.home_dir().to_path_buf())
                .unwrap_or_else(|| documents.clone());
            let database_path = app_data.join("gatorfinance.sqlite3");
            let library_path = documents.join("AOS").join("gatorFinance");
            let legacy_path = home.join(".gatorfinance").join("db");
            let database = Database::new(database_path.clone());
            // Legacy beta stores are detected for transparency but never imported implicitly.
            let migration = database.initialize(&legacy_path, false)?;
            let library = OutputLibrary::new(library_path.clone());
            library.ensure()?;
            app.manage(AppState {
                database,
                library,
                paths: AppPaths {
                    database: database_path.to_string_lossy().to_string(),
                    library: library_path.to_string_lossy().to_string(),
                    legacy_database: legacy_path.to_string_lossy().to_string(),
                },
                migration: Mutex::new(migration),
                pending_imports: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            inspect_statements,
            commit_import,
            update_transaction,
            update_account,
            save_smart_list,
            delete_smart_list,
            refresh_library,
            save_report,
            import_artifact,
            trash_artifact,
            artifact_text,
            share_artifact,
        ])
        .run(tauri::generate_context!())
        .expect("error while running gatorFinance");
}
