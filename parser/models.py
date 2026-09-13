"""
gatorFinance - Transaction Models (v0.2)
========================================
Pydantic schemas for parsed financial transactions.

ARCHITECTURE DECISION:
======================
We explicitly separate two concepts:

1. TransactionRaw — IMMUTABLE
   - Exactly what the bank gave us
   - Never modified after import
   - Source of truth for re-processing

2. TransactionNormalized — DERIVED
   - What the app reasons about
   - Can be re-computed from Raw
   - Includes categorization, vendor cleanup, user overrides
   - gatorHealth operates on this layer

WHY THIS MATTERS:
- Memo cleanup will evolve
- Categorization rules will change
- You can re-run logic without re-importing
- Users may override categories later
- Future ML/heuristics slot in cleanly
- gatorHealth can be recomputed safely
- Deduplication is reliable (based on immutable IDs)
"""

from pydantic import BaseModel, Field, computed_field
from typing import Optional
from datetime import datetime
from enum import Enum
import hashlib


# =============================================================================
# ENUMS - Transaction Categories
# =============================================================================

class TransactionType(str, Enum):
    """Top-level: money in or money out"""
    CREDIT = "credit"
    DEBIT = "debit"


class CreditCategory(str, Enum):
    """Subcategories for credits (money in)"""
    INCOME = "income"              # work-related, usually
    GIFT = "gift"                  # credit without commitment
    LOAN = "loan"                  # credit with commitment
    TRANSFER_IN = "transfer_in"    # internal account transfer
    INTEREST = "interest"          # bank interest
    REFUND = "refund"              # returned money
    OTHER_CREDIT = "other_credit"


class DebitCategory(str, Enum):
    """Subcategories for debits (money out)"""
    # Expenses
    SUBSCRIPTION = "subscription"
    ONLINE_SHOPPING = "online_shopping"
    GROCERIES = "groceries"
    FOOD_DINING = "food_dining"
    TRANSPORT = "transport"
    UTILITIES = "utilities"
    HEALTH = "health"
    SHOPPING = "shopping"
    ENTERTAINMENT = "entertainment"
    BANK_FEE = "bank_fee"
    ATM_WITHDRAWAL = "atm_withdrawal"
    P2P_PAYMENT = "p2p_payment"
    # Repayments
    REPAYMENT = "repayment"        # opposite of loan
    TRANSFER_OUT = "transfer_out"  # internal account transfer
    OTHER_EXPENSE = "other_expense"


# =============================================================================
# LAYER 1: TRANSACTION RAW (IMMUTABLE)
# =============================================================================

class TransactionRaw(BaseModel):
    """
    Raw transaction data exactly as imported from OFX.

    THIS IS IMMUTABLE. Never modify after import.
    This is the source of truth for all derived data.

    The `dedupe_hash` field enables safe re-imports:
    - Same transaction imported twice = same hash
    - DB can use this as unique constraint
    """

    # === Identity (from OFX) ===
    fitid: str = Field(..., description="FITID from OFX - bank's unique transaction ID")
    refnum: Optional[str] = Field(None, description="REFNUM from OFX")

    # === Temporal (from OFX) ===
    dtposted: str = Field(..., description="Original DTPOSTED string exactly as in OFX")

    # === Financial (from OFX) ===
    trnamt: float = Field(..., description="Original TRNAMT (signed: negative=debit, positive=credit)")
    trntype: str = Field(..., description="Original TRNTYPE from OFX")

    # === Vendor (from OFX) ===
    memo: str = Field(..., description="Original MEMO field exactly as in OFX")

    # === Source Context ===
    source_file: str = Field(..., description="Filename this was imported from")
    account_id: Optional[str] = Field(None, description="ACCTID from OFX")
    bank_id: Optional[str] = Field(None, description="BANKID/ORG from OFX")
    currency: str = Field(default="USD", description="CURDEF from OFX")

    # === Import Metadata ===
    imported_at: datetime = Field(default_factory=datetime.now, description="When this was imported")

    @computed_field
    @property
    def dedupe_hash(self) -> str:
        """
        Unique hash for deduplication.

        Composed of: bank_id + account_id + fitid + dtposted + trnamt
        This ensures the same transaction imported twice gets the same hash.

        Use this as a unique constraint in your database.
        """
        components = [
            self.bank_id or "",
            self.account_id or "",
            self.fitid,
            self.dtposted,
            str(self.trnamt),
        ]
        combined = "|".join(components)
        return hashlib.sha256(combined.encode()).hexdigest()[:16]

    @computed_field
    @property
    def unique_id(self) -> str:
        """
        Human-readable unique ID for this transaction.
        Format: {bank_id}-{fitid} or just {fitid} if no bank_id
        """
        if self.bank_id:
            return f"{self.bank_id}-{self.fitid}"
        return self.fitid

    class Config:
        frozen = True  # Makes instances immutable
        json_schema_extra = {
            "example": {
                "fitid": "DEMO-100001",
                "refnum": "DEMO-100001",
                "dtposted": "20260115120000.000",
                "trnamt": -9.5,
                "trntype": "OTHER",
                "memo": "DEMO MARKET-0000-00XX-XXXX-0000",
                "source_file": "sample_statement.ofx",
                "account_id": "00-00-00-000000-0",
                "bank_id": "GATOR",
                "currency": "USD"
            }
        }


