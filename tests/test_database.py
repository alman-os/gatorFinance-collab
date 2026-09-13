from pathlib import Path

from parser import parse_ofx_to_raw
from store import open_database

FIXTURE = Path(__file__).parent / "fixtures" / "sample_statement.ofx"


def test_import_dedup_and_reload(tmp_path):
    db_path = tmp_path / "db"

    db = open_database(db_path)
    result = db.import_raw_statement(parse_ofx_to_raw(FIXTURE))
    assert result.new_transactions == 4
    assert db.get_transaction_count() == 4

    # Re-importing the same statement adds nothing
    result = db.import_raw_statement(parse_ofx_to_raw(FIXTURE))
    assert result.new_transactions == 0
    assert result.duplicate_transactions == 4
    assert db.get_transaction_count() == 4

    # A fresh handle reloads the same state from disk
    reloaded = open_database(db_path)
    assert reloaded.get_transaction_count() == 4
    assert reloaded.get_meta().total_imports == 1


def test_override_survives_renormalization(tmp_path):
    db = open_database(tmp_path / "db")
    db.import_raw_statement(parse_ofx_to_raw(FIXTURE))

    target = db.get_all_transactions()[0]
    assert db.set_override(
        target.raw_dedupe_hash, category="entertainment", notes="test note"
    )

    db.renormalize_all()

    updated = next(
        t for t in db.get_all_transactions()
        if t.raw_dedupe_hash == target.raw_dedupe_hash
    )
    assert updated.category_user == "entertainment"
    assert updated.category_effective == "entertainment"
    assert updated.notes_user == "test note"
