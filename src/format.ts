export function money(minor: number, currency = "USD", locale = "en-US", compact = false) {
  return new Intl.NumberFormat(locale, {
    style: "currency",
    currency,
    maximumFractionDigits: compact ? 0 : 2,
    notation: compact ? "compact" : "standard",
  }).format(minor / 100);
}

export function monthLabel(year: number, month: number, locale = "en-US") {
  return new Intl.DateTimeFormat(locale, { month: "short", year: "2-digit" })
    .format(new Date(year, month - 1, 1))
    .toUpperCase();
}

export function dateLabel(value: string, locale = "en-US") {
  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "2-digit",
    year: "numeric",
  }).format(new Date(value));
}

export function periodLabel(
  start: string | null,
  end: string | null,
  locale = "en-US",
  fallback = "No period",
) {
  if (!start || !end) return fallback;
  const formatter = new Intl.DateTimeFormat(locale, { month: "short", year: "numeric", timeZone: "UTC" });
  return `${formatter.format(new Date(`${start}T00:00:00Z`))} - ${formatter.format(new Date(`${end}T00:00:00Z`))}`;
}

export function scoreTone(score: number) {
  if (score >= 80) return "good";
  if (score >= 65) return "warning";
  return "danger";
}

export function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  return String(error);
}
