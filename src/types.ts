export type TransactionType = "credit" | "debit";

export interface TransactionRecord {
  dedupeHash: string;
  uniqueId: string;
  date: string;
  dateRaw: string;
  amountMinor: number;
  currency: string;
  transactionType: TransactionType;
  categoryAuto: string;
  categoryConfidence: number;
  isRecurring: boolean | null;
  vendorRaw: string;
  vendorClean: string;
  vendorNormalized: string;
  cardMask: string | null;
  categoryUser: string | null;
  vendorUser: string | null;
  notesUser: string | null;
  tagsUser: string[];
  isExcluded: boolean;
  sourceFile: string;
  accountLabel: string | null;
  accountProfileId: string | null;
}

export interface MonthlySnapshot {
  year: number;
  month: number;
  totalCreditsMinor: number;
  totalDebitsMinor: number;
  netFlowMinor: number;
  recurringCreditsMinor: number;
  recurringDebitsMinor: number;
  transactionCount: number;
  commitmentRatio: number;
  categoryTotalsMinor: Record<string, number>;
}

export interface HealthResult {
  engineVersion: string;
  score: number;
  grade: string;
  components: {
    netFlowScore: number;
    stabilityScore: number;
    trendScore: number;
    savingsScore: number;
    volatilityPenalty: number;
    overcommitmentPenalty: number;
  };
  avgMonthlyNetFlowMinor: number;
  avgMonthlyIncomeMinor: number;
  avgMonthlyExpensesMinor: number;
  trendDirection: string;
  trendSlopeMinor: number;
  volatility: number;
  monthsAnalyzed: number;
  periodStart: string | null;
  periodEnd: string | null;
  monthlySnapshots: MonthlySnapshot[];
  bestMonth: string | null;
  worstMonth: string | null;
}

export interface CategoryTotal {
  category: string;
  amountMinor: number;
  count: number;
  percentage: number;
}

export interface LibraryItem {
  artifactId: string;
  title: string;
  createdAt: string;
  periodLabel: string;
  periodStart: string | null;
  periodEnd: string | null;
  score: number;
  grade: string;
  path: string;
  payloadHash: string;
}

export interface LibraryScan {
  items: LibraryItem[];
  skippedInvalid: number;
  duplicatePayloads: number;
}

export interface DashboardSnapshot {
  health: HealthResult;
  transactions: TransactionRecord[];
  smartListTransactions: TransactionRecord[];
  categories: CategoryTotal[];
  transactionCount: number;
  accountCount: number;
  currency: string;
  accounts: AccountProfile[];
  selectedAccountIds: string[];
  smartLists: SmartList[];
  library: LibraryScan;
  paths: {
    database: string;
    library: string;
    legacyDatabase: string;
  };
  migration: {
    legacyFound: boolean;
    migrationPerformed: boolean;
    importedTransactions: number;
    message: string;
  };
}

export interface AccountProfile {
  id: string;
  nickname: string;
  institutionName: string | null;
  displayAccountType: string | null;
  sourceAccountType: string | null;
  currency: string;
  maskedIdentifier: string | null;
  createdAt: string;
  updatedAt: string;
  archived: boolean;
}

export interface AccountUpdate {
  id: string;
  nickname: string;
  displayAccountType: string | null;
  archived: boolean;
}

export interface DetectedStatement {
  detectedId: string;
  sourceFile: string;
  institutionName: string | null;
  maskedIdentifier: string | null;
  sourceAccountType: string | null;
  currency: string;
  periodStart: string | null;
  periodEnd: string | null;
  transactionCount: number;
  matchedAccountId: string | null;
  identityReliable: boolean;
  suggestedNickname: string;
}

export interface ImportPlan {
  planId: string;
  statements: DetectedStatement[];
  errors: string[];
}

export interface ImportAssignment {
  detectedId: string;
  accountProfileId: string | null;
  newAccount: { nickname: string; displayAccountType: string | null } | null;
}

export interface ImportReceipt {
  batchId: string;
  files: Array<{
    sourceFile: string;
    accountProfileId: string;
    newTransactions: number;
    duplicateTransactions: number;
    conflictTransactions: number;
    totalTransactions: number;
  }>;
  errors: string[];
  newTransactions: number;
  duplicateTransactions: number;
  conflictTransactions: number;
}

export interface SmartListRules {
  sourceQuery: string;
  sourceMatch: "contains" | "exact" | "starts_with";
  direction: "all" | TransactionType;
  accountProfileIds: string[];
  category: string | null;
  tag: string | null;
  dateStart: string | null;
  dateEnd: string | null;
  minAmountMinor: number | null;
  maxAmountMinor: number | null;
}

export interface SmartList {
  id: string;
  name: string;
  rules: SmartListRules;
  createdAt: string;
  updatedAt: string;
}

export interface SmartListInput {
  id: string | null;
  name: string;
  rules: SmartListRules;
}

export interface TransactionUpdate {
  dedupeHash: string;
  category: string | null;
  vendor: string | null;
  notes: string | null;
  tags: string[];
  isExcluded: boolean;
}

export interface ArtifactActionResult {
  message: string;
  path: string | null;
  item: LibraryItem | null;
}

export interface SharePayload {
  url: string;
  text: string;
}

export type Workspace = "overview" | "transactions" | "lists" | "library" | "settings";
export type ThemeMode = "dark" | "light" | "system";
