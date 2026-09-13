# Account profiles, OFX identity, deduplication, and smart lists

Planning date: September 12, 2026

## Implementation status

The first production slice is implemented in the current workspace: per-`STMTRS` parsing, versioned account/identity migrations, inspect/commit import flow, account-scoped `FITID` deduplication, payload conflict retention, duplicate-only receipts, currency/account dashboard scope, profile rename/archive controls, and deterministic persisted Smart Lists with live preview. The acceptance tests cover identical reimports, provider revisions, identical provider IDs in separate manual profiles, and multi-account OFX parsing.

The source-entity alias layer, missing-`FITID` review queue, import undo, import-history UI, list rule grouping/exceptions, and high-volume pagination remain follow-up work. They are preserved below as the next expansion path rather than being implied as shipped behavior.

## Product decision

Add three related concepts with distinct jobs:

1. **Account profiles** identify where money lives: “Banco General savings,” “Banco General checking,” and so on.
2. **Source entities** identify who a transaction came from or went to: an employer, Amazon, a utility, or a person.
3. **Smart lists** are saved, deterministic views over transactions: “Income from Studio,” “Amazon purchases in 2026,” or “Utilities from checking.”

The import flow should parse before it writes. On a first encounter, gatorFinance shows the detected account and asks the user to confirm a name. Later statements with the same account identity route automatically. A filename may suggest a display name, but must never determine account identity.

```text
OFX file
  → preflight parse
  → detect account identity
  → match or create account profile
  → classify each transaction as new / duplicate / conflict
  → commit one import batch
  → refresh affected smart lists and show a receipt
```

## What the reference OFX tells us

The local file was inspected without printing transaction descriptions or full financial identifiers. It remains ignored by the repository-wide `*.ofx` rule and must not become a fixture.

| Signal | Result | Design implication |
| --- | --- | --- |
| Statement account blocks | 1 `STMTRS`, 1 `BANKACCTFROM` | This file belongs to one account. Still refactor parsing per statement block so future multi-account OFX files cannot inherit the first account ID globally. |
| Institution fields | `ORG=BG`; bank/FI identifier present | Enough institution context for a Banco General adapter/display suggestion. The short bank identifier is not unique enough by itself. |
| Account identifier | Present once; 17 characters; punctuation included | Strong automatic profile key after normalization. Show only a masked suffix. Preserve leading zeroes. |
| Declared type | `CHECKING` | Treat as a source hint. The filename says savings, so users must be able to choose or correct the visible account type without changing the underlying identity. |
| Currency | USD | Bind currency to the account profile and analyze each currency separately. |
| Transactions | 850 parseable; 381 credits and 469 debits | Suitable for a realistic import/performance fixture after sanitization. |
| Transaction identity | 850 `FITID` fields; all 850 unique | Reimport can be safely idempotent when `FITID` is scoped to the account profile. |
| Transaction dates | January 1 through September 8, 2026 | Useful coverage, although coverage should come from import evidence rather than assuming every intervening date/month is complete. |
| Declared period | `DTSTART` and `DTEND` equal the September 12 export timestamp | Existing fallback to transaction min/max is appropriate for this bank, but should be marked as inferred coverage. |
| Counterparty fields | `MEMO` on all 850; `NAME` on none | Source-entity matching must use normalized `MEMO`/vendor data. `NAME` can be another input when other banks provide it. |

### Answer for this file

Yes, the account number/identifier is present. The first import should say **“New account detected”**, show the institution, masked account suffix, currency, inferred date span, and transaction count, then ask the user to confirm a nickname and visible type. Every later file with the same normalized institution + account identifier should feed that profile automatically and display an import receipt. The user should not have to select the account again.

The type mismatch is a good reason to separate:

- `source_account_type`: the immutable last value observed in OFX (`CHECKING` here), and
- `display_account_type`: the user-controlled label (`Savings`, if that is correct).

## Current behavior and gaps

