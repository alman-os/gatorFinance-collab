#!/usr/bin/env python3
"""
gatorFinance CLI (v0.3)
=======================
Command-line interface for the complete gatorFinance system.

Commands:
    parse     - Parse OFX to JSON (raw + normalized)
    import    - Import OFX into database
    status    - Show database status
    health    - Calculate gatorHealth score
    override  - Set user category/vendor overrides

Usage:
    python main.py import <input.ofx> [--db <path>]
    python main.py status [--db <path>]
    python main.py health [--db <path>]
"""

import argparse
import json
import sys
from pathlib import Path
from datetime import datetime

from parser import (
    parse_ofx_to_raw,
    normalize_statement,
    NormalizedStatement,
    RawStatement,
    check_duplicates,
)
from store import open_database, GatorDatabase
from engine import calculate_gator_health, GatorHealthResult


# Default database path (internal storage)
DEFAULT_DB_PATH = Path.home() / ".gatorfinance" / "db"

# Default folder for user-facing exports (AOS convention: apps write user
# outputs to ~/Documents/AOS/$AppName/ so a future composite tool can parse
# them all under one root)
AOS_OUTPUT_DIR = Path.home() / "Documents" / "AOS" / "gatorFinance"


def format_currency(amount: float, currency: str = "USD") -> str:
    """Format amount as currency string."""
    if currency == "USD":
        return f"${amount:,.2f}"
    return f"{amount:,.2f} {currency}"


def print_summary(statement: NormalizedStatement):
    """Print a human-readable summary of the statement."""
    print("\n" + "=" * 60)
    print("🐊 GATORFINANCE v0.3 - STATEMENT SUMMARY")
    print("=" * 60)

    print(f"\n📁 Source: {statement.source_file}")
    print(f"🏦 Bank: {statement.bank_id or 'Unknown'}")
    print(f"💳 Account: {statement.account_id or 'Unknown'} ({statement.account_type or 'Unknown'})")
    print(f"💰 Currency: {statement.currency}")

    if statement.period_start and statement.period_end:
        print(f"📅 Period: {statement.period_start.date()} → {statement.period_end.date()}")

    if statement.ledger_balance is not None:
        print(f"💵 Ending Balance: {format_currency(statement.ledger_balance, statement.currency)}")

    print(f"\n📊 TRANSACTION STATS")
    print("-" * 40)
    print(f"   Total Transactions: {statement.transaction_count}")
    print(f"   ✅ Credits (in):    {format_currency(statement.total_credits, statement.currency)}")
    print(f"   ❌ Debits (out):    {format_currency(statement.total_debits, statement.currency)}")
    print(f"   📈 Net Flow:        {format_currency(statement.net_flow, statement.currency)}")

    # Category breakdown
    print(f"\n📂 CATEGORY BREAKDOWN")
    print("-" * 40)

    category_totals: dict[str, float] = {}
    category_counts: dict[str, int] = {}

    for tx in statement.transactions:
        cat = tx.category_effective
        category_totals[cat] = category_totals.get(cat, 0) + tx.amount
        category_counts[cat] = category_counts.get(cat, 0) + 1

    sorted_cats = sorted(category_totals.items(), key=lambda x: x[1], reverse=True)

    for cat, total in sorted_cats:
        count = category_counts[cat]
        print(f"   {cat:20s} | {count:3d} txns | {format_currency(total, statement.currency):>12s}")


