"""
gatorFinance - JSON Database Store
==================================
Local-first, file-based storage for transactions.

Architecture:
- Raw transactions stored in `raw/` (immutable, one file per import)
- Normalized transactions in `normalized.json` (single file, deduplicated)
- User overrides in `overrides.json` (persists across re-normalization)
- Metadata in `meta.json` (account info, import history)

Deduplication:
- Uses dedupe_hash as unique constraint
- Re-importing same statement = no duplicates
"""

import json
from pathlib import Path
from datetime import datetime
from typing import Optional
from pydantic import BaseModel, Field

from parser import (
    TransactionRaw,
    TransactionNormalized,
    RawStatement,
    NormalizedStatement,
    normalize_transaction,
    check_duplicates,
    DedupeResult,
)


# =============================================================================
# DATABASE MODELS
# =============================================================================

class ImportRecord(BaseModel):
    """Record of a single import operation."""
    import_id: str
    source_file: str
    imported_at: datetime
    transactions_total: int
    transactions_new: int
    transactions_duplicate: int


class UserOverride(BaseModel):
    """User overrides for a transaction (keyed by dedupe_hash)."""
    dedupe_hash: str
    category_user: Optional[str] = None
    vendor_user: Optional[str] = None
    notes_user: Optional[str] = None
    tags_user: list[str] = Field(default_factory=list)
    is_excluded: bool = False
    updated_at: datetime = Field(default_factory=datetime.now)


class DatabaseMeta(BaseModel):
    """Database metadata."""
    created_at: datetime = Field(default_factory=datetime.now)
    last_import_at: Optional[datetime] = None
    total_transactions: int = 0
    total_imports: int = 0
    accounts: list[str] = Field(default_factory=list)
    import_history: list[ImportRecord] = Field(default_factory=list)


# =============================================================================
# GATOR DATABASE
# =============================================================================