| Current behavior | Keep or change |
| --- | --- |
| `ofx.rs` builds a dedupe hash from bank ID, account ID, `FITID`, raw date, and amount, then SQLite uses that 16-hex hash as the primary key. | Keep idempotent inserts, but separate account identity, provider transaction identity, and payload fingerprint. A bank correcting the date or amount for an existing `FITID` should become a visible conflict/revision, not a second transaction. Use full-width keys or explicit composite unique indexes. |
| Reimporting the same fixture is already tested: the second import reports all rows as duplicates. | Preserve and broaden this test for overlapping statements, multiple accounts at one bank, and provider revisions. |
| Parser skips transactions without `FITID`. | Accept them only through a cautious fallback path. Exact candidates without a reliable provider ID should be reviewed instead of silently merged. |
| Parser reads the first global `BANKID`/`ACCTID` and applies it to every transaction block. | Parse each `STMTRS`/account statement independently and return one or more detected statements. |
| Full account ID is retained in `raw_transactions`; `transactions` retains a masked label only. | Add a foreign key to an account profile. Never identify an account by its display label or last four digits. Keep raw identifiers local and excluded from reports. |
| `account_count()` counts distinct masked labels. | Count account profile IDs. Two accounts with the same suffix must remain distinct. |
| Duplicate-only imports are not written to the `imports` table because the row is added only when new transactions exist. | Record every import attempt/batch so users can see that a file was recognized and contained 850 duplicates rather than wondering whether it ran. |
| Dashboard and reports currently combine all imported transactions and pick the most frequent currency as the label. | Account profiles must introduce explicit account/currency scope. Never add unlike currencies into one displayed total. |

## Data model

Introduce versioned SQLite migrations before changing the schema. `CREATE TABLE IF NOT EXISTS` alone cannot safely backfill new relationships. Use `PRAGMA user_version` and run each migration transactionally.

### `account_profiles`

| Column | Purpose |
| --- | --- |
| `id TEXT PRIMARY KEY` | Internal UUID; stable even when the user renames the account. |
| `nickname TEXT NOT NULL` | User-controlled name. |
| `institution_name TEXT` | Friendly institution label. |
| `display_account_type TEXT` | User-controlled type. |
| `source_account_type TEXT` | Latest OFX hint for diagnostics. |
| `currency TEXT NOT NULL` | Account/report scope. A changed currency becomes a conflict, not an automatic mutation. |
| `masked_identifier TEXT` | Display-only suffix. Never used for matching. |
| `created_at`, `updated_at`, `archived_at` | Lifecycle fields; archive instead of deleting profiles with transactions. |

### `account_identities`

Use a separate identity table because exports can expose more than one valid alias over time.

| Column | Purpose |
| --- | --- |
| `id TEXT PRIMARY KEY` | Identity UUID. |
| `account_profile_id TEXT NOT NULL` | Parent profile. |
| `identity_version INTEGER NOT NULL` | Version of canonicalization. |
| `institution_key TEXT NOT NULL` | Canonical `FID`, `BANKID`, or bank-adapter key. |
| `account_key TEXT NOT NULL` | Canonical `ACCTID`, preserving leading zeroes while removing agreed formatting separators. |
| `identity_fingerprint TEXT NOT NULL UNIQUE` | Full SHA-256 of the versioned institution/account tuple for indexed matching. |
| `last_seen_at TEXT NOT NULL` | Diagnostics and stale-identity management. |

Hashing is data minimization, not encryption: numeric account IDs can have low entropy. The current database already retains full IDs in raw imports. The first implementation can preserve that behavior while keeping identifiers out of UI/logs/reports; storage encryption or macOS Keychain-backed HMAC can be evaluated as a separate security change.

### Import provenance and transaction identity

- `import_batches`: one row per user action, with start/end timestamps and final status.
- `import_files`: file name, full file SHA-256, detected profile, inferred/declared coverage, counts for new/duplicate/conflict/rejected, and a safe error code/message.
- `raw_transactions.account_profile_id` and `transactions.account_profile_id`: required foreign keys after migration.
- `transactions.id`: stable internal UUID for notes, tags, list exceptions, and future provider revisions.
- `provider_transaction_id`: normalized `FITID`.
- Unique index on `(account_profile_id, provider_transaction_id)` when the provider ID exists.
- `payload_fingerprint`: full hash of canonical signed minor-unit amount, posted date, reference number, and source memo for equality/conflict checks.
- `import_file_transactions`: join table recording which file contained which transaction, including duplicates. One transaction can appear in many overlapping exports.
- `import_conflicts`: retains incoming metadata when a known provider ID arrives with a changed canonical payload.

Do not use a floating-point amount string in a new identity calculation. Parse the signed value into integer minor units first. Currency scale must eventually come from currency metadata rather than assuming two decimal places for every currency.

## Deterministic deduplication policy

For each parsed transaction:

| Situation | Result |
| --- | --- |
| Known account + known `FITID` + same payload fingerprint | Duplicate. Link it to the new import file and do not alter the transaction or user overrides. |
| Known account + known `FITID` + changed payload | Conflict/revision. Do not silently insert or overwrite; preserve incoming source data and show a resolution path. |
| Different account + same `FITID` | New transaction. Provider IDs are scoped to an account. |
| Reliable account identity + new `FITID` | New transaction. |
| Missing account ID | Require the user to choose/create an account profile before commit. Do not learn a permanent automatic mapping from filename alone. |
| Missing `FITID` | Compute a fallback candidate fingerprint from account, posted date, signed minor amount, normalized memo and reference. Exact matches become “possible duplicates” unless a bank-specific adapter has established that the fallback is safe. |
| Same file bytes imported again | Fast-path as previously seen, but still create a receipt. File hash is an audit/optimization signal, not the transaction dedupe key. |

The entire commit for one statement/account should be atomic. In a multi-file selection, one malformed file may fail without rolling back successful files, but the final receipt must show each result.

## Account-profile onboarding

### Preflight API

Replace the single `import_statements(paths)` write call with a two-step contract:

1. `inspect_statements(paths) -> ImportPlan`
2. `commit_import(plan_id, account_assignments) -> ImportReceipt`

The plan is short-lived and local. It contains masked metadata, statement/file counts, detected account candidates, duplicates/conflicts, and validation errors. The backend rechecks file fingerprints during commit so a changed file cannot be committed from a stale plan.

### Matching states

| State | Interaction |
| --- | --- |
| Exact known identity | “Importing to **Banco General savings ·••••**” with a Change link before commit; later this can be a preference for instant import. |
| Strong unseen identity | “New account detected.” Prefill institution/currency and suggest a nickname; user confirms nickname and display type once. |
| Missing/weak identity | “Which account is this statement for?” Require an existing profile or new profile. Explain that the file does not contain a reliable account identifier. |
| Several account blocks in one file | Show one row per detected account and route each independently. |
| Known identity with changed currency/type hint | Show a conflict. Nickname/type-hint changes are reviewable; currency changes block automatic commit. |
| User chose the wrong profile | Import receipt offers **Undo import** for newly inserted rows and profile assignment. Duplicates that predated the batch are never deleted by undo. |

Use visual hierarchy to keep this calm: the detected account and destination are the primary group; nickname/type are the one-time setup group; technical fields such as bank code and identity version live under Details. Never ask users to copy or type the account number when it already exists in the OFX.

### Profile management

Add an Accounts area where users can rename, reorder, archive, and inspect import history. Show nickname, institution, type, currency, masked suffix, coverage, and last import. Identity editing should be an advanced repair flow because it changes routing. Do not implement profile merge in the first release; incorrect merges are difficult to recover safely.

All dashboard, transaction, category, and report queries receive explicit `account_profile_ids` plus one currency. “All accounts” is allowed only within a single currency; otherwise show separate currency sections.

## Source entities and smart lists

### Why two layers

A transaction memo can vary even when the underlying entity is the same. “Amazon Prime,” “Amazon Mktpl,” and a user-corrected “Amazon” should be able to belong to one source entity. A smart list then references that entity plus optional conditions instead of repeating fragile text searches.

### `source_entities` and aliases

- `source_entities`: UUID, display name, optional kind (`merchant`, `income_source`, `person`, `institution`, `other`), timestamps.
- `entity_alias_rules`: entity ID, field (`vendor_effective`, `memo`, `reference`), operator, normalized pattern, optional direction/account scope, priority, enabled state, rule version.
- `transactions.source_entity_id`: resolved entity, plus `entity_match_rule_id` and a user-override flag.
- Explicit user assignment wins over alias rules; the original memo and automatic vendor remain intact.

Start with safe operators: exact normalized vendor, contains normalized text, starts with, and optional anchored regular expression under an Advanced disclosure. Regex compilation errors must be caught before save. Never execute arbitrary code or SQL from a rule.

### Smart-list predicate model

Store predicates as versioned JSON validated into a Rust enum, not as user-authored SQL. First-release fields:

- source entity or effective vendor;
- direction: credit/debit;
- account profile(s);
- category and tag;
- date range or rolling period;
- signed/absolute amount range;
- reviewed, excluded, or recurring state;
- all/any grouping with one nesting level;
- manual include/exclude transaction exceptions.

Examples:

```text
Income from X
  source entity = X
  direction = credit
  accounts = selected income accounts

Amazon purchases · 2026
  source entity = Amazon
  direction = debit
  date = Jan 1–Dec 31, 2026
```