def print_health_report(result: GatorHealthResult):
    """Print the gatorHealth report."""
    print("\n" + "=" * 60)
    print("🐊 GATORHEALTH REPORT")
    print("=" * 60)

    # Big score display
    grade_colors = {
        "A": "🟢", "B": "🟡", "C": "🟠", "D": "🔴", "F": "⚫"
    }
    grade_icon = grade_colors.get(result.grade, "⚪")

    print(f"\n   {grade_icon} SCORE: {result.score:.1f} / 100  (Grade: {result.grade})")

    # Period
    if result.period_start and result.period_end:
        print(f"\n   📅 Period: {result.period_start.strftime('%b %Y')} → {result.period_end.strftime('%b %Y')}")
        print(f"   📊 Months Analyzed: {result.months_analyzed}")

    # Key metrics
    print(f"\n💰 KEY METRICS")
    print("-" * 40)
    print(f"   Avg Monthly Income:   {format_currency(result.avg_monthly_income)}")
    print(f"   Avg Monthly Expenses: {format_currency(result.avg_monthly_expenses)}")
    print(f"   Avg Monthly Net Flow: {format_currency(result.avg_monthly_net_flow)}")
    print(f"   Trend: {result.trend_direction.replace('_', ' ').title()}")

    # Component breakdown
    print(f"\n📊 SCORE COMPONENTS")
    print("-" * 40)
    print(f"   Net Flow Score:      {result.net_flow_score:.1%} (+)")
    print(f"   Stability Score:     {result.stability_score:.1%} (+)")
    print(f"   Trend Score:         {result.trend_score:.1%} (+)")
    print(f"   Savings Score:       {result.savings_score:.1%} (+)")
    print(f"   Volatility Penalty:  {result.volatility_penalty:.1%} (-)")
    print(f"   Overcommit Penalty:  {result.overcommitment_penalty:.1%} (-)")

    # Best/worst months
    if result.best_month and result.worst_month:
        print(f"\n📅 NOTABLE MONTHS")
        print("-" * 40)
        print(f"   Best:  {result.best_month.year}-{result.best_month.month:02d} ({format_currency(result.best_month.net_flow)} net)")
        print(f"   Worst: {result.worst_month.year}-{result.worst_month.month:02d} ({format_currency(result.worst_month.net_flow)} net)")

    # Advice based on score
    print(f"\n💡 INSIGHTS")
    print("-" * 40)

    if result.grade == "A":
        print("   Excellent financial health! Keep it up.")
    elif result.grade == "B":
        print("   Good financial health with room for improvement.")
    elif result.grade == "C":
        print("   Average financial health. Consider reducing expenses.")
    elif result.grade == "D":
        print("   Below average. Focus on increasing income or cutting costs.")
    else:
        print("   Needs attention. Consider reviewing your budget urgently.")

    if result.volatility_penalty > 0.3:
        print("   ⚠️  High spending volatility detected. Try to stabilize expenses.")

    if result.overcommitment_penalty > 0.3:
        print("   ⚠️  High commitment ratio. Too much income goes to fixed costs.")

    if result.trend_direction == "declining":
        print("   ⚠️  Declining trend. Your financial situation is getting worse.")
    elif result.trend_direction == "improving":
        print("   ✅ Improving trend! You're on the right track.")

    print("\n" + "=" * 60 + "\n")


# =============================================================================
# COMMANDS
# =============================================================================

def cmd_parse(args):
    """Parse an OFX file and save both raw and normalized JSON."""
    input_path = Path(args.input)

    if not input_path.exists():
        print(f"❌ Error: File not found: {input_path}")
        sys.exit(1)

    print(f"🔄 Parsing: {input_path}")

    try:
        raw_statement = parse_ofx_to_raw(input_path)
    except Exception as e:
        print(f"❌ Error parsing file: {e}")
        sys.exit(1)

    print(f"✅ Stage 1: Parsed {raw_statement.transaction_count} raw transactions")

    normalized_statement = normalize_statement(raw_statement)
    print(f"✅ Stage 2: Normalized {normalized_statement.transaction_count} transactions")

    if args.output_dir:
        output_dir = Path(args.output_dir)
    else:
        # Default next to the input would drop sensitive JSON into the
        # statements folder — use the AOS output library instead.
        output_dir = AOS_OUTPUT_DIR / "parsed"

    output_dir.mkdir(parents=True, exist_ok=True)

    base_name = input_path.stem
    raw_output = output_dir / f"{base_name}.raw.json"
    norm_output = output_dir / f"{base_name}.normalized.json"

    raw_data = raw_statement.model_dump(mode='json')
    norm_data = normalized_statement.model_dump(mode='json')

    raw_output.write_text(json.dumps(raw_data, indent=2, default=str))
    norm_output.write_text(json.dumps(norm_data, indent=2, default=str))

    print(f"💾 Raw saved to: {raw_output}")
    print(f"💾 Normalized saved to: {norm_output}")

    if args.summary:
        print_summary(normalized_statement)