# =============================================================================
# LAYER 2: TRANSACTION NORMALIZED (DERIVED)
# =============================================================================

class TransactionNormalized(BaseModel):
    """
    Normalized transaction for app logic and gatorHealth.

    THIS IS DERIVED from TransactionRaw.
    Can be re-computed any time by re-processing the raw data.

    This is what the UI displays and what gatorHealth reasons about.
    """

    # === Link to Raw ===
    raw_dedupe_hash: str = Field(..., description="Links back to TransactionRaw.dedupe_hash")
    unique_id: str = Field(..., description="Human-readable ID from raw")

    # === Parsed Temporal ===
    date: datetime = Field(..., description="Parsed datetime from dtposted")
    date_raw: str = Field(..., description="Original dtposted for reference")

    # === Parsed Financial ===
    amount: float = Field(..., description="Absolute value of transaction")
    amount_signed: float = Field(..., description="Original signed amount")
    currency: str = Field(default="USD")
    transaction_type: TransactionType = Field(..., description="credit or debit (derived from sign)")

    # === Auto-Classification (can be re-run) ===
    category_auto: str = Field(..., description="Auto-detected category")
    category_confidence: float = Field(default=1.0, description="0-1 confidence in auto-categorization")
    is_recurring: Optional[bool] = Field(None, description="Detected as recurring payment")

    # === Cleaned Vendor (can be re-run) ===
    vendor_raw: str = Field(..., description="Original MEMO")
    vendor_clean: str = Field(..., description="Cleaned vendor name (card mask removed)")
    vendor_normalized: Optional[str] = Field(None, description="Normalized vendor name")
    card_mask: Optional[str] = Field(None, description="Extracted card mask if present")

    # === User Overrides (manual, persisted separately) ===
    category_user: Optional[str] = Field(None, description="User-assigned category override")
    vendor_user: Optional[str] = Field(None, description="User-assigned vendor name")
    notes_user: Optional[str] = Field(None, description="User notes")
    tags_user: list[str] = Field(default_factory=list, description="User-assigned tags")
    is_excluded: bool = Field(default=False, description="Exclude from gatorHealth calculations")

    # === Normalization Metadata ===
    normalized_at: datetime = Field(default_factory=datetime.now)
    normalizer_version: str = Field(default="0.2.0", description="Version of normalization logic")

    @computed_field
    @property
    def category_effective(self) -> str:
        """
        The category to use for display and calculations.
        User override takes precedence over auto-detection.
        """
        return self.category_user or self.category_auto

    @computed_field
    @property
    def vendor_effective(self) -> str:
        """
        The vendor name to use for display.
        User override takes precedence.
        """
        return self.vendor_user or self.vendor_normalized or self.vendor_clean

    class Config:
        json_schema_extra = {
            "example": {
                "raw_dedupe_hash": "0123456789abcdef",
                "unique_id": "GATOR-DEMO-100001",
                "date": "2026-01-15T12:00:00",
                "date_raw": "20260115120000.000",
                "amount": 9.5,
                "amount_signed": -9.5,
                "currency": "USD",
                "transaction_type": "debit",
                "category_auto": "food_dining",
                "category_confidence": 0.9,
                "vendor_raw": "DEMO MARKET-0000-00XX-XXXX-0000",
                "vendor_clean": "DEMO MARKET",
                "vendor_normalized": "Demo Market",
                "card_mask": "0000-00XX-XXXX-0000"
            }
        }