class GatorDatabase:
    """
    JSON-based local database for gatorFinance.

    Directory structure:
        {db_path}/
        ├── meta.json           # Database metadata
        ├── raw/                # Immutable raw imports
        │   ├── import_001.json
        │   └── import_002.json
        ├── normalized.json     # All normalized transactions (deduplicated)
        └── overrides.json      # User category/vendor overrides
    """

    def __init__(self, db_path: str | Path):
        self.db_path = Path(db_path)
        self.raw_dir = self.db_path / "raw"
        self.meta_file = self.db_path / "meta.json"
        self.normalized_file = self.db_path / "normalized.json"
        self.overrides_file = self.db_path / "overrides.json"

        # In-memory caches
        self._meta: Optional[DatabaseMeta] = None
        self._normalized: list[TransactionNormalized] = []
        self._overrides: dict[str, UserOverride] = {}
        self._dedupe_hashes: set[str] = set()

    # =========================================================================
    # INITIALIZATION
    # =========================================================================

    def init(self) -> "GatorDatabase":
        """Initialize the database directory structure."""
        self.db_path.mkdir(parents=True, exist_ok=True)
        self.raw_dir.mkdir(exist_ok=True)

        # Initialize meta if not exists
        if not self.meta_file.exists():
            self._meta = DatabaseMeta()
            self._save_meta()

        # Initialize empty normalized if not exists
        if not self.normalized_file.exists():
            self.normalized_file.write_text("[]")

        # Initialize empty overrides if not exists
        if not self.overrides_file.exists():
            self.overrides_file.write_text("{}")

        # Load into memory
        self._load_all()

        return self

    def _load_all(self):
        """Load all data into memory."""
        # Load meta
        if self.meta_file.exists():
            data = json.loads(self.meta_file.read_text())
            self._meta = DatabaseMeta(**data)
        else:
            self._meta = DatabaseMeta()

        # Load normalized transactions
        if self.normalized_file.exists():
            data = json.loads(self.normalized_file.read_text())
            self._normalized = [TransactionNormalized(**t) for t in data]
            self._dedupe_hashes = {t.raw_dedupe_hash for t in self._normalized}

        # Load overrides
        if self.overrides_file.exists():
            data = json.loads(self.overrides_file.read_text())
            self._overrides = {k: UserOverride(**v) for k, v in data.items()}

    def _save_meta(self):
        """Save metadata to disk."""
        self.meta_file.write_text(
            json.dumps(self._meta.model_dump(mode='json'), indent=2, default=str)
        )

    def _save_normalized(self):
        """Save normalized transactions to disk."""
        data = [t.model_dump(mode='json') for t in self._normalized]
        self.normalized_file.write_text(json.dumps(data, indent=2, default=str))

    def _save_overrides(self):
        """Save overrides to disk."""
        data = {k: v.model_dump(mode='json') for k, v in self._overrides.items()}
        self.overrides_file.write_text(json.dumps(data, indent=2, default=str))

    # =========================================================================
    # IMPORT OPERATIONS
    # =========================================================================

    def import_raw_statement(self, raw_statement: RawStatement) -> DedupeResult:
        """
        Import a raw statement into the database.

        - Saves raw to raw/ directory (immutable archive)
        - Deduplicates against existing transactions
        - Normalizes new transactions
        - Applies any existing user overrides
        - Updates metadata

        Returns DedupeResult with import statistics.
        """
        # Check for duplicates
        new_raw_txns, dedupe_result = check_duplicates(
            raw_statement.transactions,
            self._dedupe_hashes
        )

        if not new_raw_txns:
            # All duplicates, nothing to import
            return dedupe_result

        # Generate import ID
        import_id = f"import_{len(self._meta.import_history) + 1:04d}"

        # Save raw statement (immutable archive)
        raw_file = self.raw_dir / f"{import_id}.json"
        raw_data = raw_statement.model_dump(mode='json')
        raw_file.write_text(json.dumps(raw_data, indent=2, default=str))

        # Normalize new transactions
        for raw_txn in new_raw_txns:
            norm_txn = normalize_transaction(raw_txn)
            if norm_txn:
                # Apply user overrides if they exist
                if norm_txn.raw_dedupe_hash in self._overrides:
                    override = self._overrides[norm_txn.raw_dedupe_hash]
                    norm_txn.category_user = override.category_user
                    norm_txn.vendor_user = override.vendor_user
                    norm_txn.notes_user = override.notes_user
                    norm_txn.tags_user = override.tags_user
                    norm_txn.is_excluded = override.is_excluded

                self._normalized.append(norm_txn)
                self._dedupe_hashes.add(norm_txn.raw_dedupe_hash)

        # Sort by date
        self._normalized.sort(key=lambda t: t.date)

        # Update metadata
        import_record = ImportRecord(
            import_id=import_id,
            source_file=raw_statement.source_file,
            imported_at=datetime.now(),
            transactions_total=dedupe_result.total_incoming,
            transactions_new=dedupe_result.new_transactions,
            transactions_duplicate=dedupe_result.duplicate_transactions,
        )
        self._meta.import_history.append(import_record)
        self._meta.last_import_at = datetime.now()
        self._meta.total_transactions = len(self._normalized)
        self._meta.total_imports += 1

        # Track accounts
        if raw_statement.account_id and raw_statement.account_id not in self._meta.accounts:
            self._meta.accounts.append(raw_statement.account_id)

        # Persist
        self._save_normalized()
        self._save_meta()

        return dedupe_result

    # =========================================================================
    # QUERY OPERATIONS
    # =========================================================================

    def get_all_transactions(self) -> list[TransactionNormalized]:
        """Get all normalized transactions."""
        return self._normalized.copy()

    def get_transactions_by_month(self, year: int, month: int) -> list[TransactionNormalized]:
        """Get transactions for a specific month."""
        return [
            t for t in self._normalized
            if t.date.year == year and t.date.month == month
        ]

    def get_transactions_by_category(self, category: str) -> list[TransactionNormalized]:
        """Get transactions by category (uses effective category)."""
        return [
            t for t in self._normalized
            if t.category_effective == category
        ]

    def get_transactions_daterange(
        self,
        start: datetime,
        end: datetime
    ) -> list[TransactionNormalized]:
        """Get transactions within a date range."""
        return [
            t for t in self._normalized
            if start <= t.date <= end
        ]

    def get_meta(self) -> DatabaseMeta:
        """Get database metadata."""
        return self._meta

    def get_transaction_count(self) -> int:
        """Get total transaction count."""
        return len(self._normalized)

    # =========================================================================
    # OVERRIDE OPERATIONS
    # =========================================================================

    def set_override(
        self,
        dedupe_hash: str,
        category: Optional[str] = None,
        vendor: Optional[str] = None,
        notes: Optional[str] = None,
        tags: Optional[list[str]] = None,
        is_excluded: Optional[bool] = None,
    ) -> bool:
        """
        Set user overrides for a transaction.

        Returns True if transaction exists and was updated.
        """
        # Find the transaction
        txn = next((t for t in self._normalized if t.raw_dedupe_hash == dedupe_hash), None)
        if not txn:
            return False

        # Create or update override
        if dedupe_hash not in self._overrides:
            self._overrides[dedupe_hash] = UserOverride(dedupe_hash=dedupe_hash)

        override = self._overrides[dedupe_hash]

        if category is not None:
            override.category_user = category
            txn.category_user = category
        if vendor is not None:
            override.vendor_user = vendor
            txn.vendor_user = vendor
        if notes is not None:
            override.notes_user = notes
            txn.notes_user = notes
        if tags is not None:
            override.tags_user = tags
            txn.tags_user = tags
        if is_excluded is not None:
            override.is_excluded = is_excluded
            txn.is_excluded = is_excluded

        override.updated_at = datetime.now()

        # Persist
        self._save_normalized()
        self._save_overrides()

        return True

    # =========================================================================
    # RE-NORMALIZATION
    # =========================================================================

    def renormalize_all(self) -> int:
        """
        Re-run normalization on all raw data.

        This applies updated categorization rules while preserving user overrides.
        Returns count of transactions re-normalized.
        """
        # Collect all raw transactions from archive
        all_raw: list[TransactionRaw] = []

        for raw_file in sorted(self.raw_dir.glob("*.json")):
            data = json.loads(raw_file.read_text())
            raw_stmt = RawStatement(**data)
            all_raw.extend(raw_stmt.transactions)

        # Re-normalize
        new_normalized = []
        for raw_txn in all_raw:
            norm_txn = normalize_transaction(raw_txn)
            if norm_txn:
                # Re-apply user overrides
                if norm_txn.raw_dedupe_hash in self._overrides:
                    override = self._overrides[norm_txn.raw_dedupe_hash]
                    norm_txn.category_user = override.category_user
                    norm_txn.vendor_user = override.vendor_user
                    norm_txn.notes_user = override.notes_user
                    norm_txn.tags_user = override.tags_user
                    norm_txn.is_excluded = override.is_excluded

                new_normalized.append(norm_txn)

        # Sort and replace
        new_normalized.sort(key=lambda t: t.date)
        self._normalized = new_normalized
        self._dedupe_hashes = {t.raw_dedupe_hash for t in self._normalized}

        # Update meta
        self._meta.total_transactions = len(self._normalized)

        # Persist
        self._save_normalized()
        self._save_meta()

        return len(new_normalized)


# =============================================================================
# CONVENIENCE FUNCTIONS
# =============================================================================

def open_database(db_path: str | Path) -> GatorDatabase:
    """Open (or create) a gatorFinance database."""
    return GatorDatabase(db_path).init()