def cmd_import(args):
    """Import OFX file(s) into the database."""
    db_path = Path(args.db) if args.db else DEFAULT_DB_PATH

    # Determine what to import
    if args.all:
        # Import all .ofx files from the statements folder
        statements_dir = Path(args.statements_dir) if args.statements_dir else Path(__file__).parent / "data" / "statements"

        if not statements_dir.exists():
            print(f"❌ Statements folder not found: {statements_dir}")
            sys.exit(1)

        ofx_files = sorted(statements_dir.glob("*.ofx"))

        if not ofx_files:
            print(f"❌ No .ofx files found in: {statements_dir}")
            sys.exit(1)

        print(f"📂 Found {len(ofx_files)} statement(s) in: {statements_dir}")

    elif args.input:
        # Single file import
        input_path = Path(args.input)
        if not input_path.exists():
            print(f"❌ Error: File not found: {input_path}")
            sys.exit(1)
        ofx_files = [input_path]
    else:
        print("❌ Error: Provide either a file path or use --all")
        sys.exit(1)

    # Open database
    print(f"📂 Database: {db_path}")
    db = open_database(db_path)

    # Track totals across all files
    total_files = len(ofx_files)
    total_new = 0
    total_duplicates = 0
    successful_imports = 0
    failed_imports = []

    print(f"\n{'=' * 60}")
    print(f"🐊 BATCH IMPORT: {total_files} file(s)")
    print(f"{'=' * 60}\n")

    for i, ofx_file in enumerate(ofx_files, 1):
        print(f"[{i}/{total_files}] 🔄 {ofx_file.name}...", end=" ")

        try:
            raw_statement = parse_ofx_to_raw(ofx_file)
            result = db.import_raw_statement(raw_statement)

            total_new += result.new_transactions
            total_duplicates += result.duplicate_transactions
            successful_imports += 1

            if result.new_transactions > 0:
                print(f"✅ {result.new_transactions} new, {result.duplicate_transactions} dups")
            else:
                print(f"⚠️  all {result.duplicate_transactions} duplicates")

        except Exception as e:
            print(f"❌ Error: {e}")
            failed_imports.append((ofx_file.name, str(e)))

    # Summary
    print(f"\n{'=' * 60}")
    print(f"📥 IMPORT SUMMARY")
    print(f"{'=' * 60}")
    print(f"   Files processed:    {successful_imports}/{total_files}")
    print(f"   ✅ New transactions: {total_new}")
    print(f"   ⚠️  Duplicates:      {total_duplicates}")
    print(f"   📊 Total in DB:      {db.get_transaction_count()}")

    if failed_imports:
        print(f"\n❌ FAILED IMPORTS:")
        for name, error in failed_imports:
            print(f"   - {name}: {error}")

    print()


def cmd_status(args):
    """Show database status."""
    db_path = Path(args.db) if args.db else DEFAULT_DB_PATH

    if not db_path.exists():
        print(f"❌ Database not found: {db_path}")
        print(f"   Run 'python main.py import <file.ofx>' to create it.")
        sys.exit(1)

    db = open_database(db_path)
    meta = db.get_meta()

    print("\n" + "=" * 60)
    print("🐊 GATORFINANCE DATABASE STATUS")
    print("=" * 60)

    print(f"\n📂 Location: {db_path}")
    print(f"📅 Created: {meta.created_at.strftime('%Y-%m-%d %H:%M')}")

    if meta.last_import_at:
        print(f"📥 Last Import: {meta.last_import_at.strftime('%Y-%m-%d %H:%M')}")

    print(f"\n📊 STATS")
    print("-" * 40)
    print(f"   Total Transactions: {meta.total_transactions}")
    print(f"   Total Imports: {meta.total_imports}")
    print(f"   Accounts: {', '.join(meta.accounts) if meta.accounts else 'None'}")

    if meta.import_history:
        print(f"\n📜 RECENT IMPORTS")
        print("-" * 40)
        for imp in meta.import_history[-5:]:
            print(f"   {imp.import_id}: {imp.source_file} ({imp.transactions_new} new, {imp.transactions_duplicate} dups)")

    print("\n" + "=" * 60 + "\n")