# =============================================================================
# STATEMENT MODELS
# =============================================================================

class RawStatement(BaseModel):
    """
    Collection of raw transactions from a single OFX import.
    This is what gets stored as the immutable source.
    """

    # === Source Info ===
    source_file: str = Field(..., description="Original filename")
    imported_at: datetime = Field(default_factory=datetime.now)

    # === Account Info (from OFX header) ===
    bank_id: Optional[str] = Field(None)
    account_id: Optional[str] = Field(None)
    account_type: Optional[str] = Field(None)
    currency: str = Field(default="USD")

    # === Statement Period ===
    period_start: Optional[datetime] = Field(None)
    period_end: Optional[datetime] = Field(None)

    # === Balance ===
    ledger_balance: Optional[float] = Field(None)

    # === Raw Transactions ===
    transactions: list[TransactionRaw] = Field(default_factory=list)

    # === Import Stats ===
    transaction_count: int = Field(default=0)

    def finalize(self) -> "RawStatement":
        """Compute final stats."""
        self.transaction_count = len(self.transactions)
        return self


class NormalizedStatement(BaseModel):
    """
    Collection of normalized transactions ready for the app.
    Derived from RawStatement, can be regenerated.
    """

    # === Link to Source ===
    source_file: str
    raw_import_id: Optional[str] = Field(None, description="ID of the RawStatement this came from")

    # === Account Info ===
    bank_id: Optional[str] = None
    account_id: Optional[str] = None
    account_type: Optional[str] = None
    currency: str = "USD"

    # === Period ===
    period_start: Optional[datetime] = None
    period_end: Optional[datetime] = None

    # === Balance ===
    ledger_balance: Optional[float] = None

    # === Normalized Transactions ===
    transactions: list[TransactionNormalized] = Field(default_factory=list)

    # === Computed Stats ===
    transaction_count: int = 0
    total_credits: float = 0.0
    total_debits: float = 0.0
    net_flow: float = 0.0

    # === Normalization Metadata ===
    normalized_at: datetime = Field(default_factory=datetime.now)
    normalizer_version: str = "0.2.0"

    def compute_stats(self) -> "NormalizedStatement":
        """Compute summary statistics from transactions."""
        self.transaction_count = len(self.transactions)
        self.total_credits = sum(
            t.amount for t in self.transactions
            if t.transaction_type == TransactionType.CREDIT and not t.is_excluded
        )
        self.total_debits = sum(
            t.amount for t in self.transactions
            if t.transaction_type == TransactionType.DEBIT and not t.is_excluded
        )
        self.net_flow = self.total_credits - self.total_debits
        return self


# =============================================================================
# DEDUPLICATION HELPERS
# =============================================================================

class DedupeResult(BaseModel):
    """Result of a deduplication check."""

    total_incoming: int = Field(..., description="Total transactions in the import")
    new_transactions: int = Field(..., description="Transactions not seen before")
    duplicate_transactions: int = Field(..., description="Transactions already in DB")
    duplicate_hashes: list[str] = Field(default_factory=list, description="Hashes of duplicates")

    @computed_field
    @property
    def has_duplicates(self) -> bool:
        return self.duplicate_transactions > 0


def check_duplicates(
    incoming: list[TransactionRaw],
    existing_hashes: set[str]
) -> tuple[list[TransactionRaw], DedupeResult]:
    """
    Check incoming transactions against existing hashes.

    Args:
        incoming: New transactions to check
        existing_hashes: Set of dedupe_hash values already in DB

    Returns:
        Tuple of (new_transactions_only, DedupeResult)
    """
    new_txns = []
    dup_hashes = []

    for txn in incoming:
        if txn.dedupe_hash in existing_hashes:
            dup_hashes.append(txn.dedupe_hash)
        else:
            new_txns.append(txn)

    result = DedupeResult(
        total_incoming=len(incoming),
        new_transactions=len(new_txns),
        duplicate_transactions=len(dup_hashes),
        duplicate_hashes=dup_hashes,
    )

    return new_txns, result


# =============================================================================
# LEGACY COMPATIBILITY (v0.1 -> v0.2 migration)
# =============================================================================

# Alias for backwards compatibility during migration
Transaction = TransactionNormalized
ParsedStatement = NormalizedStatement
