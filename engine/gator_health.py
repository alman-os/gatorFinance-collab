"""
gatorFinance - gatorHealth Engine
=================================
The proprietary financial health index.

gatorHealth is a dynamic net-flow index based on:
- Credit inflow vs debit outflow
- Recurrence patterns
- Averages over time windows
- Variance / volatility
- Monthly commitment load
- Trend direction

Formula (from the spec):
    gatorHealth =
        (w1 * normalized_net_flow)
        + (w2 * stability_index)
        + (w3 * trend_coefficient)
        + (w4 * savings_behavior_score)
        - (w5 * volatility_penalty)
        - (w6 * overcommitment_penalty)

The result is scaled to 0-100 for human readability.
"""

from dataclasses import dataclass, field
from datetime import datetime
from typing import Optional
from statistics import mean, stdev
from collections import defaultdict

from parser import TransactionNormalized, TransactionType


# =============================================================================
# CONFIGURATION
# =============================================================================

@dataclass
class GatorHealthConfig:
    """Weights for the gatorHealth formula."""
    # Positive factors
    w_net_flow: float = 0.30          # How much net flow matters
    w_stability: float = 0.20         # Recurring income vs recurring expenses
    w_trend: float = 0.20             # Direction of improvement
    w_savings: float = 0.10           # Savings behavior (future: piggy_savings)

    # Negative factors (penalties)
    w_volatility: float = 0.10        # Penalize chaotic spending
    w_overcommitment: float = 0.10    # Penalize too many fixed costs

    # Scaling
    min_months_for_trend: int = 2     # Need at least 2 months for trend
    volatility_threshold: float = 0.5  # Variance threshold for penalty


# =============================================================================
# MONTHLY SNAPSHOT
# =============================================================================

@dataclass
class MonthlySnapshot:
    """Financial snapshot for a single month."""
    year: int
    month: int

    # Totals
    total_credits: float = 0.0
    total_debits: float = 0.0
    net_flow: float = 0.0

    # Recurring
    recurring_credits: float = 0.0
    recurring_debits: float = 0.0
    stability: float = 0.0  # recurring_credits - recurring_debits

    # Counts
    transaction_count: int = 0
    credit_count: int = 0
    debit_count: int = 0

    # Category breakdown
    category_totals: dict[str, float] = field(default_factory=dict)

    # Computed
    commitment_ratio: float = 0.0  # recurring_debits / total_credits

    def compute(self):
        """Compute derived values."""
        self.net_flow = self.total_credits - self.total_debits
        self.stability = self.recurring_credits - self.recurring_debits

        if self.total_credits > 0:
            self.commitment_ratio = self.recurring_debits / self.total_credits
        else:
            self.commitment_ratio = 1.0  # No income = fully committed

        return self


# =============================================================================
# GATOR HEALTH RESULT
# =============================================================================