def cmd_health(args):
    """Calculate and display gatorHealth score."""
    db_path = Path(args.db) if args.db else DEFAULT_DB_PATH

    if not db_path.exists():
        print(f"❌ Database not found: {db_path}")
        print(f"   Run 'python main.py import <file.ofx>' first.")
        sys.exit(1)

    db = open_database(db_path)
    transactions = db.get_all_transactions()

    if not transactions:
        print("❌ No transactions in database. Import some data first.")
        sys.exit(1)

    # Filter by month if specified
    if args.month:
        try:
            # Parse YYYY-MM format
            year, month = map(int, args.month.split('-'))
            transactions = [t for t in transactions if t.date.year == year and t.date.month == month]

            if not transactions:
                print(f"❌ No transactions found for {args.month}")
                sys.exit(1)

            print(f"🔄 Calculating gatorHealth for {args.month} ({len(transactions)} transactions)...")
        except ValueError:
            print(f"❌ Invalid month format. Use YYYY-MM (e.g., 2025-03)")
            sys.exit(1)
    else:
        print(f"🔄 Calculating gatorHealth for {len(transactions)} transactions...")

    result = calculate_gator_health(transactions)

    print_health_report(result)

    # Show monthly breakdown if analyzing full period
    if args.breakdown and not args.month:
        print_monthly_breakdown(result)

    # Optionally save to JSON
    if args.output:
        output_path = Path(args.output)
        output_path.write_text(json.dumps(result.to_dict(), indent=2))
        print(f"💾 Report saved to: {output_path}")


def print_monthly_breakdown(result: GatorHealthResult):
    """Print month-by-month breakdown."""
    print(f"\n{'=' * 60}")
    print(f"📅 MONTHLY BREAKDOWN")
    print(f"{'=' * 60}\n")

    print(f"   {'Month':<10} | {'Income':>12} | {'Expenses':>12} | {'Net Flow':>12} | {'Status'}")
    print(f"   {'-' * 10}-+-{'-' * 12}-+-{'-' * 12}-+-{'-' * 12}-+-{'-' * 8}")

    for snap in result.monthly_snapshots:
        month_str = f"{snap.year}-{snap.month:02d}"
        status = "✅" if snap.net_flow >= 0 else "❌"

        print(f"   {month_str:<10} | ${snap.total_credits:>10,.2f} | ${snap.total_debits:>10,.2f} | ${snap.net_flow:>10,.2f} | {status}")

    print(f"\n   {'AVERAGE':<10} | ${result.avg_monthly_income:>10,.2f} | ${result.avg_monthly_expenses:>10,.2f} | ${result.avg_monthly_net_flow:>10,.2f} |")
    print()


def cmd_summary(args):
    """Show summary of an OFX file without saving."""
    input_path = Path(args.input)

    if not input_path.exists():
        print(f"❌ Error: File not found: {input_path}")
        sys.exit(1)

    try:
        raw_statement = parse_ofx_to_raw(input_path)
        normalized_statement = normalize_statement(raw_statement)
    except Exception as e:
        print(f"❌ Error parsing file: {e}")
        sys.exit(1)

    print_summary(normalized_statement)


