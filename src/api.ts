import { invoke } from "@tauri-apps/api/core";
import type {
  ArtifactActionResult,
  AccountProfile,
  AccountUpdate,
  DashboardSnapshot,
  ImportAssignment,
  ImportPlan,
  ImportReceipt,
  LibraryScan,
  SharePayload,
  SmartList,
  SmartListInput,
  TransactionUpdate,
} from "./types";

export const isTauri = () => "__TAURI_INTERNALS__" in window;

export const api = {
  bootstrap: (accountIds?: string[], currency?: string) =>
    invoke<DashboardSnapshot>("bootstrap", { accountIds, currency }),
  inspectStatements: (paths: string[]) =>
    invoke<ImportPlan>("inspect_statements", { paths }),
  commitImport: (planId: string, assignments: ImportAssignment[]) =>
    invoke<ImportReceipt>("commit_import", { planId, assignments }),
  updateTransaction: (update: TransactionUpdate) =>
    invoke<void>("update_transaction", { update }),
  updateAccount: (update: AccountUpdate) =>
    invoke<AccountProfile>("update_account", { update }),
  saveSmartList: (input: SmartListInput) =>
    invoke<SmartList>("save_smart_list", { input }),
  deleteSmartList: (id: string) => invoke<void>("delete_smart_list", { id }),
  refreshLibrary: () => invoke<LibraryScan>("refresh_library"),
  saveReport: (title: string, accountIds: string[], currency: string) =>
    invoke<ArtifactActionResult>("save_report", { title, accountIds, currency }),
  importArtifact: (path: string) =>
    invoke<ArtifactActionResult>("import_artifact", { path }),
  trashArtifact: (path: string) =>
    invoke<ArtifactActionResult>("trash_artifact", { path }),
  artifactText: (path: string) => invoke<string>("artifact_text", { path }),
  shareArtifact: (path: string, provider: "chatgpt" | "claude") =>
    invoke<SharePayload>("share_artifact", { path, provider }),
};
