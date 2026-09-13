import { Search, SlidersHorizontal, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { dateLabel, money } from "../format";
import { useI18n } from "../i18n";
import type { TransactionRecord, TransactionUpdate } from "../types";

const CATEGORIES = [
  "income", "gift", "loan", "transfer_in", "interest", "refund", "other_credit",
  "subscription", "online_shopping", "groceries", "food_dining", "transport",
  "utilities", "health", "shopping", "entertainment", "bank_fee", "atm_withdrawal",
  "p2p_payment", "repayment", "transfer_out", "other_expense",
];

interface TransactionsProps {
  transactions: TransactionRecord[];
  currency: string;
  busy: boolean;
  status: string;
  onUpdate: (update: TransactionUpdate) => Promise<void>;
}

type Filter = "all" | "credit" | "debit" | "review";

export function Transactions({ transactions, currency, busy, status, onUpdate }: TransactionsProps) {
  const { L, categoryLabel } = useI18n();
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [selectedHash, setSelectedHash] = useState<string | null>(transactions[0]?.dedupeHash ?? null);
  const selected = transactions.find((item) => item.dedupeHash === selectedHash) ?? null;

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return transactions.filter((item) => {
      if (filter === "credit" && item.transactionType !== "credit") return false;
      if (filter === "debit" && item.transactionType !== "debit") return false;
      if (filter === "review" && item.categoryConfidence >= 0.6) return false;
      if (!needle) return true;
      return [item.vendorUser, item.vendorNormalized, item.categoryUser, item.categoryAuto, categoryLabel(item.categoryUser || item.categoryAuto), item.notesUser]
        .filter(Boolean)
        .some((value) => value!.toLowerCase().includes(needle));
    });
  }, [categoryLabel, filter, search, transactions]);

  useEffect(() => {
    if (selectedHash && !transactions.some((item) => item.dedupeHash === selectedHash)) {
      setSelectedHash(transactions[0]?.dedupeHash ?? null);
    }
  }, [selectedHash, transactions]);

  return (
    <main className="workspace transactions-workspace">
      <header className="workspace-title-row">
        <div><span className="eyebrow">{L.transactions.ledger(transactions.length)}</span><h1>{L.transactions.title}</h1></div>
        <span className="inline-status">{status || L.transactions.selectRow}</span>
      </header>

      <div className="transaction-tools">
        <label className="search-field">
          <Search size={17} />
          <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder={L.transactions.searchPlaceholder} />
          {search && <button aria-label={L.transactions.clearSearch} onClick={() => setSearch("")}><X size={15} /></button>}
        </label>
        <div className="segmented-control" aria-label={L.transactions.filterLabel}>
          {(["all", "credit", "debit", "review"] as Filter[]).map((value) => (
            <button key={value} className={filter === value ? "active" : ""} onClick={() => setFilter(value)}>{L.transactions.filters[value]}</button>
          ))}
        </div>
      </div>

      <div className="transaction-layout">
        <div className="table-wrap">
          <table className="transaction-table">
            <thead><tr><th>{L.transactions.columns.date}</th><th>{L.transactions.columns.vendor}</th><th>{L.transactions.columns.category}</th><th>{L.transactions.columns.account}</th><th>{L.transactions.columns.amount}</th></tr></thead>
            <tbody>
              {filtered.map((transaction) => (
                <tr
                  key={transaction.dedupeHash}
                  className={transaction.dedupeHash === selectedHash ? "selected" : ""}
                  onClick={() => setSelectedHash(transaction.dedupeHash)}
                >
                  <td>{dateLabel(transaction.date, L.locale)}</td>
                  <td><strong>{transaction.vendorUser || transaction.vendorNormalized}</strong><small>{transaction.sourceFile}</small></td>
                  <td><span className={transaction.categoryConfidence < 0.6 ? "review-tag" : "technical-tag"}>{categoryLabel(transaction.categoryUser || transaction.categoryAuto)}</span></td>
                  <td>{transaction.accountLabel || "—"}</td>
                  <td className={transaction.transactionType === "credit" ? "good" : ""}>
                    {transaction.transactionType === "credit" ? "+" : "−"}{money(transaction.amountMinor, transaction.currency || currency, L.locale)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {filtered.length === 0 && <div className="table-empty">{L.transactions.noMatches}</div>}
        </div>

        <aside className={`transaction-inspector ${selected ? "open" : ""}`}>
          {selected ? <TransactionEditor key={selected.dedupeHash} transaction={selected} busy={busy} onUpdate={onUpdate} /> : (
            <div className="inspector-empty"><SlidersHorizontal size={24} /><span>{L.transactions.noSelection}</span></div>
          )}
        </aside>
      </div>
    </main>
  );
}

function TransactionEditor({ transaction, busy, onUpdate }: {
  transaction: TransactionRecord;
  busy: boolean;
  onUpdate: (update: TransactionUpdate) => Promise<void>;
}) {
  const { L, categoryLabel } = useI18n();
  const [category, setCategory] = useState(transaction.categoryUser || transaction.categoryAuto);
  const [vendor, setVendor] = useState(transaction.vendorUser || transaction.vendorNormalized);
  const [notes, setNotes] = useState(transaction.notesUser || "");
  const [tags, setTags] = useState(transaction.tagsUser.join(", "));
  const [excluded, setExcluded] = useState(transaction.isExcluded);

  const save = () => onUpdate({
    dedupeHash: transaction.dedupeHash,
    category: category === transaction.categoryAuto ? null : category,
    vendor: vendor === transaction.vendorNormalized ? null : vendor,
    notes: notes || null,
    tags: tags.split(",").map((value) => value.trim()).filter(Boolean),
    isExcluded: excluded,
  });

  return (
    <form onSubmit={(event) => { event.preventDefault(); void save(); }}>
      <div className="inspector-heading"><span className="eyebrow">{L.transactions.editEyebrow}</span><strong>{money(transaction.amountMinor, transaction.currency, L.locale)}</strong></div>
      <label className="field-label">{L.transactions.vendor}<input value={vendor} onChange={(event) => setVendor(event.target.value)} /></label>
      <label className="field-label">{L.transactions.category}<select value={category} onChange={(event) => setCategory(event.target.value)}>{CATEGORIES.map((value) => <option key={value} value={value}>{categoryLabel(value)}</option>)}</select></label>
      <label className="field-label">{L.transactions.notes}<textarea value={notes} onChange={(event) => setNotes(event.target.value)} rows={3} placeholder={L.transactions.privateNote} /></label>
      <label className="field-label">{L.transactions.tags}<input value={tags} onChange={(event) => setTags(event.target.value)} placeholder={L.transactions.tagsPlaceholder} /></label>
      <label className="toggle-row">
        <span><strong>{L.transactions.exclude}</strong><small>{L.transactions.excludeDetail}</small></span>
        <input type="checkbox" checked={excluded} onChange={(event) => setExcluded(event.target.checked)} />
        <span className="toggle-track"><i /></span>
      </label>
      <div className="inspector-meta">
        <span>{L.transactions.autoConfidence}</span><strong>{(transaction.categoryConfidence * 100).toFixed(0)}%</strong>
        <span>{L.transactions.recurring}</span><strong>{transaction.isRecurring === true ? L.values.yes : transaction.isRecurring === false ? L.values.no : L.values.unknown}</strong>
        <span>{L.transactions.reference}</span><strong>{transaction.uniqueId}</strong>
      </div>
      <button className="primary-button full-button" disabled={busy}>{L.transactions.saveChanges}</button>
    </form>
  );
}
