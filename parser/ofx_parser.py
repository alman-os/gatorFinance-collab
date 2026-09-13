"""
gatorFinance - OFX Parser (v0.2)
================================
Parse OFX (Open Financial Exchange) files into structured Transaction objects.

ARCHITECTURE (v0.2):
====================
Two-stage parsing:
1. parse_ofx_to_raw() -> RawStatement (immutable, exactly as imported)
2. normalize_statement() -> NormalizedStatement (derived, can be re-run)

This separation allows:
- Re-running normalization without re-importing
- User overrides that persist across re-normalization
- Reliable deduplication via dedupe_hash
- Version-tracked normalization logic
"""

import re
from datetime import datetime
from pathlib import Path
from typing import Optional

from .models import (
    TransactionRaw,
    TransactionNormalized,
    RawStatement,
    NormalizedStatement,
    TransactionType,
    # Legacy compatibility
    Transaction,
    ParsedStatement,
)
from .categorizer import categorize_transaction, clean_vendor, detect_recurring


# =============================================================================
# OFX PARSING PATTERNS
# =============================================================================

# Header parsing
HEADER_PATTERN = re.compile(r'^([A-Z]+):(.*)$', re.MULTILINE)

# Main sections
STMTTRN_PATTERN = re.compile(r'<STMTTRN>(.*?)</STMTTRN>', re.DOTALL)

# Individual fields within a transaction
FIELD_PATTERNS = {
    'TRNTYPE': re.compile(r'<TRNTYPE>([^<\n]+)'),
    'DTPOSTED': re.compile(r'<DTPOSTED>([^<\n]+)'),
    'TRNAMT': re.compile(r'<TRNAMT>([^<\n]+)'),
    'FITID': re.compile(r'<FITID>([^<\n]+)'),
    'REFNUM': re.compile(r'<REFNUM>([^<\n]+)'),
    'MEMO': re.compile(r'<MEMO>([^<\n]+)'),
}

# Account info
ACCTID_PATTERN = re.compile(r'<ACCTID>([^<\n]+)')
ACCTTYPE_PATTERN = re.compile(r'<ACCTTYPE>([^<\n]+)')
BANKID_PATTERN = re.compile(r'<BANKID>([^<\n]+)')
CURDEF_PATTERN = re.compile(r'<CURDEF>([^<\n]+)')

# Balance
LEDGERBAL_PATTERN = re.compile(r'<LEDGERBAL>.*?<BALAMT>([^<\n]+)', re.DOTALL)
BALDATE_PATTERN = re.compile(r'<DTASOF>([^<\n]+)')

# Statement period
DTSTART_PATTERN = re.compile(r'<DTSTART>([^<\n]+)')
DTEND_PATTERN = re.compile(r'<DTEND>([^<\n]+)')


# =============================================================================
# DATE PARSING
# =============================================================================

def parse_ofx_date(date_str: str) -> Optional[datetime]:
    """
    Parse OFX date format: YYYYMMDDHHMMSS.XXX or YYYYMMDD

    Examples:
        20250402004344.000 -> datetime(2025, 4, 2, 0, 43, 44)
        20250402 -> datetime(2025, 4, 2, 0, 0, 0)
    """
    if not date_str:
        return None

    date_str = date_str.strip()

    try:
        # Remove milliseconds if present
        if '.' in date_str:
            date_str = date_str.split('.')[0]

        if len(date_str) >= 14:
            # Full datetime: YYYYMMDDHHMMSS
            return datetime.strptime(date_str[:14], '%Y%m%d%H%M%S')
        elif len(date_str) >= 8:
            # Date only: YYYYMMDD
            return datetime.strptime(date_str[:8], '%Y%m%d')
        else:
            return None
    except ValueError:
        return None


# =============================================================================
# LAYER 1: RAW TRANSACTION PARSING (IMMUTABLE)
# =============================================================================

