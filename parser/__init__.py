"""
gatorFinance Parser Module (v0.2)
=================================
Parse OFX bank statements into structured transaction data.

Architecture:
- TransactionRaw: Immutable, exactly as imported
- TransactionNormalized: Derived, can be re-computed
- RawStatement: Collection of raw imports
- NormalizedStatement: Collection of normalized data

Deduplication:
- Use TransactionRaw.dedupe_hash as unique constraint
- check_duplicates() filters already-seen transactions
"""

from .models import (
    # Enums
    TransactionType,
    CreditCategory,
    DebitCategory,
    # Layer 1: Raw (immutable)
    TransactionRaw,
    RawStatement,
    # Layer 2: Normalized (derived)
    TransactionNormalized,
    NormalizedStatement,
    # Deduplication
    DedupeResult,
    check_duplicates,
    # Legacy aliases
    Transaction,
    ParsedStatement,
)
from .ofx_parser import (
    # Two-stage parsing
    parse_ofx_to_raw,
    normalize_statement,
    normalize_transaction,
    # Convenience (full pipeline)
    parse_ofx_file,
    parse_ofx_string,
)
from .categorizer import (
    categorize_transaction,
    clean_vendor,
    detect_recurring,
)

__all__ = [
    # Enums
    "TransactionType",
    "CreditCategory",
    "DebitCategory",
    # Layer 1: Raw
    "TransactionRaw",
    "RawStatement",
    # Layer 2: Normalized
    "TransactionNormalized",
    "NormalizedStatement",
    # Deduplication
    "DedupeResult",
    "check_duplicates",
    # Two-stage parsing
    "parse_ofx_to_raw",
    "normalize_statement",
    "normalize_transaction",
    # Convenience
    "parse_ofx_file",
    "parse_ofx_string",
    # Categorizer
    "categorize_transaction",
    "clean_vendor",
    "detect_recurring",
    # Legacy
    "Transaction",
    "ParsedStatement",
]
