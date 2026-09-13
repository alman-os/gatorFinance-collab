"""
gatorFinance - Transaction Categorizer
======================================
Heuristic-based auto-categorization of transactions.

This module applies pattern matching to MEMO fields to assign categories.
Users can override these later in the UI.
"""

import re
from typing import Tuple
from .models import TransactionType, CreditCategory, DebitCategory


# =============================================================================
# CATEGORY PATTERNS
# =============================================================================

# Each pattern: (compiled_regex, category, confidence)
# Patterns are checked in order - first match wins

DEBIT_PATTERNS: list[Tuple[re.Pattern, str, float]] = [
    # === Subscriptions ===
    (re.compile(r'APPLE\.COM/BILL', re.I), DebitCategory.SUBSCRIPTION.value, 0.95),
    (re.compile(r'Google One', re.I), DebitCategory.SUBSCRIPTION.value, 0.95),
    (re.compile(r'AMAZON PRIME', re.I), DebitCategory.SUBSCRIPTION.value, 0.95),
    (re.compile(r'COBRO DE MEMBRESIA', re.I), DebitCategory.SUBSCRIPTION.value, 0.9),
    (re.compile(r'NETFLIX', re.I), DebitCategory.SUBSCRIPTION.value, 0.95),
    (re.compile(r'SPOTIFY', re.I), DebitCategory.SUBSCRIPTION.value, 0.95),
    (re.compile(r'DISNEY\+', re.I), DebitCategory.SUBSCRIPTION.value, 0.95),

    # === Transport ===
    (re.compile(r'UBER RIDES', re.I), DebitCategory.TRANSPORT.value, 0.95),
    (re.compile(r'DL Uber', re.I), DebitCategory.TRANSPORT.value, 0.95),
    (re.compile(r'SISTEMA DE TRANSPORTE', re.I), DebitCategory.TRANSPORT.value, 0.95),
    (re.compile(r'CABIFY', re.I), DebitCategory.TRANSPORT.value, 0.95),
    (re.compile(r'DIDI', re.I), DebitCategory.TRANSPORT.value, 0.95),

    # === Groceries ===
    (re.compile(r'SUPER\s*99', re.I), DebitCategory.GROCERIES.value, 0.95),
    (re.compile(r'SUPER\s*XTRA', re.I), DebitCategory.GROCERIES.value, 0.95),
    (re.compile(r'EL MACHETAZO', re.I), DebitCategory.GROCERIES.value, 0.95),
    (re.compile(r'SUPER\s*7', re.I), DebitCategory.GROCERIES.value, 0.9),
    (re.compile(r'MINI SUPER', re.I), DebitCategory.GROCERIES.value, 0.85),
    (re.compile(r'REY\s', re.I), DebitCategory.GROCERIES.value, 0.85),
    (re.compile(r'RIBA SMITH', re.I), DebitCategory.GROCERIES.value, 0.9),

    # === Food & Dining ===
    (re.compile(r'RESTAURANTE', re.I), DebitCategory.FOOD_DINING.value, 0.9),
    (re.compile(r'BEERMARKT', re.I), DebitCategory.FOOD_DINING.value, 0.9),
    (re.compile(r'PANDA HOUSE', re.I), DebitCategory.FOOD_DINING.value, 0.9),
    (re.compile(r'MCDON', re.I), DebitCategory.FOOD_DINING.value, 0.95),
    (re.compile(r'STARBUCKS', re.I), DebitCategory.FOOD_DINING.value, 0.95),
    (re.compile(r'KFC', re.I), DebitCategory.FOOD_DINING.value, 0.95),
    (re.compile(r'WENDYS', re.I), DebitCategory.FOOD_DINING.value, 0.95),
    (re.compile(r'DOMINOS', re.I), DebitCategory.FOOD_DINING.value, 0.95),

    # === Utilities ===
    (re.compile(r'NATURGY', re.I), DebitCategory.UTILITIES.value, 0.95),
    (re.compile(r'EDEMET', re.I), DebitCategory.UTILITIES.value, 0.95),
    (re.compile(r'EDECHI', re.I), DebitCategory.UTILITIES.value, 0.95),
    (re.compile(r'CABLE AND WIRELESS', re.I), DebitCategory.UTILITIES.value, 0.95),
    (re.compile(r'IDAAN', re.I), DebitCategory.UTILITIES.value, 0.95),
    (re.compile(r'RECARGA TELEFONIA', re.I), DebitCategory.UTILITIES.value, 0.9),

    # === Health ===
    (re.compile(r'FARMACIA', re.I), DebitCategory.HEALTH.value, 0.9),
    (re.compile(r'ARROCHA', re.I), DebitCategory.HEALTH.value, 0.85),
    (re.compile(r'HOSPITAL', re.I), DebitCategory.HEALTH.value, 0.9),
    (re.compile(r'CLINICA', re.I), DebitCategory.HEALTH.value, 0.9),

    # === Online Shopping ===
    (re.compile(r'AMAZON MKTPL', re.I), DebitCategory.ONLINE_SHOPPING.value, 0.95),
    (re.compile(r'AMAZON RETA', re.I), DebitCategory.ONLINE_SHOPPING.value, 0.95),
    (re.compile(r'MERCADOLIBRE', re.I), DebitCategory.ONLINE_SHOPPING.value, 0.95),
    (re.compile(r'EBAY', re.I), DebitCategory.ONLINE_SHOPPING.value, 0.95),

    # === Shopping (Physical) ===
    (re.compile(r'FACTORY FASHION', re.I), DebitCategory.SHOPPING.value, 0.85),
    (re.compile(r'ALMACEN', re.I), DebitCategory.SHOPPING.value, 0.7),
    (re.compile(r'LA OCA', re.I), DebitCategory.SHOPPING.value, 0.8),

    # === Bank Fees ===
    (re.compile(r'ITBMS', re.I), DebitCategory.BANK_FEE.value, 0.9),
    (re.compile(r'IMPUESTO.*FRAUDE', re.I), DebitCategory.BANK_FEE.value, 0.95),
    (re.compile(r'SEGURO DE FRAUDE', re.I), DebitCategory.BANK_FEE.value, 0.95),
    (re.compile(r'COMISION', re.I), DebitCategory.BANK_FEE.value, 0.9),

    # === ATM ===
    (re.compile(r'ATM-', re.I), DebitCategory.ATM_WITHDRAWAL.value, 0.95),

    # === P2P Payments ===
    (re.compile(r'YAPPY\s+A\s+', re.I), DebitCategory.P2P_PAYMENT.value, 0.95),
    (re.compile(r'PAGO YAPPY', re.I), DebitCategory.P2P_PAYMENT.value, 0.95),

    # === Internal Transfers (OUT) ===
    (re.compile(r'TRANSFERENCIA\s+A\s+.*ENTRE CUENTAS', re.I), DebitCategory.TRANSFER_OUT.value, 0.99),
]

