import { Archive, ArchiveRestore, Database, FolderOpen, Landmark, Monitor, Moon, Save, Sun } from "lucide-react";
import { useState } from "react";
import { translatedMigrationMessage, useI18n } from "../i18n";
import type { AccountProfile, AccountUpdate, DashboardSnapshot, ThemeMode } from "../types";

interface SettingsProps {
  snapshot: DashboardSnapshot;
  theme: ThemeMode;
  onTheme: (theme: ThemeMode) => void;
  onReveal: (path: string) => Promise<void>;
  onUpdateAccount: (update: AccountUpdate) => Promise<void>;
}

function AccountEditor({ account, onUpdate, language }: { account: AccountProfile; onUpdate: (update: AccountUpdate) => Promise<void>; language: "en" | "es" }) {
  const [nickname, setNickname] = useState(account.nickname);
  const [accountType, setAccountType] = useState(account.displayAccountType ?? "");
  const changed = nickname.trim() !== account.nickname || accountType.trim() !== (account.displayAccountType ?? "");
  return (
    <article className={`account-editor ${account.archived ? "archived" : ""}`}>
      <div className="account-editor-heading"><Landmark size={18} /><span><strong>{account.institutionName || (language === "es" ? "Cuenta bancaria" : "Bank account")} {account.maskedIdentifier || ""}</strong><small>{account.currency} · {language === "es" ? "Tipo de origen" : "Source type"}: {account.sourceAccountType || (language === "es" ? "desconocido" : "unknown")}</small></span><em>{account.archived ? (language === "es" ? "ARCHIVADA" : "ARCHIVED") : (language === "es" ? "ACTIVA" : "ACTIVE")}</em></div>
      <div className="account-editor-fields">
        <label className="field-label">{language === "es" ? "NOMBRE DEL PERFIL" : "PROFILE NAME"}<input value={nickname} maxLength={80} onChange={(event) => setNickname(event.target.value)} /></label>
        <label className="field-label">{language === "es" ? "TIPO VISIBLE" : "DISPLAY TYPE"}<input value={accountType} maxLength={40} placeholder={language === "es" ? "Ahorros, corriente…" : "Savings, checking…"} onChange={(event) => setAccountType(event.target.value)} /></label>
      </div>
      <div className="account-editor-actions">
        <button className="secondary-button" disabled={!changed || !nickname.trim()} onClick={() => void onUpdate({ id: account.id, nickname: nickname.trim(), displayAccountType: accountType.trim() || null, archived: account.archived })}><Save size={15} /> {language === "es" ? "GUARDAR" : "SAVE"}</button>
        <button className="quiet-button" onClick={() => void onUpdate({ id: account.id, nickname: nickname.trim() || account.nickname, displayAccountType: accountType.trim() || null, archived: !account.archived })}>{account.archived ? <ArchiveRestore size={15} /> : <Archive size={15} />}{account.archived ? (language === "es" ? "RESTAURAR" : "RESTORE") : (language === "es" ? "ARCHIVAR" : "ARCHIVE")}</button>
      </div>
    </article>
  );
}

export function Settings({ snapshot, theme, onTheme, onReveal, onUpdateAccount }: SettingsProps) {
  const { L, language } = useI18n();
  return (
    <main className="workspace settings-workspace">
      <header className="workspace-title-row"><div><span className="eyebrow">{L.settings.eyebrow}</span><h1>{L.settings.title}</h1></div></header>

      <section className="settings-section">
        <div className="settings-heading"><span>01</span><div><h2>{L.settings.appearance}</h2><p>{L.settings.appearanceDetail}</p></div></div>
        <div className="theme-picker" role="radiogroup" aria-label={L.settings.themeLabel}>
          {([
            ["dark", Moon],
            ["light", Sun],
            ["system", Monitor],
          ] as const).map(([value, Icon]) => (
            <button key={value} className={theme === value ? "active" : ""} onClick={() => onTheme(value)} role="radio" aria-checked={theme === value}>
              <Icon size={20} /><strong>{L.settings.themes[value]}</strong><small>{L.settings.themeCaptions[value]}</small>
            </button>
          ))}
        </div>
      </section>

      <section className="settings-section">
        <div className="settings-heading"><span>02</span><div><h2>{language === "es" ? "Perfiles de cuenta" : "Account profiles"}</h2><p>{language === "es" ? "Cambia el nombre o corrige el tipo visible. El tipo original permanece visible para auditoría." : "Rename accounts or correct the display type. The source type stays visible for auditability."}</p></div></div>
        <div className="account-editors">{snapshot.accounts.map((account) => <AccountEditor key={`${account.id}-${account.updatedAt}`} account={account} onUpdate={onUpdateAccount} language={language} />)}</div>
      </section>

      <section className="settings-section">
        <div className="settings-heading"><span>03</span><div><h2>{L.settings.dataLocations}</h2><p>{L.settings.dataDetail}</p></div></div>
        <div className="path-list">
          <button onClick={() => void onReveal(snapshot.paths.database)}>
            <Database size={19} /><span><small>{L.settings.databasePath}</small><strong>{snapshot.paths.database}</strong></span><FolderOpen size={17} />
          </button>
          <button onClick={() => void onReveal(snapshot.paths.library)}>
            <FolderOpen size={19} /><span><small>{L.settings.libraryPath}</small><strong>{snapshot.paths.library}</strong></span><FolderOpen size={17} />
          </button>
        </div>
      </section>

      <section className="settings-section">
        <div className="settings-heading"><span>04</span><div><h2>{L.settings.migration}</h2><p>{L.settings.migrationDetail}</p></div></div>
        <div className="migration-readout">
          <span className={snapshot.migration.migrationPerformed ? "good" : ""}>{snapshot.migration.migrationPerformed ? L.settings.migrated : L.settings.noAction}</span>
          <strong>{translatedMigrationMessage(snapshot.migration.message, snapshot.migration.importedTransactions, L)}</strong>
          <small>{snapshot.paths.legacyDatabase}</small>
        </div>
      </section>

      <section className="settings-section">
        <div className="settings-heading"><span>05</span><div><h2>{L.settings.privacy}</h2><p>{L.settings.privacyDetail}</p></div></div>
        <div className="privacy-grid">
          <span><strong>{L.settings.ofxImports}</strong><small>{L.settings.localOnly}</small></span>
          <span><strong>{L.settings.accountIds}</strong><small>{L.settings.neverReports}</small></span>
          <span><strong>{L.settings.aiShare}</strong><small>{L.settings.explicitSummary}</small></span>
          <span><strong>{L.settings.delete}</strong><small>{L.settings.movesToTrash}</small></span>
        </div>
      </section>
    </main>
  );
}
