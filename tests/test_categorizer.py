from parser import TransactionType
from parser.categorizer import categorize_transaction, clean_vendor, detect_recurring


def test_debit_subscription():
    tx_type, category, confidence = categorize_transaction("NETFLIX.COM", -15.99)
    assert tx_type == TransactionType.DEBIT
    assert category == "subscription"
    assert confidence >= 0.9


def test_debit_groceries():
    _, category, _ = categorize_transaction("SUPER 99 VIA GATOR", -52.30)
    assert category == "groceries"


def test_credit_internal_transfer():
    tx_type, category, confidence = categorize_transaction(
        "TRANSFERENCIA DE AHORRO ENTRE CUENTAS", 1500.0
    )
    assert tx_type == TransactionType.CREDIT
    assert category == "transfer_in"
    assert confidence == 0.99


def test_unknown_memo_falls_back_with_low_confidence():
    _, category, confidence = categorize_transaction("XYZZY UNKNOWN VENDOR", -10.0)
    assert category == "other_expense"
    assert confidence == 0.3


def test_clean_vendor_extracts_card_mask():
    clean, mask, normalized = clean_vendor("RESTAURANTE EL CAIMAN-1234-56XX-XXXX-7890")
    assert clean == "RESTAURANTE EL CAIMAN"
    assert mask == "1234-56XX-XXXX-7890"
    assert normalized == "Restaurante El Caiman"


def test_clean_vendor_without_card_mask():
    clean, mask, _ = clean_vendor("NETFLIX.COM")
    assert clean == "NETFLIX.COM"
    assert mask is None


def test_detect_recurring():
    assert detect_recurring("NETFLIX.COM") is True
    assert detect_recurring("SOME RANDOM SHOP") is None