@dataclass
class GatorHealthResult:
    """The complete gatorHealth assessment."""

    # Overall score (0-100)
    score: float
    grade: str  # A, B, C, D, F

    # Component scores (0-1 scale, before weighting)
    net_flow_score: float
    stability_score: float
    trend_score: float
    savings_score: float
    volatility_penalty: float
    overcommitment_penalty: float

    # Raw values
    avg_monthly_net_flow: float
    avg_monthly_income: float
    avg_monthly_expenses: float
    trend_direction: str  # "improving", "stable", "declining"
    trend_slope: float
    volatility: float

    # Period analyzed
    months_analyzed: int
    period_start: Optional[datetime]
    period_end: Optional[datetime]

    # Monthly snapshots
    monthly_snapshots: list[MonthlySnapshot] = field(default_factory=list)

    # Best/worst months
    best_month: Optional[MonthlySnapshot] = None
    worst_month: Optional[MonthlySnapshot] = None

    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization."""
        return {
            "score": round(self.score, 1),
            "grade": self.grade,
            "components": {
                "net_flow_score": round(self.net_flow_score, 3),
                "stability_score": round(self.stability_score, 3),
                "trend_score": round(self.trend_score, 3),
                "savings_score": round(self.savings_score, 3),
                "volatility_penalty": round(self.volatility_penalty, 3),
                "overcommitment_penalty": round(self.overcommitment_penalty, 3),
            },
            "metrics": {
                "avg_monthly_net_flow": round(self.avg_monthly_net_flow, 2),
                "avg_monthly_income": round(self.avg_monthly_income, 2),
                "avg_monthly_expenses": round(self.avg_monthly_expenses, 2),
                "trend_direction": self.trend_direction,
                "trend_slope": round(self.trend_slope, 4),
                "volatility": round(self.volatility, 3),
            },
            "period": {
                "months_analyzed": self.months_analyzed,
                "start": self.period_start.isoformat() if self.period_start else None,
                "end": self.period_end.isoformat() if self.period_end else None,
            },
            "best_month": f"{self.best_month.year}-{self.best_month.month:02d}" if self.best_month else None,
            "worst_month": f"{self.worst_month.year}-{self.worst_month.month:02d}" if self.worst_month else None,
        }


# =============================================================================
# HELPER FUNCTIONS
# =============================================================================

def compute_trend_slope(values: list[float]) -> float:
    """
    Compute linear regression slope for trend detection.
    Positive slope = improving, negative = declining.
    """
    if len(values) < 2:
        return 0.0

    n = len(values)
    x_mean = (n - 1) / 2
    y_mean = mean(values)

    numerator = sum((i - x_mean) * (y - y_mean) for i, y in enumerate(values))
    denominator = sum((i - x_mean) ** 2 for i in range(n))

    if denominator == 0:
        return 0.0

    return numerator / denominator


def score_to_grade(score: float) -> str:
    """Convert 0-100 score to letter grade."""
    if score >= 90:
        return "A"
    elif score >= 80:
        return "B"
    elif score >= 70:
        return "C"
    elif score >= 60:
        return "D"
    else:
        return "F"


def sigmoid(x: float, center: float = 0, steepness: float = 1) -> float:
    """Sigmoid function for smooth scoring curves."""
    import math
    try:
        return 1 / (1 + math.exp(-steepness * (x - center)))
    except OverflowError:
        return 0.0 if x < center else 1.0


# =============================================================================
# MAIN ENGINE
# =============================================================================

class GatorHealthEngine:
    """
    The gatorHealth calculation engine.

    Usage:
        engine = GatorHealthEngine()
        result = engine.calculate(transactions)
        print(f"Score: {result.score}, Grade: {result.grade}")
    """

    def __init__(self, config: Optional[GatorHealthConfig] = None):
        self.config = config or GatorHealthConfig()

    def _build_monthly_snapshots(
        self,
        transactions: list[TransactionNormalized]
    ) -> list[MonthlySnapshot]:
        """Group transactions into monthly snapshots."""

        # Group by year-month
        monthly: dict[tuple[int, int], MonthlySnapshot] = {}

        for txn in transactions:
            if txn.is_excluded:
                continue

            key = (txn.date.year, txn.date.month)

            if key not in monthly:
                monthly[key] = MonthlySnapshot(year=key[0], month=key[1])

            snap = monthly[key]
            snap.transaction_count += 1

            if txn.transaction_type == TransactionType.CREDIT:
                snap.total_credits += txn.amount
                snap.credit_count += 1
                if txn.is_recurring:
                    snap.recurring_credits += txn.amount
            else:
                snap.total_debits += txn.amount
                snap.debit_count += 1
                if txn.is_recurring:
                    snap.recurring_debits += txn.amount

            # Category tracking
            cat = txn.category_effective
            snap.category_totals[cat] = snap.category_totals.get(cat, 0) + txn.amount

        # Compute derived values and sort
        snapshots = sorted(monthly.values(), key=lambda s: (s.year, s.month))
        for snap in snapshots:
            snap.compute()

        return snapshots

    def calculate(
        self,
        transactions: list[TransactionNormalized]
    ) -> GatorHealthResult:
        """
        Calculate gatorHealth score from transactions.

        The formula:
            score = (w1 * net_flow_score)
                  + (w2 * stability_score)
                  + (w3 * trend_score)
                  + (w4 * savings_score)
                  - (w5 * volatility_penalty)
                  - (w6 * overcommitment_penalty)

        All component scores are 0-1, final score scaled to 0-100.
        """

        # Build monthly snapshots
        snapshots = self._build_monthly_snapshots(transactions)

        if not snapshots:
            # No data
            return GatorHealthResult(
                score=50.0,
                grade="C",
                net_flow_score=0.5,
                stability_score=0.5,
                trend_score=0.5,
                savings_score=0.5,
                volatility_penalty=0.0,
                overcommitment_penalty=0.0,
                avg_monthly_net_flow=0.0,
                avg_monthly_income=0.0,
                avg_monthly_expenses=0.0,
                trend_direction="stable",
                trend_slope=0.0,
                volatility=0.0,
                months_analyzed=0,
                period_start=None,
                period_end=None,
            )

        # Extract monthly values
        net_flows = [s.net_flow for s in snapshots]
        incomes = [s.total_credits for s in snapshots]
        expenses = [s.total_debits for s in snapshots]
        stabilities = [s.stability for s in snapshots]
        commitment_ratios = [s.commitment_ratio for s in snapshots]

        # Averages
        avg_net_flow = mean(net_flows)
        avg_income = mean(incomes) if incomes else 0
        avg_expenses = mean(expenses) if expenses else 0

        # =====================================================================
        # COMPONENT 1: Net Flow Score
        # =====================================================================
        # Positive net flow is good, negative is bad
        # Normalized against income
        if avg_income > 0:
            normalized_net_flow = avg_net_flow / avg_income
        else:
            normalized_net_flow = 0.0

        # Use sigmoid to map to 0-1 (center at 0, positive = better)
        net_flow_score = sigmoid(normalized_net_flow, center=0, steepness=5)

        # =====================================================================
        # COMPONENT 2: Stability Score
        # =====================================================================
        # Positive stability (recurring income > recurring expenses) is good
        avg_stability = mean(stabilities) if stabilities else 0

        if avg_income > 0:
            normalized_stability = avg_stability / avg_income
        else:
            normalized_stability = 0.0

        stability_score = sigmoid(normalized_stability, center=0, steepness=5)

        # =====================================================================
        # COMPONENT 3: Trend Score
        # =====================================================================
        # Improving trend is good
        if len(snapshots) >= self.config.min_months_for_trend:
            trend_slope = compute_trend_slope(net_flows)

            # Normalize slope against average income
            if avg_income > 0:
                normalized_slope = trend_slope / avg_income
            else:
                normalized_slope = 0.0

            trend_score = sigmoid(normalized_slope, center=0, steepness=10)

            if trend_slope > 0.01:
                trend_direction = "improving"
            elif trend_slope < -0.01:
                trend_direction = "declining"
            else:
                trend_direction = "stable"
        else:
            trend_slope = 0.0
            trend_score = 0.5  # Neutral
            trend_direction = "insufficient_data"

        # =====================================================================
        # COMPONENT 4: Savings Score
        # =====================================================================
        # For now, based on % of income saved (net positive)
        # Future: will incorporate piggy_savings
        if avg_income > 0 and avg_net_flow > 0:
            savings_rate = avg_net_flow / avg_income
            savings_score = min(savings_rate * 2, 1.0)  # 50% savings = perfect score
        else:
            savings_score = 0.0

        # =====================================================================
        # COMPONENT 5: Volatility Penalty
        # =====================================================================
        # High variance in expenses = penalty
        if len(expenses) > 1:
            try:
                expense_stdev = stdev(expenses)
                volatility = expense_stdev / avg_expenses if avg_expenses > 0 else 0
            except:
                volatility = 0.0
        else:
            volatility = 0.0

        # Penalty kicks in above threshold
        if volatility > self.config.volatility_threshold:
            volatility_penalty = min((volatility - self.config.volatility_threshold) * 2, 1.0)
        else:
            volatility_penalty = 0.0

        # =====================================================================
        # COMPONENT 6: Overcommitment Penalty
        # =====================================================================
        # If too much income goes to recurring expenses
        avg_commitment = mean(commitment_ratios) if commitment_ratios else 0

        # Penalty if more than 50% of income is committed
        if avg_commitment > 0.5:
            overcommitment_penalty = min((avg_commitment - 0.5) * 2, 1.0)
        else:
            overcommitment_penalty = 0.0

        # =====================================================================
        # FINAL CALCULATION
        # =====================================================================
        raw_score = (
            self.config.w_net_flow * net_flow_score
            + self.config.w_stability * stability_score
            + self.config.w_trend * trend_score
            + self.config.w_savings * savings_score
            - self.config.w_volatility * volatility_penalty
            - self.config.w_overcommitment * overcommitment_penalty
        )

        # Scale to 0-100
        # raw_score is roughly in [-0.2, 0.8] range, normalize
        final_score = max(0, min(100, (raw_score + 0.2) * 100))

        # Find best/worst months
        best_month = max(snapshots, key=lambda s: s.net_flow)
        worst_month = min(snapshots, key=lambda s: s.net_flow)

        return GatorHealthResult(
            score=final_score,
            grade=score_to_grade(final_score),
            net_flow_score=net_flow_score,
            stability_score=stability_score,
            trend_score=trend_score,
            savings_score=savings_score,
            volatility_penalty=volatility_penalty,
            overcommitment_penalty=overcommitment_penalty,
            avg_monthly_net_flow=avg_net_flow,
            avg_monthly_income=avg_income,
            avg_monthly_expenses=avg_expenses,
            trend_direction=trend_direction,
            trend_slope=trend_slope,
            volatility=volatility,
            months_analyzed=len(snapshots),
            period_start=datetime(snapshots[0].year, snapshots[0].month, 1),
            period_end=datetime(snapshots[-1].year, snapshots[-1].month, 1),
            monthly_snapshots=snapshots,
            best_month=best_month,
            worst_month=worst_month,
        )


# =============================================================================
# CONVENIENCE
# =============================================================================

def calculate_gator_health(
    transactions: list[TransactionNormalized],
    config: Optional[GatorHealthConfig] = None,
) -> GatorHealthResult:
    """Calculate gatorHealth for a list of transactions."""
    engine = GatorHealthEngine(config)
    return engine.calculate(transactions)
