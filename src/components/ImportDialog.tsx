import { CheckCircle2, FileText, ShieldCheck, X } from "lucide-react";
import { useMemo, useState } from "react";
import { useI18n } from "../i18n";
import type { AccountProfile, ImportAssignment, ImportPlan, ImportReceipt } from "../types";

interface Draft {
  mode: "existing" | "new";
  accountId: string;
  nickname: string;
  accountType: string;
}

interface ImportDialogProps {
  plan: ImportPlan;
  accounts: AccountProfile[];
  receipt: ImportReceipt | null;
  busy: boolean;
  onCancel: () => void;
  onCommit: (assignments: ImportAssignment[]) => Promise<void>;
}

export function ImportDialog({ plan, accounts, receipt, busy, onCancel, onCommit }: ImportDialogProps) {
  const { language } = useI18n();
  const C = language === "es" ? {
    preflight: "REVISIÓN OFX / SOLO LOCAL", complete: "Importación completa", confirm: "Confirmar cuentas", close: "Cerrar diálogo",
    review: "Revisa a qué cuenta pertenece cada estado. Los números completos permanecen en la base local y nunca se muestran aquí.",
    fresh: "NUEVOS", duplicates: "DUPLICADOS OMITIDOS", conflicts: "CONFLICTOS RETENIDOS", checked: "movimientos revisados", done: "LISTO",
    idFound: "ID DE CUENTA ENCONTRADO", idMissing: "FALTA ID DE CUENTA", matched: "Asignada automáticamente a", existing: "CUENTA EXISTENTE", newProfile: "PERFIL NUEVO",
    account: "CUENTA", profileName: "NOMBRE DEL PERFIL", accountType: "TIPO DE CUENTA", accountPlaceholder: "Ahorros, corriente…", cancel: "CANCELAR", importing: "IMPORTANDO…", submit: "IMPORTAR MOVIMIENTOS", transactions: "movimientos",
  } : {
    preflight: "OFX PREFLIGHT / LOCAL ONLY", complete: "Import complete", confirm: "Confirm accounts", close: "Close dialog",
    review: "Review where each statement belongs. Full account numbers remain inside the local database and are never shown here.",
    fresh: "NEW", duplicates: "DUPLICATES SKIPPED", conflicts: "CONFLICTS HELD", checked: "transactions checked", done: "DONE",
    idFound: "ACCOUNT ID FOUND", idMissing: "ACCOUNT ID MISSING", matched: "Matched automatically to", existing: "EXISTING ACCOUNT", newProfile: "NEW PROFILE",
    account: "ACCOUNT", profileName: "PROFILE NAME", accountType: "ACCOUNT TYPE", accountPlaceholder: "Savings, checking…", cancel: "CANCEL", importing: "IMPORTING…", submit: "IMPORT TRANSACTIONS", transactions: "transactions",
  };
  const initial = useMemo(() => Object.fromEntries(plan.statements.map((statement) => {
    const compatible = accounts.find((account) => !account.archived && account.currency === statement.currency);
    return [statement.detectedId, {
      mode: statement.identityReliable || !compatible ? "new" : "existing",
      accountId: compatible?.id ?? "",
      nickname: statement.suggestedNickname,
      accountType: statement.sourceAccountType === "CHECKING" ? "Checking" : statement.sourceAccountType === "SAVINGS" ? "Savings" : statement.sourceAccountType ?? "",
    } satisfies Draft];
  })), [accounts, plan.statements]);
  const [drafts, setDrafts] = useState<Record<string, Draft>>(initial);
  const setDraft = (id: string, patch: Partial<Draft>) => setDrafts((current) => ({ ...current, [id]: { ...current[id], ...patch } }));
  const assignments = plan.statements.map((statement): ImportAssignment => {
    const draft = drafts[statement.detectedId];
    if (statement.matchedAccountId) return { detectedId: statement.detectedId, accountProfileId: statement.matchedAccountId, newAccount: null };
    return draft.mode === "existing"
      ? { detectedId: statement.detectedId, accountProfileId: draft.accountId || null, newAccount: null }
      : { detectedId: statement.detectedId, accountProfileId: null, newAccount: { nickname: draft.nickname.trim(), displayAccountType: draft.accountType.trim() || null } };
  });
  const valid = plan.statements.every((statement) => {
    if (statement.matchedAccountId) return true;
    const draft = drafts[statement.detectedId];
    return draft.mode === "existing" ? !!draft.accountId : !!draft.nickname.trim();
  });

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={busy ? undefined : onCancel}>
      <section className="import-dialog" role="dialog" aria-modal="true" aria-labelledby="import-dialog-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="dialog-heading">
          <div><span className="eyebrow">{C.preflight}</span><h2 id="import-dialog-title">{receipt ? C.complete : C.confirm}</h2></div>
          <button type="button" className="icon-button" aria-label={C.close} disabled={busy} onClick={onCancel}><X size={18} /></button>
        </div>
        {receipt ? (
          <div className="import-receipt">
            <CheckCircle2 size={30} className="good" />
            <div className="receipt-totals">
              <span><strong>{receipt.newTransactions}</strong><small>{C.fresh}</small></span>
              <span><strong>{receipt.duplicateTransactions}</strong><small>{C.duplicates}</small></span>
              <span><strong>{receipt.conflictTransactions}</strong><small>{C.conflicts}</small></span>
            </div>
            {receipt.files.map((file) => <div className="receipt-file" key={`${file.sourceFile}-${file.accountProfileId}`}><FileText size={16} /><span><strong>{file.sourceFile}</strong><small>{file.totalTransactions} {C.checked}</small></span></div>)}
            {receipt.errors.map((error) => <p className="form-error" key={error}>{error}</p>)}
            <button className="primary-button" onClick={onCancel}>{C.done}</button>
          </div>
        ) : (
          <>
            <p className="import-intro"><ShieldCheck size={17} /> {C.review}</p>
            <div className="detected-statements">
              {plan.statements.map((statement) => {
                const draft = drafts[statement.detectedId];
                const matched = accounts.find((account) => account.id === statement.matchedAccountId);
                const compatible = accounts.filter((account) => !account.archived && account.currency === statement.currency);
                return (
                  <article className="detected-statement" key={statement.detectedId}>
                    <div className="statement-summary">
                      <FileText size={19} />
                      <span><strong>{statement.institutionName || (language === "es" ? "Estado bancario" : "Bank statement")} {statement.maskedIdentifier || ""}</strong><small>{statement.sourceFile} · {statement.transactionCount} {C.transactions} · {statement.currency}</small></span>
                      <em>{statement.identityReliable ? C.idFound : C.idMissing}</em>
                    </div>
                    {matched ? (
                      <div className="account-match"><CheckCircle2 size={16} /><span>{C.matched} <strong>{matched.nickname}</strong></span></div>
                    ) : (
                      <div className="account-assignment">
                        {!statement.identityReliable && compatible.length > 0 && (
                          <div className="assignment-tabs" role="group" aria-label={language === "es" ? "Asignación de cuenta" : "Account assignment"}>
                            <button className={draft.mode === "existing" ? "active" : ""} onClick={() => setDraft(statement.detectedId, { mode: "existing" })}>{C.existing}</button>
                            <button className={draft.mode === "new" ? "active" : ""} onClick={() => setDraft(statement.detectedId, { mode: "new" })}>{C.newProfile}</button>
                          </div>
                        )}
                        {draft.mode === "existing" && !statement.identityReliable ? (
                          <label className="field-label">{C.account}<select value={draft.accountId} onChange={(event) => setDraft(statement.detectedId, { accountId: event.target.value })}>{compatible.map((account) => <option key={account.id} value={account.id}>{account.nickname} {account.maskedIdentifier || ""}</option>)}</select></label>
                        ) : (
                          <div className="assignment-fields">
                            <label className="field-label">{C.profileName}<input value={draft.nickname} maxLength={80} onChange={(event) => setDraft(statement.detectedId, { nickname: event.target.value })} /></label>
                            <label className="field-label">{C.accountType}<input value={draft.accountType} maxLength={40} placeholder={C.accountPlaceholder} onChange={(event) => setDraft(statement.detectedId, { accountType: event.target.value })} /></label>
                          </div>
                        )}
                      </div>
                    )}
                  </article>
                );
              })}
            </div>
            {plan.errors.map((error) => <p className="form-error" key={error}>{error}</p>)}
            <div className="dialog-actions"><button className="secondary-button" disabled={busy} onClick={onCancel}>{C.cancel}</button><button className="primary-button" disabled={busy || !valid} onClick={() => void onCommit(assignments)}>{busy ? C.importing : C.submit}</button></div>
          </>
        )}
      </section>
    </div>
  );
}
