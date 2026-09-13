import { Bot, Clipboard, FileDown, FolderOpen, MessageSquareText, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { dateLabel, periodLabel, scoreTone } from "../format";
import { useI18n } from "../i18n";
import type { LibraryScan } from "../types";

interface LibraryProps {
  library: LibraryScan;
  busy: boolean;
  status: string;
  onRefresh: () => Promise<void>;
  onImport: () => Promise<void>;
  onRead: (path: string) => Promise<string>;
  onCopy: (path: string) => Promise<void>;
  onReveal: (path: string) => Promise<void>;
  onDelete: (path: string) => Promise<void>;
  onShare: (path: string, provider: "chatgpt" | "claude") => Promise<void>;
}

export function Library({ library, busy, status, onRefresh, onImport, onRead, onCopy, onReveal, onDelete, onShare }: LibraryProps) {
  const { L } = useI18n();
  const [selectedId, setSelectedId] = useState<string | null>(library.items[0]?.artifactId ?? null);
  const [preview, setPreview] = useState("");
  const selected = library.items.find((item) => item.artifactId === selectedId) ?? library.items[0] ?? null;

  useEffect(() => {
    if (!selected) {
      setPreview("");
      return;
    }
    void onRead(selected.path).then(setPreview).catch(() => setPreview(L.library.previewUnavailable));
  }, [L, onRead, selected]);

  const parsed = useMemo(() => {
    try { return JSON.parse(preview); } catch { return null; }
  }, [preview]);

  return (
    <main className="workspace library-workspace">
      <header className="workspace-title-row">
        <div><span className="eyebrow">{L.library.eyebrow(library.items.length)}</span><h1>{L.library.title}</h1></div>
        <div className="title-actions">
          <button className="icon-button" aria-label={L.library.rescan} title={L.library.rescan} onClick={() => void onRefresh()} disabled={busy}><RefreshCw size={18} /></button>
          <button className="secondary-button" onClick={() => void onImport()} disabled={busy}><FileDown size={16} /> {L.library.importReport}</button>
        </div>
      </header>

      {(library.skippedInvalid > 0 || library.duplicatePayloads > 0 || status) && (
        <div className="library-status">
          <span>{status || L.library.scanComplete}</span>
          <span>{L.library.invalidSkipped(library.skippedInvalid)}</span>
          <span>{L.library.duplicatePayloads(library.duplicatePayloads)}</span>
        </div>
      )}

      <div className="library-layout">
        <aside className="library-list">
          {library.items.map((item) => (
            <button key={item.artifactId} className={item.artifactId === selected?.artifactId ? "active" : ""} onClick={() => setSelectedId(item.artifactId)}>
              <span className={`library-score ${scoreTone(item.score)}`}>{item.score.toFixed(1)}</span>
              <span className="library-item-copy"><strong>{item.title}</strong><small>{periodLabel(item.periodStart, item.periodEnd, L.locale, L.library.noPeriod)}</small><small>{dateLabel(item.createdAt, L.locale)}</small></span>
              <span className="library-grade">{item.grade}</span>
            </button>
          ))}
          {library.items.length === 0 && <div className="library-empty"><span>{L.library.empty}</span><small>{L.library.emptyDetail}</small></div>}
        </aside>

        <section className="library-preview">
          {selected ? (
            <>
              <div className="preview-heading">
                <div><span className="eyebrow">{L.library.reportEyebrow}</span><h2>{selected.title}</h2></div>
                <div className={`preview-score ${scoreTone(selected.score)}`}>{selected.score.toFixed(1)}<small>/100</small></div>
              </div>
              <div className="preview-metrics">
                <span>{L.library.period}<strong>{periodLabel(selected.periodStart, selected.periodEnd, L.locale, L.library.noPeriod)}</strong></span>
                <span>{L.library.engine}<strong>{parsed?.engineVersion || parsed?.payload?.health?.engineVersion || "0.4"}</strong></span>
                <span>{L.library.privacy}<strong>{L.library.summaryOnly}</strong></span>
              </div>
              <pre className="artifact-preview">{preview || L.library.loadingPreview}</pre>
              <div className="preview-actions">
                <button className="icon-text-button" onClick={() => void onCopy(selected.path)} disabled={busy}><Clipboard size={16} /> {L.library.copy}</button>
                <button className="icon-text-button" onClick={() => void onReveal(selected.path)} disabled={busy}><FolderOpen size={16} /> {L.library.reveal}</button>
                <button className="icon-text-button" onClick={() => void onShare(selected.path, "chatgpt")} disabled={busy}><MessageSquareText size={16} /> CHATGPT</button>
                <button className="icon-text-button" onClick={() => void onShare(selected.path, "claude")} disabled={busy}><Bot size={16} /> CLAUDE</button>
                <button className="destructive-button" onClick={() => void onDelete(selected.path)} disabled={busy}><Trash2 size={16} /> {L.library.trash}</button>
              </div>
            </>
          ) : <div className="preview-empty"><span>{L.library.noneSelected}</span></div>}
        </section>
      </div>
    </main>
  );
}
