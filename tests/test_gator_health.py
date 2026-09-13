from datetime import datetime

from engine import calculate_gator_health
from parser import TransactionNormalized


def make_txn(date: datetime, amount_signed: float, recurring: bool = False) -> TransactionNormalized:
    return TransactionNormalized(
        raw_dedupe_hash=f"hash-{date.isoformat()}-{amount_signed}",
        unique_id=f"TEST-{date.isoformat()}-{amount_signed}",
        date=date,
        date_raw=date.strftime("%Y%m%d%H%M%S"),
        amount=abs(amount_signed),
        amount_signed=amount_signed,
        transaction_type="credit" if amount_signed >= 0 else "debit",
        category_auto="other_credit" if amount_signed >= 0 else "other_expense",
        is_recurring=recurring,
        vendor_raw="TEST VENDOR",
        vendor_clean="TEST VENDOR",
    )


def test_empty_transactions_neutral_score():
    result = calculate_gator_health([])
    assert result.score == 50.0
    assert result.grade == "C"
    assert result.months_analyzed == 0


def test_improving_three_month_period():
    txns = []
    for month, expense in [(1, 900.0), (2, 700.0), (3, 400.0)]:
        txns.append(make_txn(datetime(2025, month, 5), 1000.0, recurring=True))
        txns.append(make_txn(datetime(2025, month, 15), -expense))

    result = calculate_gator_health(txns)

    assert result.months_analyzed == 3
    assert result.avg_monthly_income == 1000.0
    assert result.trend_direction == "improving"
    assert 0 <= result.score <= 100
    assert result.best_month.month == 3
    assert result.worst_month.month == 1


def test_excluded_transactions_are_ignored():
    txns = [
        make_txn(datetime(2025, 1, 5), 1000.0),
        make_txn(datetime(2025, 2, 5), 1000.0),
    ]
    excluded = make_txn(datetime(2025, 3, 15), -5000.0)
    excluded.is_excluded = True
    txns.append(excluded)

    result = calculate_gator_health(txns)
    assert result.months_analyzed == 2
    assert result.avg_monthly_expenses == 0.0