Smart lists remain dynamic: new matching imports appear automatically. Each list shows count, total, monthly trend, accounts included, last matched transaction, and its rule in plain language. A transaction should provide **Track this source**; it opens a prefilled list preview rather than saving invisibly.

When a rule or vendor-normalization version changes, recompute membership transactionally and show the before/after count. Manual exclusions remain stable. This is what makes the feature resilient without AI: deterministic inputs, visible rules, previews, explainable matches, and user-controlled exceptions.

## Delivery sequence

### PR 1 — Parser preflight and sanitized fixtures

- Parse per `STMTRS` block and return detected account metadata separately from transactions.
- Add safe account canonicalization and masking helpers.
- Create a fully synthetic Banco General-style fixture that mirrors the reference structure: punctuated 17-character fake account ID, equal declared start/end timestamps, `MEMO` without `NAME`, overlapping dates, and fake `FITID`s.
- Add preflight models without changing production import behavior yet.

### PR 2 — Versioned schema and account migration

- Introduce transactional `PRAGMA user_version` migrations.
- Create profile, identity, batch/file provenance, and transaction-identity columns/tables.
- Backfill profiles from distinct canonical `(bank_id, account_id, currency)` values in `raw_transactions`.
- Put rows lacking reliable identity in a visible **Unassigned imports** repair state.
- Preserve existing dedupe hashes, overrides, notes, tags, exclusions, and report files.

### PR 3 — Plan/commit import and stronger dedupe

- Implement account-scoped provider IDs, payload fingerprints, conflict handling, and complete import receipts.
- Record duplicate-only imports.
- Add undo limited to rows first created by the selected import batch.
- Route all analysis by profile/currency, resolving the current mixed-currency risk.

### PR 4 — Account onboarding and management

- Add known/new/ambiguous/conflict preflight screens in English and Spanish.
- Add account selector/scope to Overview and Transactions.
- Add Accounts settings with rename/archive/import history.
- Preserve drag-and-drop and multi-file selection.

### PR 5 — Source entities and smart lists

- Implement the validated rule engine in Rust and entity-resolution provenance.
- Add Track this source, list builder with live preview, list results, summary metrics, edit, disable, and delete/undo behavior.
- Keep report export privacy-safe; smart-list definitions and transaction membership stay out of summary reports unless a future schema explicitly opts them in.

## Acceptance matrix

Account and dedupe tests:

- identical reference-shaped file twice → first import new, second import all duplicate, two receipts;
- overlapping exports → only unseen `FITID`s are inserted;
- same bank, two account IDs, same last four → two profiles and correct routing;
- same `FITID` in two profiles → two transactions;
- same profile/`FITID`, changed amount/date → one unresolved revision, no silent duplicate/overwrite;
- formatted and unformatted variants of the same account ID → match only when the versioned canonicalization rule says they are equivalent;
- missing account ID → explicit account assignment required;
- missing `FITID` → possible-duplicate review path;
- multi-account OFX → no transaction inherits another block’s account;
- duplicate-only file, malformed file, and mixed multi-file selection → accurate per-file receipts;
- failed commit → no partial statement rows;
- migration → transaction count and every user override remain identical;
- archive/rename → profile identity and dedupe behavior remain unchanged;
- two currencies → no combined monetary score or report.

Smart-list tests:

- effective vendor exact/contains matching, case and whitespace normalization;
- credit-only income list and debit-only merchant list;
- account/date/amount/category/tag predicates with all/any grouping;
- user vendor/entity override precedence;
- alias added/disabled → explainable membership diff;
- manual include/exclude survives rule edits and app restart;
- new OFX import refreshes affected list counts;
- invalid or pathological regex is rejected/bounded;
- 50k synthetic transactions remain responsive before choosing pagination or virtualization.

## Recommended first usable slice

Ship account profiles and import receipts before smart lists. The slice is complete when this reference-shaped flow works:

1. Drop the first statement onto gatorFinance.
2. See “New Banco General account detected,” masked identity, USD, 850 transactions, and inferred January–September coverage.
3. Confirm a nickname/type and import.
4. Drop the same statement again and see “0 new · 850 already imported” under the same account.
5. Drop an overlapping later statement and see only new transactions added.
6. Filter the entire app to that account without mixing currencies or accounts that share a suffix.

Then add **Track this source** and the two example smart lists. This order gives the rules a reliable account and transaction identity foundation instead of making saved lists depend on today’s masked labels and truncated hashes.