def parse_transaction_raw(
    trn_block: str,
    source_file: str,
    currency: str = "USD",
    account_id: Optional[str] = None,
    bank_id: Optional[str] = None
) -> Optional[TransactionRaw]:
    """
    Parse a single <STMTTRN> block into a TransactionRaw object.

    This produces IMMUTABLE raw data exactly as imported.
    """

    # Extract fields
    fields = {}
    for field_name, pattern in FIELD_PATTERNS.items():
        match = pattern.search(trn_block)
        fields[field_name] = match.group(1).strip() if match else None

    # Validate required fields
    if not fields.get('FITID') or not fields.get('TRNAMT') or not fields.get('DTPOSTED'):
        return None

    # Parse amount
    try:
        trnamt = float(fields['TRNAMT'])
    except (ValueError, TypeError):
        return None

    # Build raw transaction (immutable)
    return TransactionRaw(
        fitid=fields['FITID'],
        refnum=fields.get('REFNUM'),
        dtposted=fields['DTPOSTED'],
        trnamt=trnamt,
        trntype=fields.get('TRNTYPE', 'OTHER'),
        memo=fields.get('MEMO', ''),
        source_file=source_file,
        account_id=account_id,
        bank_id=bank_id,
        currency=currency,
    )


# =============================================================================
# LAYER 2: NORMALIZATION (DERIVED)
# =============================================================================

def normalize_transaction(raw: TransactionRaw) -> Optional[TransactionNormalized]:
    """
    Normalize a raw transaction into app-ready format.

    This can be re-run at any time to apply updated normalization logic.
    """

    # Parse date
    date = parse_ofx_date(raw.dtposted)
    if not date:
        return None

    # Categorize
    tx_type, category, confidence = categorize_transaction(raw.memo, raw.trnamt)

    # Clean vendor
    vendor_clean, card_mask, vendor_normalized = clean_vendor(raw.memo)

    # Detect recurring
    is_recurring = detect_recurring(raw.memo)

    return TransactionNormalized(
        # Link to raw
        raw_dedupe_hash=raw.dedupe_hash,
        unique_id=raw.unique_id,

        # Parsed temporal
        date=date,
        date_raw=raw.dtposted,

        # Parsed financial
        amount=abs(raw.trnamt),
        amount_signed=raw.trnamt,
        currency=raw.currency,
        transaction_type=tx_type,

        # Auto-classification
        category_auto=category,
        category_confidence=confidence,
        is_recurring=is_recurring,

        # Cleaned vendor
        vendor_raw=raw.memo,
        vendor_clean=vendor_clean,
        vendor_normalized=vendor_normalized,
        card_mask=card_mask,
    )


def normalize_statement(raw_statement: RawStatement) -> NormalizedStatement:
    """
    Normalize an entire raw statement.

    This can be re-run to apply updated normalization logic.
    """
    normalized_txns = []

    for raw_txn in raw_statement.transactions:
        norm_txn = normalize_transaction(raw_txn)
        if norm_txn:
            normalized_txns.append(norm_txn)

    # Sort by date
    normalized_txns.sort(key=lambda t: t.date)

    # Build normalized statement
    statement = NormalizedStatement(
        source_file=raw_statement.source_file,
        bank_id=raw_statement.bank_id,
        account_id=raw_statement.account_id,
        account_type=raw_statement.account_type,
        currency=raw_statement.currency,
        period_start=raw_statement.period_start,
        period_end=raw_statement.period_end,
        ledger_balance=raw_statement.ledger_balance,
        transactions=normalized_txns,
    )

    return statement.compute_stats()


# =============================================================================
# MAIN PARSER (TWO-STAGE)
# =============================================================================