def cmd_categories(args):
    """Show spending breakdown by category."""
    db_path = Path(args.db) if args.db else DEFAULT_DB_PATH

    if not db_path.exists():
        print(f"❌ Database not found: {db_path}")
        sys.exit(1)

    db = open_database(db_path)
    transactions = db.get_all_transactions()

    if not transactions:
        print("❌ No transactions in database.")
        sys.exit(1)

    # Filter by month if specified
    if args.month:
        try:
            year, month = map(int, args.month.split('-'))
            transactions = [t for t in transactions if t.date.year == year and t.date.month == month]
            period_label = args.month
        except ValueError:
            print(f"❌ Invalid month format. Use YYYY-MM")
            sys.exit(1)
    else:
        period_label = "All Time"

    print(f"\n{'=' * 60}")
    print(f"🐊 CATEGORY BREAKDOWN ({period_label})")
    print(f"{'=' * 60}\n")

    # Separate credits and debits
    credits = [t for t in transactions if t.transaction_type.value == "credit"]
    debits = [t for t in transactions if t.transaction_type.value == "debit"]

    # Credit categories
    print(f"📥 INCOME ({len(credits)} transactions)")
    print("-" * 50)

    credit_cats: dict[str, tuple[float, int]] = {}
    for t in credits:
        cat = t.category_effective
        amt, cnt = credit_cats.get(cat, (0.0, 0))
        credit_cats[cat] = (amt + t.amount, cnt + 1)

    for cat, (total, count) in sorted(credit_cats.items(), key=lambda x: x[1][0], reverse=True):
        print(f"   {cat:<25} | {count:>4} txns | ${total:>12,.2f}")

    total_credits = sum(t.amount for t in credits)
    print(f"   {'TOTAL':<25} | {len(credits):>4} txns | ${total_credits:>12,.2f}")

    # Debit categories
    print(f"\n📤 EXPENSES ({len(debits)} transactions)")
    print("-" * 50)

    debit_cats: dict[str, tuple[float, int]] = {}
    for t in debits:
        cat = t.category_effective
        amt, cnt = debit_cats.get(cat, (0.0, 0))
        debit_cats[cat] = (amt + t.amount, cnt + 1)

    for cat, (total, count) in sorted(debit_cats.items(), key=lambda x: x[1][0], reverse=True):
        pct = (total / sum(t.amount for t in debits)) * 100 if debits else 0
        print(f"   {cat:<25} | {count:>4} txns | ${total:>12,.2f} | {pct:>5.1f}%")

    total_debits = sum(t.amount for t in debits)
    print(f"   {'TOTAL':<25} | {len(debits):>4} txns | ${total_debits:>12,.2f} | 100.0%")

    # Top vendors
    if args.vendors:
        print(f"\n🏪 TOP VENDORS (by spending)")
        print("-" * 50)

        vendor_totals: dict[str, float] = {}
        for t in debits:
            vendor = t.vendor_effective
            vendor_totals[vendor] = vendor_totals.get(vendor, 0) + t.amount

        for vendor, total in sorted(vendor_totals.items(), key=lambda x: x[1], reverse=True)[:15]:
            pct = (total / total_debits) * 100
            print(f"   {vendor[:35]:<35} | ${total:>10,.2f} | {pct:>5.1f}%")

    print()


# =============================================================================
# MAIN
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="🐊 gatorFinance v0.3 - Personal Finance Intelligence",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    subparsers = parser.add_subparsers(dest='command', help='Commands')

    # Parse command
    parse_parser = subparsers.add_parser('parse', help='Parse OFX file to Raw + Normalized JSON')
    parse_parser.add_argument('input', help='Input OFX file path')
    parse_parser.add_argument('-o', '--output-dir', help='Output directory (default: ~/Documents/AOS/gatorFinance/parsed)')
    parse_parser.add_argument('-s', '--summary', action='store_true', help='Also print summary')
    parse_parser.set_defaults(func=cmd_parse)

    # Import command
    import_parser = subparsers.add_parser('import', help='Import OFX file(s) into database')
    import_parser.add_argument('input', nargs='?', help='Input OFX file path (optional if using --all)')
    import_parser.add_argument('--all', action='store_true', help='Import all .ofx files from statements folder')
    import_parser.add_argument('--statements-dir', help='Statements folder (default: ./data/statements)')
    import_parser.add_argument('--db', help=f'Database path (default: {DEFAULT_DB_PATH})')
    import_parser.set_defaults(func=cmd_import)

    # Status command
    status_parser = subparsers.add_parser('status', help='Show database status')
    status_parser.add_argument('--db', help=f'Database path (default: {DEFAULT_DB_PATH})')
    status_parser.set_defaults(func=cmd_status)

    # Health command
    health_parser = subparsers.add_parser('health', help='Calculate gatorHealth score')
    health_parser.add_argument('--db', help=f'Database path (default: {DEFAULT_DB_PATH})')
    health_parser.add_argument('--month', help='Analyze specific month (YYYY-MM format, e.g., 2025-03)')
    health_parser.add_argument('--breakdown', action='store_true', help='Show month-by-month breakdown')
    health_parser.add_argument('-o', '--output', help='Save report to JSON file')
    health_parser.set_defaults(func=cmd_health)

    # Summary command
    summary_parser = subparsers.add_parser('summary', help='Show statement summary')
    summary_parser.add_argument('input', help='Input OFX file path')
    summary_parser.set_defaults(func=cmd_summary)

    # Categories command
    categories_parser = subparsers.add_parser('categories', help='Show spending by category')
    categories_parser.add_argument('--db', help=f'Database path (default: {DEFAULT_DB_PATH})')
    categories_parser.add_argument('--month', help='Filter by month (YYYY-MM format)')
    categories_parser.add_argument('--vendors', action='store_true', help='Also show top vendors')
    categories_parser.set_defaults(func=cmd_categories)

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(1)

    args.func(args)


if __name__ == '__main__':
    main()
