<p align="center">
  <img src="icon_gatorFinance.png" width="112" alt="gatorFinance app icon">
</p>

# gatorFinance

Local-first macOS finance analysis from your OFX exports.

Import statements, correct the categorization, inspect monthly cash flow, and save privacy-safe `gatorHealth` reports without connecting a bank account or uploading your transaction history.

> **Alpha:** validate important figures against your statements. `gatorHealth` is a directional signal, not financial advice.

## Table of Contents

- [What Does This Do](#what-does-this-do)
- [Installation](#installation)
- [Usage](#usage)
- [What Can You Do With This](#what-can-you-do-with-this)
- [Privacy](#privacy)
- [Tech Details](#tech-details)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## What Does This Do

- Imports one or more OFX statements by file picker or drag and drop.
- Stores money as integer minor units in a local SQLite database.
- Previews every OFX import before writing and routes it to a durable account profile.
- Deduplicates provider transaction IDs within each account, skips identical reimports, and holds changed payloads as conflicts.
- Supports multiple accounts at the same bank, including accounts with the same visible last four digits.
- Keeps dashboards and reports scoped to one account or one currency at a time.
- Normalizes vendor names and categorizes credits and debits with inspectable rules.
- Preserves category, vendor, note, tag, and score-exclusion overrides.
- Excludes internal transfers from income, expenses, and `gatorHealth`.
- Shows monthly net flow, score components, and expense category totals.
- Saves strict `.gatorfinance.json` summary reports under the common AOS output root.
- Copies, reveals, imports, and moves saved reports to Trash from the Library.
- Previews the exact privacy-safe summary before opening ChatGPT or Claude.
- Switches the full interface between English and Spanish without restarting.
- Provides persistent dark, light, and system themes plus a header quick toggle.
- Saves deterministic Smart Lists for a merchant or income source, direction, account, category, tag, date, and amount range.
- Detects the previous `~/.gatorfinance/db` beta store but never imports it automatically.

## Installation

### macOS alpha

**Requires:** macOS 12 or newer. The universal build supports Apple Silicon and Intel Macs.

1. Download [gatorFinance 0.4.0-alpha.4](https://github.com/alman-os/gatorFinance-collab/releases/download/v0.4.0-alpha.4/gatorFinance_0.4.0-alpha.4_macOS-universal_notarized.dmg).
2. Open the DMG and drag `gatorFinance` to Applications.
3. Launch `gatorFinance` and choose one or more `.ofx` files.

The release DMG is Developer ID signed, notarized, and stapled. A checksum is published beside it:

```bash
cd ~/Downloads
shasum -a 256 -c gatorFinance_0.4.0-alpha.4_macOS-universal_notarized.dmg.sha256
```

**Download not found:** alpha artifacts appear on the [Releases page](https://github.com/alman-os/gatorFinance/releases) when published. Build from source in the meantime.

**No transactions imported:** confirm the export is OFX, not CSV, QFX, PDF, or an HTML download from the bank portal.

**Older beta data:** first launch detects `~/.gatorfinance/db` and leaves it untouched. Import current OFX exports to create a clean ledger; beta data is never restored automatically.

### Build from source

Install Node.js 22+, pnpm, Rust stable, and the Xcode command line tools.

```bash
git clone https://github.com/alman-os/gatorFinance-collab.git
cd gatorFinance-collab
pnpm install
pnpm tauri dev
```

## Usage

1. Click **Import OFX** or drop statements onto the window.
2. Confirm the account profile when a new account is detected. Later exports with the same OFX identity route automatically.
3. Use the account selector to view one account or all accounts in the selected currency.
4. Review the score, monthly net flow, score components, and category totals in **Overview**.
5. Open **Transactions** to search the ledger or correct a category, vendor, note, tags, or score exclusion.
6. Open **Smart Lists** to save a live, rule-based view such as Amazon debits this year or credits from one payer.
7. Return to **Overview** and click **Save Report**.
8. Use **Library** to inspect, copy, reveal, import, share, or move a report to Trash.

### Account identity and repeat imports

gatorFinance matches accounts using a versioned fingerprint of the institution and full OFX account identifier. The interface exposes only the masked suffix. A first import creates a named profile; the same account is recognized on later imports even if the profile is renamed.

Within a profile, `FITID` is the provider transaction ID. The importer compares a second payload fingerprint built from the signed minor-unit amount, date, reference, and memo:

- the same `FITID` and payload is reported as an already imported duplicate;
- the same `FITID` with a changed payload is retained as a conflict and does not overwrite the ledger;
- the same `FITID` in a different account is a separate transaction.

Every import produces a receipt, including files that contain only duplicates. Files without a reliable account identity require an explicit account choice.

### Smart Lists

Smart Lists use inspectable rules and do not call an AI service. Source matching is case- and accent-insensitive, and supports contains, exact, or starts-with matching. Optional predicates cover credit/debit direction, account profiles, category, tag, dates, and amount range. Lists update whenever newly imported transactions match their saved rule.

### Report library

Shareable reports are written to:

```text
~/Documents/AOS/gatorFinance/<title>.gatorfinance.json
```

Each report contains a schema version, artifact ID, source app, engine version, currency, period, privacy marker, health summary, and category totals. It does not contain account IDs, card masks, raw memos, or individual transactions.

### Appearance

Use the Sun/Moon control in the header for a quick light or dark switch. Open **Settings** to choose dark, light, or system appearance.

Use **EN / ES** in the header to switch the complete interface between English and Spanish. Theme and language choices persist locally.

## What Can You Do With This

- Audit a year of bank exports without granting account access to another service.
- Correct rule-based categories while keeping the original imported values intact.
- Find spending changes and recurring commitments month by month.
- Exclude reimbursements or unusual entries from the score without deleting them.
- Keep versioned financial-health snapshots for personal review.
- Share aggregate context with an AI provider without sharing the transaction ledger.
- Inspect or extend the parser and scoring engine from source.

## Privacy

gatorFinance has no telemetry, hosted database, or bank integration. OFX parsing, normalization, categorization, storage, and scoring run on your Mac.

The internal database is stored at:

```text
~/Library/Application Support/com.almanos.gatorfinance/gatorfinance.sqlite3
```

ChatGPT and Claude actions are opt-in. The app shows a confirmation, copies a summary without transactions or identifiers to the clipboard, and opens a new chat in the selected provider. Paste with Command-V to review the exact copied text before sending it.

OFX files are bank records. Do not commit real statements, account IDs, card masks, or generated transaction exports to this repository.

## Tech Details

- Desktop shell: Tauri v2.
- Interface: React, TypeScript, Vite, and locally bundled fonts.
- Domain engine: Rust.
- Storage: SQLite in WAL mode through `rusqlite`.
- Money representation: signed 64-bit integer minor units.
- Output schema: versioned `.gatorfinance.json` artifacts.
- Bundle ID: `com.almanos.gatorfinance`.
- Engine version: `0.4.0-alpha.1`.
- Minimum macOS version: 12.0.

The original Python implementation remains in `parser/`, `store/`, and `engine/` as a behavioral reference. Rust parity tests preserve the previous `gatorHealth v0.3` result while the app uses corrected v0.4 behavior.

Banco General OFX exports can stamp the declared statement period with the export timestamp. When that period is missing or zero-length, gatorFinance derives it from transaction dates. The ledger balance still reflects export time.

The importer parses each `STMTRS` block independently, so a multi-account OFX cannot leak one account identifier into another statement block. Account profiles, identities, import batches, conflicts, and Smart Lists are managed through versioned SQLite migrations.

## Development

Run the webview and native test suites:

```bash
pnpm build
pnpm test:native

python3 -m venv .venv
source .venv/bin/activate
python -m pip install -r requirements-dev.txt
python -m pytest
```

Build a local macOS app:

```bash
pnpm tauri build --bundles app
```

Maintainers with the Alman OS Developer ID certificate and `AudioGrabberNotary` keychain profile can produce the verified universal release artifact:

```bash
pnpm package:macos
```

The final files are written to `dist/` with a SHA-256 checksum. The script verifies both architectures, signing, app notarization, stapling, mounted-app Gatekeeper assessment, DMG notarization, DMG Gatekeeper assessment, and disk-image integrity.

## Contributing

Focused fixes are welcome. Add sanitized OFX fixtures for new bank variants, keep parser and scoring changes covered by tests, and never submit real financial data. See [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request.

This repository is source-available so users can inspect local storage and contribute fixes. It does not grant permission to ship competing clones.

## License

This project is source-available under the PolyForm Shield License 1.0.0.

You may read the source, learn from it, and contribute. You may not sell, repackage, white-label, or distribute competing versions of this app without a commercial license.

Commercial licensing: business@alman-os.com

See [LICENSE.md](LICENSE.md).
