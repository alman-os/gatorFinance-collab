# Contributing to gatorFinance

Thank you for helping improve gatorFinance. Focused bug fixes, accessibility improvements, parser compatibility work, deterministic categorization rules, and clear documentation changes are welcome.

## Protect financial data

Never commit a real bank statement, account identifier, card mask, transaction export, SQLite database, or generated report. The repository blocks `.ofx` files by default. The sole exception is `tests/fixtures/sample_statement.ofx`, which is synthetic.

When adding support for another OFX variant, create the smallest synthetic fixture that reproduces the structure. Replace institution names, account identifiers, transaction IDs, dates, amounts, references, and memo text with fabricated values before adding it to the repository.

## Development setup

The desktop app requires Node.js 22+, pnpm, Rust stable, and the Xcode command line tools on macOS.

```bash
pnpm install
pnpm tauri dev
```

The original Python implementation remains as a behavioral reference. Its tests use the packages in `requirements-dev.txt`.

## Before opening a pull request

Run the checks relevant to your change:

```bash
pnpm build
pnpm test:native

python3 -m venv .venv
source .venv/bin/activate
python -m pip install -r requirements-dev.txt
python -m pytest
```

Keep pull requests narrow, explain the user-visible behavior, and include a synthetic regression test when correcting parser, identity, deduplication, storage, or scoring behavior.

Contributions are accepted under the repository's [PolyForm Shield License](LICENSE.md).
