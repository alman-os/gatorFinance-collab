import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { confirm as askConfirm, open } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  BarChart3,
  BookOpen,
  FileUp,
  ListFilter,
  LoaderCircle,
  Menu,
  Moon,
  ReceiptText,
  Save,
  Settings as SettingsIcon,
  Sun,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import appIconDark from "./assets/icons/gatorfinance-dark.png";
import appIconDefault from "./assets/icons/gatorfinance-default.png";
import { api, isTauri } from "./api";
import "./App.css";
import { Library } from "./components/Library";
import { ImportDialog } from "./components/ImportDialog";
import { Overview } from "./components/Overview";
import { Settings } from "./components/Settings";
import { SmartLists } from "./components/SmartLists";
import { Transactions } from "./components/Transactions";
import { errorMessage } from "./format";
import { I18N, useI18n, type Copy } from "./i18n";
import { mockSnapshot } from "./mock";
import type { AccountUpdate, DashboardSnapshot, ImportAssignment, ImportPlan, ImportReceipt, SmartListInput, ThemeMode, TransactionUpdate, Workspace } from "./types";

const NAV_ITEMS: Array<[Workspace, typeof BarChart3]> = [
  ["overview", BarChart3],
  ["transactions", ReceiptText],
  ["lists", ListFilter],
  ["library", BookOpen],
  ["settings", SettingsIcon],
];

type StatusRenderer = (copy: Copy) => string;

function getStoredTheme(): ThemeMode {
  const stored = localStorage.getItem("gatorfinance-theme");
  return stored === "light" || stored === "system" ? stored : "dark";
}

