import { BookmarkPlus, Save, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { dateLabel, money } from "../format";
import { useI18n } from "../i18n";
import type { AccountProfile, SmartList, SmartListInput, SmartListRules, TransactionRecord } from "../types";
import { SchemaForm, type FormField, type FormValue, type FormValues } from "./SchemaForm";

const EMPTY_RULES: SmartListRules = {
  sourceQuery: "",
  sourceMatch: "contains",
  direction: "all",
  accountProfileIds: [],
  category: null,
  tag: null,
  dateStart: null,
  dateEnd: null,
  minAmountMinor: null,
  maxAmountMinor: null,
};

function normalize(value: string) {
  return value.normalize("NFKD").replace(/[\u0300-\u036f]/g, "").trim().toLocaleLowerCase();
}

function matchesSmartList(transaction: TransactionRecord, rules: SmartListRules) {
  const source = normalize(transaction.vendorUser || transaction.vendorNormalized || transaction.vendorClean || transaction.vendorRaw);
  const query = normalize(rules.sourceQuery);
  const sourceMatches = !query
    || (rules.sourceMatch === "exact" && source === query)
    || (rules.sourceMatch === "starts_with" && source.startsWith(query))
    || (rules.sourceMatch === "contains" && source.includes(query));
  const absoluteAmount = Math.abs(transaction.amountMinor);
  return sourceMatches
    && (rules.direction === "all" || transaction.transactionType === rules.direction)
    && (rules.accountProfileIds.length === 0 || (!!transaction.accountProfileId && rules.accountProfileIds.includes(transaction.accountProfileId)))
    && (!rules.category || (transaction.categoryUser || transaction.categoryAuto) === rules.category)
    && (!rules.tag || transaction.tagsUser.some((tag) => normalize(tag) === normalize(rules.tag || "")))
    && (!rules.dateStart || transaction.date.slice(0, 10) >= rules.dateStart)
    && (!rules.dateEnd || transaction.date.slice(0, 10) <= rules.dateEnd)
    && (rules.minAmountMinor == null || absoluteAmount >= rules.minAmountMinor)
    && (rules.maxAmountMinor == null || absoluteAmount <= rules.maxAmountMinor);
}

function rulesToValues(rules: SmartListRules): FormValues {
  return {
    sourceQuery: rules.sourceQuery,
    sourceMatch: rules.sourceMatch,
    direction: rules.direction,
    accountProfileIds: rules.accountProfileIds,
    category: rules.category ?? "",
    tag: rules.tag ?? "",
    dateStart: rules.dateStart ?? "",
    dateEnd: rules.dateEnd ?? "",
    minAmount: rules.minAmountMinor == null ? "" : (rules.minAmountMinor / 100).toString(),
    maxAmount: rules.maxAmountMinor == null ? "" : (rules.maxAmountMinor / 100).toString(),
  };
}

function amountMinor(value: FormValue) {
  const raw = Array.isArray(value) ? "" : value.trim();
  if (!raw) return null;
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? Math.round(parsed * 100) : null;
}

function valuesToRules(values: FormValues): SmartListRules {
  const text = (id: string) => Array.isArray(values[id]) ? "" : String(values[id] ?? "");
  return {
    sourceQuery: text("sourceQuery").trim(),
    sourceMatch: text("sourceMatch") as SmartListRules["sourceMatch"],
    direction: text("direction") as SmartListRules["direction"],
    accountProfileIds: Array.isArray(values.accountProfileIds) ? values.accountProfileIds : [],
    category: text("category").trim() || null,
    tag: text("tag").trim() || null,
    dateStart: text("dateStart") || null,
    dateEnd: text("dateEnd") || null,
    minAmountMinor: amountMinor(values.minAmount),
    maxAmountMinor: amountMinor(values.maxAmount),
  };
}

interface SmartListsProps {
  lists: SmartList[];
  accounts: AccountProfile[];
  transactions: TransactionRecord[];
  currency: string;
  busy: boolean;
  onSave: (input: SmartListInput) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}

export function SmartLists({ lists, accounts, transactions, currency, busy, onSave, onDelete }: SmartListsProps) {
  const { language } = useI18n();
  const es = language === "es";
  const [selectedId, setSelectedId] = useState<string | null>(lists[0]?.id ?? null);
  const selected = lists.find((list) => list.id === selectedId) ?? null;
  const [name, setName] = useState(selected?.name ?? "");
  const [values, setValues] = useState<FormValues>(rulesToValues(selected?.rules ?? EMPTY_RULES));
  const fields = useMemo<FormField[]>(() => [{
    kind: "group", id: "source", label: es ? "Origen" : "Source", description: es ? "Coincide con el comercio o remitente normalizado. Se ignoran tildes y mayúsculas." : "Match the normalized merchant or sender name. Accents and letter case are ignored.", children: [
      { kind: "text", id: "sourceQuery", label: es ? "NOMBRE DEL ORIGEN" : "SOURCE NAME", placeholder: es ? "Amazon, planilla, servicio…" : "Amazon, payroll, utility…" },
      { kind: "select", id: "sourceMatch", label: es ? "COINCIDENCIA" : "MATCH", options: [
        { value: "contains", label: es ? "Contiene" : "Contains" }, { value: "exact", label: es ? "Exacta" : "Exact" }, { value: "starts_with", label: es ? "Empieza con" : "Starts with" },
      ] },
      { kind: "select", id: "direction", label: es ? "DIRECCIÓN" : "DIRECTION", options: [
        { value: "all", label: es ? "Créditos + débitos" : "Credits + debits" }, { value: "credit", label: es ? "Solo créditos" : "Credits only" }, { value: "debit", label: es ? "Solo débitos" : "Debits only" },
      ] },
    ],
  }, {
    kind: "group", id: "scope", label: es ? "Alcance" : "Scope", children: [
      { kind: "multi", id: "accountProfileIds", label: es ? "CUENTAS (NINGUNA = TODAS)" : "ACCOUNTS (NONE MEANS ALL)", options: accounts.filter((account) => !account.archived && account.currency === currency).map((account) => ({ value: account.id, label: account.nickname })) },
      { kind: "text", id: "category", label: es ? "CATEGORÍA" : "CATEGORY", placeholder: "online_shopping" },
      { kind: "text", id: "tag", label: es ? "ETIQUETA" : "TAG", placeholder: es ? "deducible" : "tax deductible" },
    ],
  }, {
    kind: "group", id: "range", label: es ? "Rango opcional" : "Optional range", children: [
      { kind: "date", id: "dateStart", label: es ? "DESDE" : "FROM" }, { kind: "date", id: "dateEnd", label: es ? "HASTA" : "TO" },
      { kind: "number", id: "minAmount", label: `${es ? "MÍN" : "MIN"} ${currency}`, step: "0.01" }, { kind: "number", id: "maxAmount", label: `${es ? "MÁX" : "MAX"} ${currency}`, step: "0.01" },
    ],
  }], [accounts, currency, es]);
  const rules = valuesToRules(values);
  const results = useMemo(() => transactions.filter((transaction) => matchesSmartList(transaction, rules)), [transactions, rules.sourceQuery, rules.sourceMatch, rules.direction, JSON.stringify(rules.accountProfileIds), rules.category, rules.tag, rules.dateStart, rules.dateEnd, rules.minAmountMinor, rules.maxAmountMinor]);
  const total = results.reduce((sum, transaction) => sum + (transaction.transactionType === "credit" ? transaction.amountMinor : -transaction.amountMinor), 0);

  const edit = (list: SmartList | null) => {
    setSelectedId(list?.id ?? null);
    setName(list?.name ?? "");
    setValues(rulesToValues(list?.rules ?? EMPTY_RULES));
  };
  return (
    <main className="workspace lists-workspace">
      <header className="workspace-title-row"><div><span className="eyebrow">{es ? "SEGUIMIENTO PERSONAL / DETERMINISTA" : "CUSTOM TRACKING / DETERMINISTIC"}</span><h1>{es ? "Listas inteligentes" : "Smart lists"}</h1></div><button className="secondary-button" onClick={() => edit(null)}><BookmarkPlus size={16} /> {es ? "NUEVA LISTA" : "NEW LIST"}</button></header>
      <div className="lists-layout">
        <aside className="saved-lists">
          {lists.map((list) => <button key={list.id} className={selectedId === list.id ? "active" : ""} onClick={() => edit(list)}><strong>{list.name}</strong><small>{transactions.filter((transaction) => matchesSmartList(transaction, list.rules)).length} {es ? "coincidencias" : "matches"}</small></button>)}
          {lists.length === 0 && <p>{es ? "[NO HAY LISTAS GUARDADAS]" : "[NO SAVED LISTS]"}</p>}
        </aside>
        <section className="list-builder">
          <label className="field-label">{es ? "NOMBRE DE LA LISTA" : "LIST NAME"}<input value={name} maxLength={80} placeholder={es ? "Amazon este año" : "Amazon this year"} onChange={(event) => setName(event.target.value)} /></label>
          <SchemaForm fields={fields} values={values} onChange={(id, value) => setValues((current) => ({ ...current, [id]: value }))} />
          <div className="list-actions">
            {selected && <button className="destructive-button" disabled={busy} onClick={async () => { await onDelete(selected.id); edit(null); }}><Trash2 size={16} /> {es ? "ELIMINAR" : "DELETE"}</button>}
            <button className="primary-button" disabled={busy || !name.trim()} onClick={() => void onSave({ id: selectedId, name: name.trim(), rules })}><Save size={16} /> {es ? "GUARDAR LISTA" : "SAVE LIST"}</button>
          </div>
        </section>
        <section className="list-preview">
          <div className="list-preview-summary"><span>{results.length} {es ? "COINCIDENCIAS" : "MATCHES"}</span><strong>{money(total, currency)}</strong><small>{es ? "TOTAL NETO" : "NET TOTAL"}</small></div>
          <div className="list-preview-rows">{results.slice(0, 50).map((transaction) => <div key={transaction.dedupeHash}><span><strong>{transaction.vendorUser || transaction.vendorClean}</strong><small>{dateLabel(transaction.date)} · {transaction.categoryUser || transaction.categoryAuto}</small></span><strong className={transaction.transactionType === "credit" ? "good" : ""}>{transaction.transactionType === "credit" ? "+" : "−"}{money(transaction.amountMinor, currency)}</strong></div>)}</div>
          {results.length === 0 && <p className="empty-readout">{es ? "[NINGÚN MOVIMIENTO COINCIDE]" : "[NO TRANSACTIONS MATCH THESE RULES]"}</p>}
        </section>
      </div>
    </main>
  );
}