def parse_ofx_to_raw(file_path: str | Path) -> RawStatement:
    """
    STAGE 1: Parse an OFX file into a RawStatement (immutable).

    This is the import stage. Raw data is never modified.
    """
    file_path = Path(file_path)

    # Read file (handle encoding issues)
    try:
        content = file_path.read_text(encoding='utf-8')
    except UnicodeDecodeError:
        content = file_path.read_text(encoding='latin-1')

    # Extract account info
    acctid_match = ACCTID_PATTERN.search(content)
    accttype_match = ACCTTYPE_PATTERN.search(content)
    bankid_match = BANKID_PATTERN.search(content)
    curdef_match = CURDEF_PATTERN.search(content)

    account_id = acctid_match.group(1).strip() if acctid_match else None
    account_type = accttype_match.group(1).strip() if accttype_match else None
    bank_id = bankid_match.group(1).strip() if bankid_match else None
    currency = curdef_match.group(1).strip() if curdef_match else "USD"

    # Extract statement period
    dtstart_match = DTSTART_PATTERN.search(content)
    dtend_match = DTEND_PATTERN.search(content)

    period_start = parse_ofx_date(dtstart_match.group(1)) if dtstart_match else None
    period_end = parse_ofx_date(dtend_match.group(1)) if dtend_match else None

    # Extract balance
    ledgerbal_match = LEDGERBAL_PATTERN.search(content)
    ledger_balance = float(ledgerbal_match.group(1)) if ledgerbal_match else None

    # Parse all transactions
    transactions = []
    for trn_match in STMTTRN_PATTERN.finditer(content):
        trn_block = trn_match.group(1)
        raw_txn = parse_transaction_raw(
            trn_block,
            source_file=file_path.name,
            currency=currency,
            account_id=account_id,
            bank_id=bank_id
        )
        if raw_txn:
            transactions.append(raw_txn)

    # Some banks (e.g. Banco General) stamp DTSTART/DTEND with the export
    # timestamp instead of the statement period, yielding a zero-length range.
    # Fall back to the actual transaction date span.
    if transactions and (period_start is None or period_end is None or period_start == period_end):
        tx_dates = [d for d in (parse_ofx_date(t.dtposted) for t in transactions) if d]
        if tx_dates:
            period_start = min(tx_dates)
            period_end = max(tx_dates)

    # Build raw statement
    return RawStatement(
        source_file=file_path.name,
        bank_id=bank_id,
        account_id=account_id,
        account_type=account_type,
        currency=currency,
        period_start=period_start,
        period_end=period_end,
        ledger_balance=ledger_balance,
        transactions=transactions,
    ).finalize()


def parse_ofx_file(file_path: str | Path) -> NormalizedStatement:
    """
    FULL PIPELINE: Parse OFX file -> Raw -> Normalized.

    This is the convenience function that does both stages.
    For more control, use parse_ofx_to_raw() + normalize_statement().
    """
    raw_statement = parse_ofx_to_raw(file_path)
    return normalize_statement(raw_statement)


# =============================================================================
# LEGACY COMPATIBILITY
# =============================================================================

def parse_ofx_string(content: str, source_name: str = "inline") -> NormalizedStatement:
    """
    Parse OFX content from a string (useful for testing).
    Legacy function maintained for backwards compatibility.
    """
    # Extract account info
    acctid_match = ACCTID_PATTERN.search(content)
    accttype_match = ACCTTYPE_PATTERN.search(content)
    bankid_match = BANKID_PATTERN.search(content)
    curdef_match = CURDEF_PATTERN.search(content)

    account_id = acctid_match.group(1).strip() if acctid_match else None
    account_type = accttype_match.group(1).strip() if accttype_match else None
    bank_id = bankid_match.group(1).strip() if bankid_match else None
    currency = curdef_match.group(1).strip() if curdef_match else "USD"

    # Parse transactions to raw first
    raw_transactions = []
    for trn_match in STMTTRN_PATTERN.finditer(content):
        trn_block = trn_match.group(1)
        raw_txn = parse_transaction_raw(
            trn_block,
            source_file=source_name,
            currency=currency,
            account_id=account_id,
            bank_id=bank_id
        )
        if raw_txn:
            raw_transactions.append(raw_txn)

    # Build raw statement
    raw_statement = RawStatement(
        source_file=source_name,
        bank_id=bank_id,
        account_id=account_id,
        account_type=account_type,
        currency=currency,
        transactions=raw_transactions,
    ).finalize()

    # Normalize and return
    return normalize_statement(raw_statement)