function App() {
  const { language, setLanguage, L } = useI18n();
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [workspace, setWorkspace] = useState<Workspace>("overview");
  const [theme, setTheme] = useState<ThemeMode>(getStoredTheme);
  const [resolvedTheme, setResolvedTheme] = useState<"dark" | "light">("dark");
  const [busy, setBusy] = useState(true);
  const [statusMessage, setStatusMessage] = useState<{ render: StatusRenderer }>({ render: (copy) => copy.status.initializing });
  const [menuOpen, setMenuOpen] = useState(false);
  const [reportDialogOpen, setReportDialogOpen] = useState(false);
  const [reportTitle, setReportTitle] = useState<string>(() => L.dialogs.defaultReportTitle);
  const [dragActive, setDragActive] = useState(false);
  const [importPlan, setImportPlan] = useState<ImportPlan | null>(null);
  const [importReceipt, setImportReceipt] = useState<ImportReceipt | null>(null);
  const [scopeValue, setScopeValue] = useState("");
  const scopeRef = useRef<{ accountIds?: string[]; currency?: string }>({});

  const refresh = useCallback(async () => {
    if (!isTauri()) {
      setSnapshot(mockSnapshot);
      return mockSnapshot;
    }
    const next = await api.bootstrap(scopeRef.current.accountIds, scopeRef.current.currency);
    setSnapshot(next);
    const requestedAccount = scopeRef.current.accountIds?.[0];
    if (requestedAccount && !next.selectedAccountIds.includes(requestedAccount)) {
      scopeRef.current = { currency: next.currency };
      setScopeValue(`currency:${next.currency}`);
    } else if (!scopeRef.current.currency && !scopeRef.current.accountIds) {
      scopeRef.current = { currency: next.currency };
      setScopeValue(`currency:${next.currency}`);
    }
    return next;
  }, []);

  const showStatus = useCallback((render: StatusRenderer) => {
    setStatusMessage({ render });
  }, []);

  const status = statusMessage.render(L);

  const run = useCallback(async (action: () => Promise<void>, pending: StatusRenderer) => {
    setBusy(true);
    showStatus(pending);
    try {
      await action();
    } catch (error) {
      showStatus((copy) => copy.status.error(errorMessage(error)));
    } finally {
      setBusy(false);
    }
  }, [showStatus]);

  useEffect(() => {
    void run(async () => {
      await refresh();
      showStatus((copy) => copy.status.localReady);
    }, (copy) => copy.status.initializing);
  }, [refresh, run, showStatus]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const applyTheme = () => {
      const resolved = theme === "system" ? (media.matches ? "light" : "dark") : theme;
      setResolvedTheme(resolved);
      document.documentElement.dataset.theme = resolved;
      document.documentElement.style.colorScheme = resolved;
    };
    applyTheme();
    media.addEventListener("change", applyTheme);
    localStorage.setItem("gatorfinance-theme", theme);
    return () => media.removeEventListener("change", applyTheme);
  }, [theme]);

  useEffect(() => {
    setReportTitle((current) => (
      current === I18N.en.dialogs.defaultReportTitle || current === I18N.es.dialogs.defaultReportTitle
        ? L.dialogs.defaultReportTitle
        : current
    ));
  }, [L]);

  const importPaths = useCallback((paths: string[]) => {
    const ofxPaths = paths.filter((path) => path.toLowerCase().endsWith(".ofx"));
    if (ofxPaths.length === 0) {
      showStatus((copy) => copy.status.noOfx);
      return Promise.resolve();
    }
    return run(async () => {
      if (!isTauri()) {
        setImportReceipt(null);
        setImportPlan({
          planId: "demo-import",
          errors: [],
          statements: [{
            detectedId: "demo-statement",
            sourceFile: ofxPaths[0].split("/").pop() || "demo-statement.ofx",
            institutionName: "Banco General",
            maskedIdentifier: "•••• 2048",
            sourceAccountType: "CHECKING",
            currency: "USD",
            periodStart: "2026-01-01T00:00:00",
            periodEnd: "2026-09-08T00:00:00",
            transactionCount: 850,
            matchedAccountId: null,
            identityReliable: true,
            suggestedNickname: "Banco General Checking •••• 2048",
          }],
        });
        showStatus((copy) => copy.status.demoOfx(ofxPaths.length));
        return;
      }
      const plan = await api.inspectStatements(ofxPaths);
      setImportReceipt(null);
      setImportPlan(plan);
      showStatus(() => `[IMPORT READY] ${plan.statements.length} ACCOUNT STATEMENT${plan.statements.length === 1 ? "" : "S"} DETECTED`);
    }, () => `[INSPECTING ${ofxPaths.length} OFX FILE${ofxPaths.length === 1 ? "" : "S"}]`);
  }, [run, showStatus]);

  const commitImport = useCallback(async (assignments: ImportAssignment[]) => {
    if (!importPlan) return;
    await run(async () => {
      const receipt = isTauri()
        ? await api.commitImport(importPlan.planId, assignments)
        : {
          batchId: "demo-batch",
          files: [{ sourceFile: importPlan.statements[0].sourceFile, accountProfileId: "demo", newTransactions: 0, duplicateTransactions: 850, conflictTransactions: 0, totalTransactions: 850 }],
          errors: [], newTransactions: 0, duplicateTransactions: 850, conflictTransactions: 0,
        };
      setImportReceipt(receipt);
      await refresh();
      showStatus((copy) => copy.status.importComplete(receipt.newTransactions, receipt.duplicateTransactions, receipt.errors.length));
    }, () => "[DEDUPLICATING + IMPORTING]");
  }, [importPlan, refresh, run, showStatus]);

  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    void getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") setDragActive(true);
      if (event.payload.type === "leave") setDragActive(false);
      if (event.payload.type === "drop") {
        setDragActive(false);
        void importPaths(event.payload.paths);
      }
    }).then((stop) => { unlisten = stop; });
    return () => unlisten?.();
  }, [importPaths]);

  const chooseStatements = useCallback(async () => {
    if (!isTauri()) return importPaths(["demo-statement.ofx"]);
    const selected = await open({
      title: L.dialogs.importOfxTitle,
      multiple: true,
      directory: false,
      filters: [{ name: L.dialogs.ofxFilter, extensions: ["ofx"] }],
    });
    if (selected) await importPaths(Array.isArray(selected) ? selected : [selected]);
  }, [L, importPaths]);

  const saveReport = useCallback(async () => {
    const title = reportTitle.trim();
    const currentSnapshot = snapshot;
    if (!currentSnapshot) return;
    if (!title) {
      showStatus((copy) => copy.status.reportTitleRequired);
      return;
    }
    setReportDialogOpen(false);
    await run(async () => {
      if (!isTauri()) {
        showStatus((copy) => copy.status.demoReportReady);
        return;
      }
      const result = await api.saveReport(title, currentSnapshot.selectedAccountIds, currentSnapshot.currency);
      await refresh();
      showStatus((copy) => copy.status.reportSaved(result.item?.title ?? title));
    }, (copy) => copy.status.buildingReport);
  }, [refresh, reportTitle, run, showStatus, snapshot]);

  const updateTransaction = useCallback(async (update: TransactionUpdate) => {
    await run(async () => {
      if (isTauri()) await api.updateTransaction(update);
      await refresh();
      showStatus((copy) => copy.status.overrideSaved);
    }, (copy) => copy.status.savingOverride);
  }, [refresh, run, showStatus]);

  const refreshLibrary = useCallback(async () => {
    await run(async () => {
      await refresh();
      showStatus((copy) => copy.status.libraryScanComplete);
    }, (copy) => copy.status.scanningLibrary);
  }, [refresh, run, showStatus]);

  const changeScope = useCallback(async (value: string) => {
    setScopeValue(value);
    const nextScope = value.startsWith("account:")
      ? { accountIds: [value.slice("account:".length)] }
      : { currency: value.slice("currency:".length) };
    scopeRef.current = nextScope;
    await run(async () => {
      const next = isTauri()
        ? await api.bootstrap(nextScope.accountIds, nextScope.currency)
        : mockSnapshot;
      setSnapshot(next);
      showStatus(() => `[VIEWING ${value.startsWith("account:") ? next.accounts.find((account) => account.id === nextScope.accountIds?.[0])?.nickname || "ACCOUNT" : `${next.currency} ACCOUNTS`}]`);
    }, () => "[CHANGING ACCOUNT VIEW]");
  }, [run, showStatus]);

  const updateAccount = useCallback(async (update: AccountUpdate) => {
    await run(async () => {
      if (isTauri()) await api.updateAccount(update);
      await refresh();
      showStatus(() => "[ACCOUNT PROFILE SAVED]");
    }, () => "[SAVING ACCOUNT PROFILE]");
  }, [refresh, run, showStatus]);

  const saveSmartList = useCallback(async (input: SmartListInput) => {
    await run(async () => {
      if (isTauri()) await api.saveSmartList(input);
      await refresh();
      showStatus(() => "[SMART LIST SAVED]");
    }, () => "[SAVING SMART LIST]");
  }, [refresh, run, showStatus]);

  const deleteSmartList = useCallback(async (id: string) => {
    await run(async () => {
      if (isTauri()) await api.deleteSmartList(id);
      await refresh();
      showStatus(() => "[SMART LIST DELETED]");
    }, () => "[DELETING SMART LIST]");
  }, [refresh, run, showStatus]);

  const importLibraryArtifact = useCallback(async () => {
    if (!isTauri()) {
      showStatus((copy) => copy.status.demoReportImported);
      return;
    }
    const selected = await open({
      title: L.dialogs.importReportTitle,
      multiple: false,
      directory: false,
      filters: [{ name: L.dialogs.reportFilter, extensions: ["json"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    await run(async () => {
      const result = await api.importArtifact(selected);
      await refresh();
      showStatus((copy) => copy.status.reportImported(result.item?.title ?? copy.values.ready));
    }, (copy) => copy.status.validatingReport);
  }, [L, refresh, run, showStatus]);

  const readArtifact = useCallback(async (path: string) => {
    if (!isTauri()) {
      return JSON.stringify({
        schemaVersion: 1,
        sourceApp: "gatorFinance",
        engineVersion: mockSnapshot.health.engineVersion,
        privacy: "summary_only",
        payload: { health: mockSnapshot.health, categories: mockSnapshot.categories },
      }, null, 2);
    }
    return api.artifactText(path);
  }, []);

  const copyArtifact = useCallback(async (path: string) => {
    await run(async () => {
      const text = await readArtifact(path);
      if (isTauri()) await writeText(text);
      else await navigator.clipboard?.writeText(text);
      showStatus((copy) => copy.status.reportCopied);
    }, (copy) => copy.status.copyingReport);
  }, [readArtifact, run, showStatus]);

  const revealPath = useCallback(async (path: string) => {
    if (!isTauri()) {
      showStatus((copy) => copy.status.demoPath(path));
      return;
    }
    try {
      if (/\.[^/]+$/.test(path)) await revealItemInDir(path);
      else await openPath(path);
      showStatus((copy) => copy.status.openedFinder);
    } catch (error) {
      showStatus((copy) => copy.status.error(errorMessage(error)));
    }
  }, [showStatus]);

  const deleteArtifact = useCallback(async (path: string) => {
    const confirmed = !isTauri() || await askConfirm(
      L.dialogs.trashMessage,
      { title: L.dialogs.trashTitle, kind: "warning", okLabel: L.dialogs.moveToTrash, cancelLabel: L.dialogs.cancel },
    );
    if (!confirmed) return;
    await run(async () => {
      if (isTauri()) await api.trashArtifact(path);
      await refresh();
      showStatus((copy) => copy.status.movedToTrash);
    }, (copy) => copy.status.movingToTrash);
  }, [L, refresh, run, showStatus]);

  const shareArtifact = useCallback(async (path: string, provider: "chatgpt" | "claude") => {
    const providerName = provider === "chatgpt" ? "ChatGPT" : "Claude";
    let confirmed = true;
    try {
      confirmed = !isTauri() || await askConfirm(
        L.dialogs.shareMessage(providerName),
        { title: L.dialogs.shareTitle(providerName), kind: "info", okLabel: L.dialogs.reviewInBrowser, cancelLabel: L.dialogs.cancel },
      );
    } catch (error) {
      showStatus((copy) => copy.status.error(errorMessage(error)));
      return;
    }
    if (!confirmed) return;
    await run(async () => {
      if (!isTauri()) {
        showStatus((copy) => copy.status.demoShareReady(providerName));
        return;
      }
      const result = await api.shareArtifact(path, provider);
      await writeText(result.text);
      await openUrl(result.url);
      showStatus((copy) => copy.status.summaryCopied(providerName));
    }, (copy) => copy.status.preparingSummary);
  }, [L, run, showStatus]);

  if (!snapshot) {
    return <div className="app-loading"><LoaderCircle size={28} /><span>{status}</span></div>;
  }

  return (
    <div className={`app-shell ${dragActive ? "drag-active" : ""}`}>
      <header className="app-header">
        <button className="mobile-menu-button" aria-label={L.header.openNavigation} onClick={() => setMenuOpen(true)}><Menu size={19} /></button>
        <button className="brand" onClick={() => setWorkspace("overview")} aria-label={L.header.openOverview}>
          <img src={resolvedTheme === "dark" ? appIconDark : appIconDefault} alt="" />
          <span><strong>gatorFinance</strong><small>{L.header.subtitle}</small></span>
        </button>
        <nav className={menuOpen ? "open" : ""} aria-label={L.header.primaryNavigation}>
          <button className="nav-close" aria-label={L.header.closeNavigation} onClick={() => setMenuOpen(false)}><X size={19} /></button>
          {NAV_ITEMS.map(([value, Icon]) => (
            <button key={value} className={workspace === value ? "active" : ""} aria-label={L.nav[value]} title={L.nav[value]} onClick={() => { setWorkspace(value); setMenuOpen(false); }}>
              <Icon size={16} /><span>{L.nav[value]}</span>
            </button>
          ))}
        </nav>
        <div className="header-actions">
          <label className="account-scope"><span>{language === "es" ? "VISTA" : "VIEW"}</span><select value={scopeValue || `currency:${snapshot.currency}`} onChange={(event) => void changeScope(event.target.value)} disabled={busy}>
            {[...new Set(snapshot.accounts.filter((account) => !account.archived).map((account) => account.currency))].map((currency) => <option key={currency} value={`currency:${currency}`}>{language === "es" ? `Todas las cuentas ${currency}` : `All ${currency} accounts`}</option>)}
            {snapshot.accounts.filter((account) => !account.archived).map((account) => <option key={account.id} value={`account:${account.id}`}>{account.nickname} {account.maskedIdentifier || ""}</option>)}
          </select></label>
          <div className="language-switch" role="group" aria-label={L.header.language}>
            <button className={language === "en" ? "active" : ""} aria-pressed={language === "en"} onClick={() => setLanguage("en")}>EN</button>
            <button className={language === "es" ? "active" : ""} aria-pressed={language === "es"} onClick={() => setLanguage("es")}>ES</button>
          </div>
          <button
            className="icon-button quick-theme-button"
            aria-label={resolvedTheme === "dark" ? L.header.switchToLight : L.header.switchToDark}
            title={resolvedTheme === "dark" ? L.header.switchToLight : L.header.switchToDark}
            onClick={() => setTheme(resolvedTheme === "dark" ? "light" : "dark")}
          >
            {resolvedTheme === "dark" ? <Sun size={17} /> : <Moon size={17} />}
          </button>
          <button className="header-import" onClick={() => void chooseStatements()} disabled={busy}><FileUp size={16} /><span>{L.header.importOfx}</span></button>
        </div>
      </header>

      <div className="global-status" role="status">
        <span className={busy ? "status-pulse active" : "status-pulse"} />
        <span>{status}</span>
        <strong>{L.header.transactionCount(snapshot.transactionCount, snapshot.accountCount)}</strong>
      </div>

      {workspace === "overview" && <Overview snapshot={snapshot} busy={busy} status={status} onImport={() => void chooseStatements()} onSaveReport={() => setReportDialogOpen(true)} />}
      {workspace === "transactions" && <Transactions transactions={snapshot.transactions} currency={snapshot.currency} busy={busy} status={status} onUpdate={updateTransaction} />}
      {workspace === "lists" && <SmartLists lists={snapshot.smartLists} accounts={snapshot.accounts} transactions={snapshot.smartListTransactions} currency={snapshot.currency} busy={busy} onSave={saveSmartList} onDelete={deleteSmartList} />}
      {workspace === "library" && <Library library={snapshot.library} busy={busy} status={status} onRefresh={refreshLibrary} onImport={importLibraryArtifact} onRead={readArtifact} onCopy={copyArtifact} onReveal={revealPath} onDelete={deleteArtifact} onShare={shareArtifact} />}
      {workspace === "settings" && <Settings snapshot={snapshot} theme={theme} onTheme={setTheme} onReveal={revealPath} onUpdateAccount={updateAccount} />}

      {dragActive && <div className="drop-overlay"><FileUp size={34} /><strong>{L.header.dropOfx}</strong><span>{L.header.localProcessing}</span></div>}

      {importPlan && <ImportDialog plan={importPlan} accounts={snapshot.accounts} receipt={importReceipt} busy={busy} onCancel={() => { setImportPlan(null); setImportReceipt(null); }} onCommit={commitImport} />}

      {reportDialogOpen && (
        <div className="modal-backdrop" role="presentation" onMouseDown={() => setReportDialogOpen(false)}>
          <form className="report-dialog" role="dialog" aria-modal="true" aria-labelledby="report-dialog-title" onMouseDown={(event) => event.stopPropagation()} onSubmit={(event) => { event.preventDefault(); void saveReport(); }}>
            <div className="dialog-heading"><div><span className="eyebrow">{L.dialogs.outputSummary}</span><h2 id="report-dialog-title">{L.dialogs.saveHealthReport}</h2></div><button type="button" className="icon-button" aria-label={L.dialogs.closeDialog} onClick={() => setReportDialogOpen(false)}><X size={18} /></button></div>
            <label className="field-label">{L.dialogs.reportTitle}<input autoFocus value={reportTitle} onChange={(event) => setReportTitle(event.target.value)} maxLength={90} /></label>
            <div className="dialog-contract"><span>{L.dialogs.included}</span><strong>{L.dialogs.includedDetail}</strong><span>{L.dialogs.excluded}</span><strong>{L.dialogs.excludedDetail}</strong></div>
            <div className="dialog-actions"><button type="button" className="secondary-button" onClick={() => setReportDialogOpen(false)}>{L.dialogs.cancel.toUpperCase()}</button><button className="primary-button" disabled={busy || !reportTitle.trim()}><Save size={16} /> {L.dialogs.saveReport}</button></div>
          </form>
        </div>
      )}
    </div>
  );
}

export default App;