CREDIT_PATTERNS: list[Tuple[re.Pattern, str, float]] = [
    # === Internal Transfers (IN) ===
    (re.compile(r'TRANSFERENCIA\s+DE\s+.*ENTRE CUENTAS', re.I), CreditCategory.TRANSFER_IN.value, 0.99),

    # === P2P Received ===
    (re.compile(r'YAPPY\s+OB\s+DE', re.I), CreditCategory.GIFT.value, 0.7),  # could be income or gift

    # === Interest ===
    (re.compile(r'INTERES\s+CUENTA', re.I), CreditCategory.INTEREST.value, 0.99),

    # === Salary patterns (common) ===
    (re.compile(r'NOMINA', re.I), CreditCategory.INCOME.value, 0.95),
    (re.compile(r'SALARIO', re.I), CreditCategory.INCOME.value, 0.95),
    (re.compile(r'PAYROLL', re.I), CreditCategory.INCOME.value, 0.95),
]


# =============================================================================
# CATEGORIZER FUNCTION
# =============================================================================

def categorize_transaction(
    memo: str,
    amount_raw: float
) -> Tuple[TransactionType, str, float]:
    """
    Categorize a transaction based on its MEMO and amount sign.

    Args:
        memo: The MEMO field from OFX
        amount_raw: The signed amount (negative = debit, positive = credit)

    Returns:
        Tuple of (TransactionType, category_string, confidence)
    """

    # Determine type from sign
    if amount_raw >= 0:
        tx_type = TransactionType.CREDIT
        patterns = CREDIT_PATTERNS
        default_category = CreditCategory.OTHER_CREDIT.value
    else:
        tx_type = TransactionType.DEBIT
        patterns = DEBIT_PATTERNS
        default_category = DebitCategory.OTHER_EXPENSE.value

    # Try to match patterns
    for pattern, category, confidence in patterns:
        if pattern.search(memo):
            return tx_type, category, confidence

    # No match - return default with low confidence
    return tx_type, default_category, 0.3


# =============================================================================
# VENDOR CLEANING
# =============================================================================

# Pattern to extract card mask from end of MEMO
CARD_MASK_PATTERN = re.compile(r'-(\d{4}-\d{2}XX-XXXX-\d{4})$')

def clean_vendor(memo: str) -> Tuple[str, str | None, str]:
    """
    Clean the vendor name from MEMO field.

    Args:
        memo: Raw MEMO from OFX

    Returns:
        Tuple of (vendor_clean, card_mask, vendor_normalized)
    """
    vendor_clean = memo.strip()
    card_mask = None

    # Extract card mask if present
    match = CARD_MASK_PATTERN.search(vendor_clean)
    if match:
        card_mask = match.group(1)
        vendor_clean = vendor_clean[:match.start()].strip()

    # Remove trailing hyphens
    vendor_clean = vendor_clean.rstrip('-').strip()

    # Normalize: title case, clean up
    vendor_normalized = vendor_clean.title()

    # Special normalizations
    normalizations = {
        'Uber Rides': 'Uber',
        'Dl Uber Rides': 'Uber',
        'Apple.Com/Bill': 'Apple',
        'Google One': 'Google One',
        'Super Xtra Transismica': 'Super Xtra',
        'Super 99 Tumba Muerto': 'Super 99',
    }

    for pattern, replacement in normalizations.items():
        if pattern.lower() in vendor_normalized.lower():
            vendor_normalized = replacement
            break

    return vendor_clean, card_mask, vendor_normalized


# =============================================================================
# RECURRING DETECTION
# =============================================================================

# Known recurring vendors/patterns
RECURRING_PATTERNS = [
    re.compile(r'APPLE\.COM/BILL', re.I),
    re.compile(r'Google One', re.I),
    re.compile(r'AMAZON PRIME', re.I),
    re.compile(r'NETFLIX', re.I),
    re.compile(r'SPOTIFY', re.I),
    re.compile(r'COBRO DE MEMBRESIA', re.I),
    re.compile(r'SEGURO DE FRAUDE MENSU', re.I),
    re.compile(r'NATURGY', re.I),
    re.compile(r'CABLE AND WIRELESS', re.I),
]

def detect_recurring(memo: str) -> bool | None:
    """
    Detect if a transaction is likely recurring.

    Returns True if recurring, False if definitely not, None if uncertain.
    """
    for pattern in RECURRING_PATTERNS:
        if pattern.search(memo):
            return True
    return None  # Uncertain
