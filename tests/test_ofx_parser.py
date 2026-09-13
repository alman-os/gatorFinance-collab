from datetime import datetime
from pathlib import Path

import pytest

from parser import parse_ofx_to_raw, normalize_statement, check_duplicates
from parser.ofx_parser import parse_ofx_date

FIXTURE = Path(__file__).parent / "fixtures" / "sample_statement.ofx"


def test_parse_ofx_date_full_timestamp():
    assert parse_ofx_date("20250402004344.000") == datetime(2025, 4, 2, 0, 43, 44)


def test_parse_ofx_date_date_only():
    assert parse_ofx_date("20250402") == datetime(2025, 4, 2)


def test_parse_ofx_date_invalid():
    assert parse_ofx_date("") is None
    assert parse_ofx_date("garbage") is None


def test_parse_fixture_counts_and_account():
    raw = parse_ofx_to_raw(FIXTURE)
    assert raw.transaction_count == 4
    assert raw.bank_id == "GATOR"
    assert raw.account_id == "00-00-00-000000-0"
    assert raw.account_type == "CHECKING"
    assert raw.currency == "USD"
    assert raw.ledger_balance == 1419.96


def test_period_falls_back_to_transaction_dates():
    # The fixture reproduces the Banco General quirk: DTSTART == DTEND ==
    # export timestamp (2026-01-01), while transactions span April 2025.
    raw = parse_ofx_to_raw(FIXTURE)
    assert raw.period_start == datetime(2025, 4, 3, 10, 15, 0)
    assert raw.period_end == datetime(2025, 4, 20, 17, 12, 0)


def test_dedupe_hash_is_stable_and_filters():
    first = parse_ofx_to_raw(FIXTURE)
    again = parse_ofx_to_raw(FIXTURE)
    hashes = {t.dedupe_hash for t in first.transactions}
    assert len(hashes) == 4

    new_txns, result = check_duplicates(again.transactions, hashes)
    assert new_txns == []
    assert result.duplicate_transactions == 4
    assert result.has_duplicates


def test_normalize_statement_stats():
    norm = normalize_statement(parse_ofx_to_raw(FIXTURE))
    assert norm.transaction_count == 4
    assert norm.total_credits == pytest.approx(1500.00)
    assert norm.total_debits == pytest.approx(81.04)
    assert norm.net_flow == pytest.approx(1418.96)
