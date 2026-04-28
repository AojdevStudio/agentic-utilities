import * as crypto from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";

export type WorkflowLane = "planning" | "architecture";
export type WorkflowStatus = "active" | "blocked" | "ready" | "done" | "cancelled";
export type IssueType = "AFK" | "HITL";
export type IssuePriority = "P0" | "P1" | "P2" | "P3";
export type EvidenceKind = "red" | "green" | "refactor" | "visual" | "review" | "log" | "diff" | "risk";
export type RunnerFailureReason = "command-failed" | "timeout" | "missing-evidence" | "review-blocked" | "integration-failed" | "cancelled";
export type PlanningPhase =
  | "intake"
  | "discovery"
  | "grill"
  | "prd-draft"
  | "glossary-draft"
  | "agreement"
  | "issue-approval"
  | "issues-created"
  | "triage"
  | "execution-approval"
  | "ready-to-execute"
  | "done"
  | "blocked";
export type ArchitecturePhase =
  | "intake"
  | "explore"
  | "candidates"
  | "interface-design"
  | "refactor-rfc"
  | "issue-approval"
  | "triage"
  | "execution-approval"
  | "ready-to-execute"
  | "done"
  | "blocked";
export type WorkflowPhase = PlanningPhase | ArchitecturePhase;

export type IntakeKind =
  | "freeform"
  | "repo"
  | "github"
  | "linear"
  | "local_plan"
  | "opensrc_package"
  | "opensrc_repo"
  | "web";

export type ApprovalGate = "before-issues" | "before-execution";

export type WorkflowSource = {
  kind: IntakeKind;
  raw: string;
  title: string;
  reference?: string;
  path?: string;
  url?: string;
};

export type GateState = {
  status: "pending" | "approved" | "rejected";
  approvedAt?: string;
  approvedBy?: string;
  approvalFile?: string;
  note?: string;
};

export type WorkflowState = {
  version: 2;
  workflowId: string;
  lane: WorkflowLane;
  repoCwd: string;
  status: WorkflowStatus;
  phase: WorkflowPhase;
  createdAt: string;
  updatedAt: string;
  source: WorkflowSource;
  paths: {
    root: string;
    workflowDir: string;
    artifactsDir: string;
    approvalsDir: string;
    issuesDir: string;
    triageDir: string;
    stateFile: string;
    eventsFile: string;
  };
  agreement: {
    prd: "missing" | "drafted" | "approved";
    glossary: "missing" | "drafted" | "approved";
    acceptance: "missing" | "explicit";
    verification: "missing" | "chosen";
    modulesInterfaces: "missing" | "named";
    hitlAfk: "missing" | "labeled";
  };
  gates: Record<ApprovalGate, GateState>;
  queue: {
    status: "missing" | "drafted" | "triaged" | "approved";
    itemCount: number;
    readyCount: number;
    queueFile: string;
  };
  notes?: string[];
};

export type WorkflowEvent = {
  version: 2;
  eventId: string;
  workflowId: string;
  at: string;
  type: string;
  phase?: WorkflowPhase;
  data?: Record<string, unknown>;
};

export type ExecutionEvidence = {
  kind: EvidenceKind;
  path?: string;
  command?: string;
  summary: string;
  exitCode?: number;
  createdAt: string;
};

export type IssueExecutionState = {
  version: 2;
  issueId: string;
  workflowId?: string;
  tracker?: "github" | "linear" | "local";
  trackerRef?: string;
  status: "queued" | "claimed" | "running" | "blocked" | "pr-open" | "done";
  phase:
    | "queued"
    | "claimed"
    | "worktree"
    | "tdd"
    | "verification"
    | "review"
    | "repair"
    | "integration"
    | "debug"
    | "pr"
    | "awaiting-merge"
    | "done"
    | "blocked";
  repoCwd: string;
  createdAt: string;
  updatedAt: string;
  manifestPath?: string;
  worktreePath?: string;
  branch?: string;
  prUrl?: string;
  evidence?: ExecutionEvidence[];
  verification?: {
    redTestObserved: boolean;
    checksPassed: boolean;
    evidence: string[];
    recordedAt: string;
  };
  review?: {
    status: "pending" | "passed" | "blocked" | "failed";
    findings: string[];
    reportPath?: string;
    recordedAt: string;
  };
  worker?: {
    command: string;
    cwd: string;
    logPath?: string;
    exitCode: number;
    failureReason?: RunnerFailureReason;
    startedAt: string;
    finishedAt: string;
  };
  repair?: {
    attempts: number;
    maxAttempts: number;
    findings: string[];
    status: "passed" | "blocked";
    recordedAt: string;
  };
  integration?: {
    branch: string;
    status: "passed" | "blocked";
    recordedAt: string;
  };
};

export type WorkflowTransitionOptions = {
  actor?: string;
  note?: string;
  evidencePaths?: string[];
  force?: boolean;
  status?: WorkflowStatus;
};

export type WorkflowTransitionValidation = {
  ok: boolean;
  from: WorkflowPhase;
  to: WorkflowPhase;
  reasons: string[];
  allowedTargets: WorkflowPhase[];
};

export type WorkflowTransitionResult =
  | { ok: true; state: WorkflowState; validation: WorkflowTransitionValidation }
  | { ok: false; state: WorkflowState; validation: WorkflowTransitionValidation };

export type TrackerIssueCreationOptions = {
  dryRun?: boolean;
  tracker?: "local";
  archiveDrafts?: boolean;
  now?: string;
};

export type TrackerIssueOperation = {
  kind: "parent" | "child";
  issueId?: string;
  title: string;
  tracker: "local";
  trackerRef: string;
  draft?: string;
  blockedBy?: string[];
  sliceKind?: "vertical" | "horizontal-justified" | "unspecified";
};

export type TrackerIssueCreationResult = {
  dryRun: boolean;
  tracker: "local";
  parentTrackerRef: string;
  operations: TrackerIssueOperation[];
  createdDir: string;
  queueFile: string;
};

export type ExecutionContextPacket = {
  version: 2;
  workflowId: string;
  issueId: string;
  generatedAt: string;
  issue: Record<string, unknown>;
  blockers: Array<Record<string, unknown>>;
  prdExcerpt: string;
  glossaryExcerpt: string;
  sourceNotes: Array<{ path: string; excerpt: string }>;
  moduleInterfaceMap: string;
  checks: string;
  recentEvidence: Array<Record<string, unknown>>;
  constraints: string[];
};

export type ExecutionClaimOptions = {
  issueId?: string;
  now?: string;
  actor?: string;
  mode?: "local" | "git-worktree";
};

export type ExecutionClaimResult = {
  state: WorkflowState;
  issue: any;
  executionState: IssueExecutionState;
  contextPacket: ExecutionContextPacket;
  contextPacketPath: string;
  implementationPromptPath: string;
  reviewPromptPath: string;
};

export type ExecutionFailureOptions = {
  ambiguous?: boolean;
  designFriction?: boolean;
  nonAmbiguous?: boolean;
  message: string;
  now?: string;
};

export type ExecutionVerificationOptions = {
  redTestObserved: boolean;
  checksPassed: boolean;
  evidence: string[];
  redEvidence?: string[];
  greenEvidence?: string[];
  refactorNotes?: string[];
  commands?: string[];
  evidencePaths?: string[];
  visualEvidence?: string[];
  now?: string;
};

export type FreshReviewRecordOptions = {
  status: "passed" | "blocked" | "failed";
  findings: string[];
  reportPath?: string;
  now?: string;
};

export type PullRequestRecordOptions = {
  title?: string;
  body?: string;
  now?: string;
};

export type NativeRunnerConfig = {
  commandTemplate: string;
  concurrency: number;
  maxRepairAttempts: number;
  idleTimeoutSeconds: number;
  envAllowlist: string[];
  evidenceProfile: "concise" | "full";
};

export type WorkerCommandContext = {
  repoCwd: string;
  worktreePath: string;
  promptPath: string;
  workflowId: string;
  issueId: string;
};

export type WorkerCommandResult = {
  command: string;
  cwd: string;
  startedAt: string;
  finishedAt: string;
  exitCode: number;
  stdout: string;
  stderr: string;
  logPath?: string;
  failureReason?: RunnerFailureReason;
};

export type QAHandoff = {
  version: 2;
  workflowId: string;
  generatedAt: string;
  summary: string;
  highlights: {
    tddEvidence: string[];
    visualEvidence: string[];
    openRisks: string[];
  };
  audit: {
    logs: string[];
    diffs: string[];
    reviewerReports: string[];
    commands: string[];
  };
};

export type SliceComputedState = "done" | "not-done" | "queued" | "claimed" | "running" | "blocked" | "pr-open";

export type SliceStatusRow = {
  id: string;
  title: string;
  queueStatus: string;
  computedState: SliceComputedState;
  evidenceHealth: "complete" | "missing" | "not-required";
  missingEvidence: string[];
  evidencePaths: string[];
  trackerRef?: string;
  prRef?: string;
  lastUpdated?: string;
};

export type WorkflowStatusReport = {
  version: 2;
  workflowId: string;
  lane: WorkflowLane;
  phase: WorkflowPhase;
  status: WorkflowStatus;
  sourceTitle: string;
  generatedAt: string;
  slices: SliceStatusRow[];
  counts: {
    total: number;
    done: number;
    notDone: number;
    queued: number;
    running: number;
    blocked: number;
    prOpen: number;
    missingEvidence: number;
  };
};

export type WorkflowStatusOverviewRow = {
  workflowId: string;
  phase: WorkflowPhase;
  status: WorkflowStatus;
  total: number;
  done: number;
  notDone: number;
  blocked: number;
};

export type WorkflowCreateOptions = {
  repoCwd: string;
  lane: WorkflowLane;
  rawInput: string;
  source?: Partial<WorkflowSource>;
};

const WORKFLOWS_DIR = "workflows";
const ISSUES_DIR = "issues";
const APPROVALS_DIR = "approvals";

const PLANNING_PHASES: PlanningPhase[] = [
  "intake",
  "discovery",
  "grill",
  "prd-draft",
  "glossary-draft",
  "agreement",
  "issue-approval",
  "issues-created",
  "triage",
  "execution-approval",
  "ready-to-execute",
  "done",
  "blocked",
];

const ARCHITECTURE_PHASES: ArchitecturePhase[] = [
  "intake",
  "explore",
  "candidates",
  "interface-design",
  "refactor-rfc",
  "issue-approval",
  "triage",
  "execution-approval",
  "ready-to-execute",
  "done",
  "blocked",
];

const PLANNING_TRANSITIONS: Record<PlanningPhase, WorkflowPhase[]> = {
  intake: ["discovery", "grill", "prd-draft", "blocked"],
  discovery: ["grill", "prd-draft", "blocked"],
  grill: ["discovery", "prd-draft", "blocked"],
  "prd-draft": ["glossary-draft", "agreement", "blocked"],
  "glossary-draft": ["agreement", "blocked"],
  agreement: ["issue-approval", "issues-created", "blocked"],
  "issue-approval": ["issues-created", "blocked"],
  "issues-created": ["triage", "execution-approval", "ready-to-execute", "blocked"],
  triage: ["execution-approval", "ready-to-execute", "blocked"],
  "execution-approval": ["ready-to-execute", "blocked"],
  "ready-to-execute": ["done", "blocked"],
  done: [],
  blocked: ["discovery", "grill", "agreement", "issue-approval", "execution-approval"],
};

const ARCHITECTURE_TRANSITIONS: Record<ArchitecturePhase, WorkflowPhase[]> = {
  intake: ["explore", "blocked"],
  explore: ["candidates", "blocked"],
  candidates: ["interface-design", "refactor-rfc", "blocked"],
  "interface-design": ["refactor-rfc", "blocked"],
  "refactor-rfc": ["issue-approval", "blocked"],
  "issue-approval": ["triage", "blocked"],
  triage: ["execution-approval", "ready-to-execute", "blocked"],
  "execution-approval": ["ready-to-execute", "blocked"],
  "ready-to-execute": ["done", "blocked"],
  done: [],
  blocked: ["explore", "candidates", "interface-design", "refactor-rfc", "issue-approval", "execution-approval"],
};

const PRE_ARTIFACT_PLANNING_PHASES = new Set<WorkflowPhase>(["intake", "discovery", "grill"]);
const LOCKED_PLANNING_ARTIFACT_PATTERNS = [
  /(?:^|\/)artifacts\/prd\.draft\.md$/,
  /(?:^|\/)artifacts\/ubiquitous-language\.draft\.md$/,
  /(?:^|\/)issues\/drafts(?:\/|$)/,
  /(?:^|\/)triage\/queue\.json$/,
  /(?:^|\/)issues\/parent-prd-issue\.json$/,
  /(?:^|\/)issues\/created(?:\/|$)/,
];

export function isPreArtifactPlanningPhase(phase: WorkflowPhase): boolean {
  return PRE_ARTIFACT_PLANNING_PHASES.has(phase);
}

export function isPlanningArtifactLockedPath(state: WorkflowState, candidatePath: string): { locked: boolean; reason?: string } {
  if (state.lane !== "planning" || !isPreArtifactPlanningPhase(state.phase)) return { locked: false };
  const absolute = path.resolve(state.repoCwd, candidatePath.replace(/^@/, ""));
  const workflowDir = path.resolve(state.paths.workflowDir);
  const relative = path.relative(workflowDir, absolute).replace(/\\/g, "/");
  if (relative.startsWith("..") || path.isAbsolute(relative)) return { locked: false };
  if (relative === "decisions.md" || relative.startsWith("artifacts/source-notes/")) return { locked: false };
  const locked = LOCKED_PLANNING_ARTIFACT_PATTERNS.some((pattern) => pattern.test(relative));
  return locked
    ? { locked: true, reason: `Autopilot planning is in ${state.phase}; PRD/glossary/issues/queue artifacts are locked until concept lock. Use decisions.md during grill.` }
    : { locked: false };
}

export function lockedPlanningArtifactPaths(state: WorkflowState): string[] {
  return [
    path.join(state.paths.artifactsDir, "prd.draft.md"),
    path.join(state.paths.artifactsDir, "ubiquitous-language.draft.md"),
    path.join(state.paths.issuesDir, "drafts"),
    state.queue.queueFile,
  ];
}

export function defaultNativeRunnerConfig(): NativeRunnerConfig {
  return {
    commandTemplate: "cd {{WORKTREE_PATH}} && pi -p @{{PROMPT_PATH}}",
    concurrency: 2,
    maxRepairAttempts: 2,
    idleTimeoutSeconds: 600,
    envAllowlist: ["PATH", "HOME", "SHELL", "TMPDIR", "PI_*", "ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GOOGLE_API_KEY", "GEMINI_API_KEY"],
    evidenceProfile: "concise",
  };
}

function shellQuote(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`;
}

export function buildWorkerCommand(template: string, context: WorkerCommandContext): string {
  const values: Record<string, string> = {
    REPO_CWD: context.repoCwd,
    WORKTREE_PATH: context.worktreePath,
    PROMPT_PATH: context.promptPath,
    WORKFLOW_ID: context.workflowId,
    ISSUE_ID: context.issueId,
  };
  return template.replace(/{{\s*([A-Z_]+)\s*}}/g, (match, key: string) => {
    const value = values[key];
    return value === undefined ? match : shellQuote(value);
  });
}

export function sortQueueItemsForExecution(items: any[]): any[] {
  const priorityRank: Record<string, number> = { P0: 0, P1: 1, P2: 2, P3: 3 };
  return queueItemsInDependencyOrder(items)
    .slice()
    .sort((a: any, b: any) => {
      const aRank = priorityRank[String(a.priority ?? "P2").toUpperCase()] ?? 2;
      const bRank = priorityRank[String(b.priority ?? "P2").toUpperCase()] ?? 2;
      if (aRank !== bRank) return aRank - bRank;
      return String(a.id ?? "").localeCompare(String(b.id ?? ""), undefined, { numeric: true });
    });
}

export function selectReadyAfkIssues(queue: any, concurrency = defaultNativeRunnerConfig().concurrency): any[] {
  const items = Array.isArray(queue?.items) ? queue.items : [];
  return sortQueueItemsForExecution(items)
    .filter((item: any) => {
      if (String(item.type ?? "AFK").toUpperCase() !== "AFK") return false;
      if (String(item.status ?? "queued") !== "queued") return false;
      const deps: string[] = Array.isArray(item.blockedBy) ? item.blockedBy.map(String) : [];
      return deps.every((dep) => items.find((candidate: any) => String(candidate.id) === dep)?.status === "done");
    })
    .slice(0, Math.max(1, concurrency));
}

export function buildQaHandoff(input: {
  workflowId: string;
  generatedAt?: string;
  issueSummaries: Array<{ issueId: string; title: string; status: string }>;
  evidence: ExecutionEvidence[];
  reviewerReports?: string[];
  diffPaths?: string[];
}): QAHandoff {
  const evidence = input.evidence;
  const byKind = (kind: EvidenceKind) => evidence.filter((item) => item.kind === kind).map((item) => item.path ?? item.summary);
  const openRisks = byKind("risk");
  return {
    version: 2,
    workflowId: input.workflowId,
    generatedAt: input.generatedAt ?? nowIso(),
    summary: `${input.issueSummaries.length} issue(s): ${input.issueSummaries.map((item) => `${item.issueId}=${item.status}`).join(", ")}`,
    highlights: {
      tddEvidence: [...byKind("red"), ...byKind("green")],
      visualEvidence: byKind("visual"),
      openRisks,
    },
    audit: {
      logs: byKind("log"),
      diffs: [...byKind("diff"), ...(input.diffPaths ?? [])],
      reviewerReports: [...byKind("review"), ...(input.reviewerReports ?? [])],
      commands: evidence.filter((item) => item.command).map((item) => item.command!),
    },
  };
}

function countStatusRows(rows: SliceStatusRow[]): WorkflowStatusReport["counts"] {
  return {
    total: rows.length,
    done: rows.filter((row) => row.computedState === "done").length,
    notDone: rows.filter((row) => row.computedState === "not-done").length,
    queued: rows.filter((row) => row.computedState === "queued").length,
    running: rows.filter((row) => row.computedState === "running" || row.computedState === "claimed").length,
    blocked: rows.filter((row) => row.computedState === "blocked").length,
    prOpen: rows.filter((row) => row.computedState === "pr-open").length,
    missingEvidence: rows.filter((row) => row.missingEvidence.length > 0).length,
  };
}

function existingWorkflowEvidencePaths(state: WorkflowState, rawPaths: unknown): { existing: string[]; missing: string[] } {
  const values = Array.isArray(rawPaths) ? rawPaths.map(String).filter(Boolean) : [];
  const existing: string[] = [];
  const missing: string[] = [];
  for (const value of values) {
    const absolute = workflowPath(state, value);
    if (fs.existsSync(absolute)) existing.push(value);
    else missing.push(value);
  }
  return { existing, missing };
}

function sliceComputedState(queueStatus: string, missingEvidence: string[]): SliceComputedState {
  if (queueStatus === "done") return missingEvidence.length ? "not-done" : "done";
  if (queueStatus === "blocked") return "blocked";
  if (queueStatus === "running") return "running";
  if (queueStatus === "claimed") return "claimed";
  if (queueStatus === "pr-open") return "pr-open";
  return "queued";
}

export function buildWorkflowStatusReport(state: WorkflowState, generatedAt = nowIso()): WorkflowStatusReport {
  const queue = readJsonOptional(state.queue.queueFile);
  const items = Array.isArray(queue?.items) ? queue.items : [];
  const slices: SliceStatusRow[] = items.map((item: any) => {
    const id = String(item.id ?? "<missing>");
    const queueStatus = String(item.status ?? "queued");
    const executionRecord = readJsonOptional(path.join(state.paths.issuesDir, "execution", `${id}.json`));
    const prRecord = readJsonOptional(path.join(state.paths.issuesDir, "prs", `${id}.json`));
    const evidence = existingWorkflowEvidencePaths(state, item.evidencePaths);
    const missingEvidence: string[] = [];
    if (queueStatus === "done") {
      if (!Array.isArray(item.evidencePaths) || item.evidencePaths.length === 0) missingEvidence.push("queue evidencePaths");
      for (const missingPath of evidence.missing) missingEvidence.push(`evidence file: ${missingPath}`);
      if (!executionRecord) missingEvidence.push(`issues/execution/${id}.json`);
      if (!prRecord) missingEvidence.push(`issues/prs/${id}.json`);
    }
    return {
      id,
      title: String(item.title ?? id),
      queueStatus,
      computedState: sliceComputedState(queueStatus, missingEvidence),
      evidenceHealth: queueStatus === "done" ? missingEvidence.length ? "missing" : "complete" : "not-required",
      missingEvidence,
      evidencePaths: evidence.existing,
      trackerRef: typeof item.trackerRef === "string" ? item.trackerRef : undefined,
      prRef: typeof item.prUrl === "string" ? item.prUrl : typeof executionRecord?.prUrl === "string" ? executionRecord.prUrl : typeof prRecord?.prUrl === "string" ? prRecord.prUrl : undefined,
      lastUpdated: String(item.completedAt ?? item.settledAt ?? item.updatedAt ?? executionRecord?.updatedAt ?? state.updatedAt ?? ""),
    };
  });
  return {
    version: 2,
    workflowId: state.workflowId,
    lane: state.lane,
    phase: state.phase,
    status: state.status,
    sourceTitle: state.source.title,
    generatedAt,
    slices,
    counts: countStatusRows(slices),
  };
}

export function buildWorkflowStatusOverview(workflows: WorkflowState[]): WorkflowStatusOverviewRow[] {
  return workflows.map((workflow) => {
    const report = buildWorkflowStatusReport(workflow);
    return {
      workflowId: workflow.workflowId,
      phase: workflow.phase,
      status: workflow.status,
      total: report.counts.total,
      done: report.counts.done,
      notDone: report.counts.notDone + report.counts.queued + report.counts.running + report.counts.prOpen,
      blocked: report.counts.blocked,
    };
  });
}

export function formatWorkflowStatusReport(report: WorkflowStatusReport, overview: WorkflowStatusOverviewRow[] = []): string {
  const lines: string[] = [
    `Autopilot status: ${report.workflowId}`,
    `phase=${report.phase} status=${report.status} slices=${report.counts.done}/${report.counts.total} done not-done=${report.counts.notDone} blocked=${report.counts.blocked}`,
  ];
  if (!report.slices.length) {
    lines.push("slices: none");
  } else {
    lines.push("slices:");
    for (const row of report.slices) {
      const pr = row.prRef ? ` pr=${row.prRef}` : " pr=-";
      const updated = row.lastUpdated ? ` updated=${row.lastUpdated}` : "";
      const evidence = row.evidenceHealth === "missing" ? `missing ${row.missingEvidence.join(", ")}` : row.evidenceHealth;
      lines.push(`- ${row.id} ${row.computedState} evidence=${evidence}${pr}${updated} — ${row.title}`);
    }
  }
  if (overview.length) {
    lines.push("workflows:");
    for (const row of overview) {
      lines.push(`- ${row.workflowId} ${row.status}/${row.phase} done=${row.done}/${row.total} not-done=${row.notDone} blocked=${row.blocked}`);
    }
  }
  return lines.join("\n");
}

export function nowIso(): string {
  return new Date().toISOString();
}

export function slugify(input: string): string {
  return input
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function shortHash(input: string, length = 10): string {
  return crypto.createHash("sha256").update(input).digest("hex").slice(0, length);
}

export function getAutopilotRoot(repoCwd: string): string {
  return path.join(repoCwd, ".pi", "autopilot");
}

export function getWorkflowDir(repoCwd: string, workflowId: string): string {
  return path.join(getAutopilotRoot(repoCwd), WORKFLOWS_DIR, workflowId);
}

export function getIssueDir(repoCwd: string, issueId: string): string {
  return path.join(getAutopilotRoot(repoCwd), ISSUES_DIR, issueId);
}

export function detectIntakeKind(repoCwd: string, rawInput: string): IntakeKind {
  const input = rawInput.trim();
  if (!input || input === "." || input === "repo") return "repo";
  const abs = path.isAbsolute(input) ? input : path.resolve(repoCwd, input);
  if (fs.existsSync(abs) && fs.statSync(abs).isFile()) return "local_plan";
  if (/^(?:https?:\/\/)?github\.com\/[^/]+\/[^/]+\/(issues|pull)\/\d+/i.test(input)) return "github";
  if (/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+#\d+$/.test(input) || /^#\d+$/.test(input)) return "github";
  if (/^(?:https?:\/\/)?linear\.app\/[^/]+\/issue\/[A-Za-z][A-Za-z0-9]+-\d+/i.test(input) || /^[A-Za-z][A-Za-z0-9]+-\d+$/.test(input)) return "linear";
  if (/^https?:\/\//i.test(input)) return "web";
  if (/^(npm:|pypi:|crates:)/i.test(input)) return "opensrc_package";
  if (/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+(?:[@#][A-Za-z0-9_.\-/]+)?$/.test(input)) return "opensrc_repo";
  return "freeform";
}

export function buildWorkflowId(rawInput: string, lane: WorkflowLane, createdAt = nowIso()): string {
  const base = slugify(rawInput).slice(0, 60) || lane;
  return `${lane}-${base}-${shortHash(`${lane}:${rawInput}:${createdAt}`, 8)}`;
}

export function createWorkflowState(options: WorkflowCreateOptions): WorkflowState {
  const createdAt = nowIso();
  const kind = options.source?.kind ?? detectIntakeKind(options.repoCwd, options.rawInput);
  const title = options.source?.title ?? deriveTitle(options.rawInput, kind);
  const workflowId = buildWorkflowId(title || options.rawInput, options.lane, createdAt);
  const root = getAutopilotRoot(options.repoCwd);
  const workflowDir = getWorkflowDir(options.repoCwd, workflowId);
  const artifactsDir = path.join(workflowDir, "artifacts");
  const approvalsDir = path.join(workflowDir, APPROVALS_DIR);
  const issuesDir = path.join(workflowDir, "issues");
  const triageDir = path.join(workflowDir, "triage");

  return {
    version: 2,
    workflowId,
    lane: options.lane,
    repoCwd: options.repoCwd,
    status: "active",
    phase: options.lane === "planning" ? "grill" : "intake",
    createdAt,
    updatedAt: createdAt,
    source: {
      kind,
      raw: options.rawInput,
      title,
      reference: options.source?.reference,
      path: options.source?.path,
      url: options.source?.url,
    },
    paths: {
      root,
      workflowDir,
      artifactsDir,
      approvalsDir,
      issuesDir,
      triageDir,
      stateFile: path.join(workflowDir, "state.json"),
      eventsFile: path.join(workflowDir, "events.jsonl"),
    },
    agreement: {
      prd: "missing",
      glossary: "missing",
      acceptance: "missing",
      verification: "missing",
      modulesInterfaces: "missing",
      hitlAfk: "missing",
    },
    gates: {
      "before-issues": { status: "pending" },
      "before-execution": { status: "pending" },
    },
    queue: {
      status: "missing",
      itemCount: 0,
      readyCount: 0,
      queueFile: path.join(triageDir, "queue.json"),
    },
    notes: [],
  };
}

export function createWorkflow(options: WorkflowCreateOptions): WorkflowState {
  const state = createWorkflowState(options);
  ensureWorkflowSkeleton(state);
  saveWorkflowState(state);
  appendWorkflowEvent(state, "workflow.created", {
    lane: state.lane,
    source: state.source,
  });
  return state;
}

export function ensureRepoV2Scaffold(repoCwd: string): void {
  const root = getAutopilotRoot(repoCwd);
  for (const dir of [
    root,
    path.join(root, WORKFLOWS_DIR),
    path.join(root, ISSUES_DIR),
    path.join(root, "runs"),
    path.join(root, "locks"),
    path.join(root, "status"),
    path.join(root, "logs"),
  ]) {
    fs.mkdirSync(dir, { recursive: true });
  }

  const checksPath = path.join(root, "checks.yml");
  if (!fs.existsSync(checksPath)) {
    fs.writeFileSync(checksPath, defaultChecksConfig(), "utf8");
  }
}

export function ensureWorkflowSkeleton(state: WorkflowState): void {
  ensureRepoV2Scaffold(state.repoCwd);
  const dirs = [
    state.paths.workflowDir,
    state.paths.artifactsDir,
    path.join(state.paths.artifactsDir, "diagrams"),
    path.join(state.paths.artifactsDir, "screenshots"),
    state.paths.approvalsDir,
    state.paths.issuesDir,
    path.join(state.paths.issuesDir, "drafts"),
    path.join(state.paths.issuesDir, "created"),
    state.paths.triageDir,
  ];
  for (const dir of dirs) fs.mkdirSync(dir, { recursive: true });

  writeIfMissing(path.join(state.paths.workflowDir, "intake.json"), `${JSON.stringify(state.source, null, 2)}\n`);
  writeIfMissing(path.join(state.paths.workflowDir, "sources.json"), `${JSON.stringify({ sources: [state.source] }, null, 2)}\n`);
  writeIfMissing(path.join(state.paths.workflowDir, "decisions.md"), decisionsTemplate(state));
  if (state.lane !== "planning" || !isPreArtifactPlanningPhase(state.phase)) {
    writeIfMissing(path.join(state.paths.artifactsDir, "prd.draft.md"), prdDraftTemplate(state));
    writeIfMissing(path.join(state.paths.artifactsDir, "ubiquitous-language.draft.md"), glossaryDraftTemplate(state));
    writeIfMissing(path.join(state.paths.issuesDir, "parent-prd-issue.json"), `${JSON.stringify({ status: "not-created", tracker: null, url: null }, null, 2)}\n`);
    writeIfMissing(state.queue.queueFile, `${JSON.stringify({ version: 2, workflowId: state.workflowId, items: [] }, null, 2)}\n`);
  }
}

export function recordConceptLock(state: WorkflowState, options: {
  summary: string;
  acceptedBy?: string;
  now?: string;
}): WorkflowState {
  if (state.lane !== "planning") throw new Error("Concept lock is only valid for planning workflows.");
  if (!isPreArtifactPlanningPhase(state.phase) && state.phase !== "prd-draft") {
    throw new Error(`Cannot record concept lock from phase ${state.phase}.`);
  }
  const now = options.now ?? nowIso();
  const next: WorkflowState = {
    ...state,
    phase: "prd-draft",
    status: state.status === "blocked" ? "active" : state.status,
    notes: [...(state.notes ?? []), `Concept locked: ${options.summary}`],
    updatedAt: now,
  };
  ensureWorkflowSkeleton(next);
  const lockPath = path.join(next.paths.artifactsDir, "concept-lock.json");
  fs.mkdirSync(path.dirname(lockPath), { recursive: true });
  fs.writeFileSync(lockPath, `${JSON.stringify({
    version: 2,
    workflowId: next.workflowId,
    summary: options.summary,
    acceptedBy: options.acceptedBy ?? "agent",
    lockedAt: now,
  }, null, 2)}\n`, "utf8");
  saveWorkflowState(next);
  appendWorkflowEvent(next, "planning.concept_locked", {
    summary: options.summary,
    acceptedBy: options.acceptedBy ?? "agent",
    conceptLockPath: path.relative(next.paths.workflowDir, lockPath),
  });
  return next;
}

export function buildArtifactDraftingPrompt(state: WorkflowState): string {
  return [
    "Concept lock is recorded. Continue artifact drafting in the same Autopilot workflow.",
    `Workflow id: ${state.workflowId}`,
    `Decisions source: ${path.join(state.paths.workflowDir, "decisions.md")}`,
    `PRD summary target: ${path.join(state.paths.artifactsDir, "prd.draft.md")}`,
    `Glossary target: ${path.join(state.paths.artifactsDir, "ubiquitous-language.draft.md")}`,
    `Issue drafts target: ${path.join(state.paths.issuesDir, "drafts")}`,
    `Queue target: ${state.queue.queueFile}`,
    "Treat the PRD as a summary of decisions.md and the locked concept, not a fresh discovery phase.",
    "After drafts pass self-check, request before-issues approval.",
  ].join("\n");
}

export function saveWorkflowState(state: WorkflowState): void {
  const next = { ...state, updatedAt: nowIso() };
  fs.mkdirSync(path.dirname(next.paths.stateFile), { recursive: true });
  fs.writeFileSync(next.paths.stateFile, `${JSON.stringify(next, null, 2)}\n`, "utf8");
}

export function appendWorkflowEvent(state: WorkflowState, type: string, data?: Record<string, unknown>): WorkflowEvent {
  const event: WorkflowEvent = {
    version: 2,
    eventId: `evt_${shortHash(`${state.workflowId}:${type}:${nowIso()}:${Math.random()}`, 12)}`,
    workflowId: state.workflowId,
    at: nowIso(),
    type,
    phase: state.phase,
    data,
  };
  fs.mkdirSync(path.dirname(state.paths.eventsFile), { recursive: true });
  fs.appendFileSync(state.paths.eventsFile, `${JSON.stringify(event)}\n`, "utf8");
  return event;
}

export function isWorkflowPhase(value: string): value is WorkflowPhase {
  return (PLANNING_PHASES as string[]).includes(value) || (ARCHITECTURE_PHASES as string[]).includes(value);
}

export function getAllowedTransitions(state: WorkflowState): WorkflowPhase[] {
  if (state.lane === "architecture") {
    return ARCHITECTURE_TRANSITIONS[state.phase as ArchitecturePhase] ?? [];
  }
  return PLANNING_TRANSITIONS[state.phase as PlanningPhase] ?? [];
}

function validateAgreementSnapshot(state: WorkflowState): string[] {
  const missing: string[] = [];
  if (state.agreement.prd === "missing") missing.push("PRD draft");
  if (state.agreement.glossary === "missing") missing.push("glossary draft");
  if (state.agreement.acceptance === "missing") missing.push("explicit acceptance criteria");
  if (state.agreement.verification === "missing") missing.push("verification choices");
  if (state.agreement.modulesInterfaces === "missing") missing.push("affected modules/interfaces");
  if (state.agreement.hitlAfk === "missing") missing.push("HITL/AFK labels");
  return missing;
}

type ArtifactValidationOptions = {
  requireExecutionEvidence?: boolean;
  evidencePaths?: string[];
};

function readOptionalText(filePath: string): string | null {
  try {
    return fs.readFileSync(filePath, "utf8");
  } catch {
    return null;
  }
}

function readJsonOptional(filePath: string): any | null {
  const raw = readOptionalText(filePath);
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function workflowPath(state: WorkflowState, maybeRelativePath: string): string {
  return path.isAbsolute(maybeRelativePath)
    ? maybeRelativePath
    : path.join(state.paths.workflowDir, maybeRelativePath);
}

function sectionBody(markdown: string, heading: string): string {
  const escaped = heading.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = markdown.match(new RegExp(`^##\\s+${escaped}\\s*\\n([\\s\\S]*?)(?=^##\\s+|(?![\\s\\S]))`, "im"));
  return match?.[1]?.trim() ?? "";
}

function isTemplateLike(value: string): boolean {
  const compact = value.replace(/[^a-z0-9]/gi, "").trim().toLowerCase();
  return !compact || compact === "tbd" || compact === "pending" || compact.includes("tbdtbd") || /^todo/.test(compact);
}

function listMarkdownFiles(dir: string): string[] {
  try {
    return fs.readdirSync(dir)
      .filter((file) => file.endsWith(".md"))
      .map((file) => path.join(dir, file));
  } catch {
    return [];
  }
}

function hasAcceptanceCriteria(markdown: string): boolean {
  return /##\s+Acceptance criteria[\s\S]*[-*]\s+\[[ xX]\]/i.test(markdown)
    || /##\s+Acceptance[\s\S]*[-*]\s+/i.test(markdown);
}

function hasHitlAfkLabel(markdown: string): boolean {
  return /^\s*Type:\s*(AFK|HITL)\s*$/im.test(markdown) || /\b(AFK|AFK slice|HITL|human in the loop)\b/i.test(markdown);
}

function hasNamedSection(markdown: string, heading: string): boolean {
  return !isTemplateLike(sectionBody(markdown, heading));
}

function isHorizontalLayerBatch(markdown: string): boolean {
  const title = markdown.match(/^#\s+(.+)$/m)?.[1] ?? "";
  const buildScope = sectionBody(markdown, "What to build");
  const text = `${title}\n${buildScope}`;
  const horizontal = /\b(schema-only|database-only|migration-only|api-only|ui-only|frontend-only|backend-only)\b/i.test(text)
    || /^(?:create|add|implement)?\s*(?:the\s*)?(?:schema|database|migration|api|frontend|ui)\s+(?:layer|only)\b/i.test(title);
  if (!horizontal) return false;
  const justified = /\b(vertical-slice rationale|horizontal-slice justification|horizontal justification|explicit horizontal|justified horizontal|tracer bullet)\b/i.test(markdown);
  return !justified;
}

function sourceNeedsSummary(source: any): boolean {
  const kind = String(source?.kind ?? "");
  const raw = String(source?.raw ?? source?.url ?? "");
  return kind === "web" || kind === "youtube-transcript" || /^https?:\/\//i.test(raw);
}

function hasSourceSummary(state: WorkflowState, source: any): boolean {
  const notesPath = source?.notesPath ?? source?.notes_path;
  if (!notesPath || typeof notesPath !== "string") return false;
  const text = readOptionalText(workflowPath(state, notesPath));
  return Boolean(text && !isTemplateLike(text) && text.trim().length >= 40);
}

function isUiFacingWorkflow(state: WorkflowState, issueDraftTexts: string[]): boolean {
  const sourceText = `${state.source.title}\n${state.source.raw}`;
  const issueTitles = issueDraftTexts
    .map((text) => text.match(/^#\s+(.+)$/m)?.[1] ?? "")
    .join("\n");
  return /\b(UI|frontend|front-end|dashboard|screen|page|component|visual design|browser view)\b/i.test(`${sourceText}\n${issueTitles}`);
}

function hasVisualEvidence(state: WorkflowState, evidencePaths: string[] = []): boolean {
  for (const evidencePath of evidencePaths) {
    if (fs.existsSync(workflowPath(state, evidencePath))) return true;
  }

  const evidenceDirs = [
    path.join(state.paths.artifactsDir, "screenshots"),
    path.join(state.paths.artifactsDir, "browser"),
    path.join(state.paths.artifactsDir, "visual-evidence"),
  ];
  return evidenceDirs.some((dir) => {
    try {
      return fs.readdirSync(dir).some((file) => !file.startsWith("."));
    } catch {
      return false;
    }
  });
}

export function validateWorkflowArtifacts(state: WorkflowState, options: ArtifactValidationOptions = {}): string[] {
  const reasons: string[] = [];
  const prdPath = path.join(state.paths.artifactsDir, "prd.draft.md");
  const prd = readOptionalText(prdPath);
  if (!prd) {
    reasons.push("PRD draft is missing.");
  } else {
    for (const heading of ["Problem Statement", "Solution", "User Stories", "Implementation Decisions", "Testing Decisions", "Out of Scope"]) {
      if (isTemplateLike(sectionBody(prd, heading))) reasons.push(`PRD draft has empty/template section: ${heading}.`);
    }
    if (!/\b(affected modules\/interfaces|modules?\b|interfaces?\b|deep module|touched interfaces?)\b/i.test(prd)) {
      reasons.push("Module/interface map is missing from PRD implementation decisions.");
    }
  }

  const glossaryPath = path.join(state.paths.artifactsDir, "ubiquitous-language.draft.md");
  const glossary = readOptionalText(glossaryPath);
  if (!glossary) {
    reasons.push("Glossary draft is missing.");
  } else {
    const termRows = glossary.split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line.startsWith("|") && !/^\|\s*-/.test(line) && !/\bTerm\b.*\bDefinition\b/i.test(line));
    if (!termRows.length || termRows.some((row) => /\|\s*TBD\s*\|\s*TBD\s*\|/i.test(row))) {
      reasons.push("Glossary draft has empty/template terms.");
    }
  }

  const issueDraftPaths = listMarkdownFiles(path.join(state.paths.issuesDir, "drafts"));
  const issueDraftTexts = issueDraftPaths.map((filePath) => readOptionalText(filePath) ?? "");
  if (!issueDraftPaths.length) {
    reasons.push("Issue drafts are missing.");
  }
  issueDraftTexts.forEach((text, index) => {
    const label = path.basename(issueDraftPaths[index] ?? `issue-${index + 1}.md`);
    if (!hasAcceptanceCriteria(text)) reasons.push(`Issue draft ${label} is missing acceptance criteria.`);
    if (!hasHitlAfkLabel(text)) reasons.push(`Issue draft ${label} is missing HITL/AFK label.`);
    if (!hasNamedSection(text, "Verification")) reasons.push(`Issue draft ${label} is missing verification commands.`);
    if (!hasNamedSection(text, "Affected modules/interfaces")) reasons.push(`Issue draft ${label} is missing affected modules/interfaces.`);
    if (!hasNamedSection(text, "Vertical-slice rationale")) reasons.push(`Issue draft ${label} is missing vertical-slice rationale.`);
    if (isHorizontalLayerBatch(text)) reasons.push(`Issue draft ${label} appears to be a horizontal layer batch without justification.`);
  });

  const queue = readJsonOptional(state.queue.queueFile);
  const queueItems = Array.isArray(queue?.items) ? queue.items : [];
  if (!queueItems.length) {
    reasons.push("Triage queue is missing items.");
  } else {
    const ids = new Set(queueItems.map((item: any) => String(item.id ?? "")));
    queueItems.forEach((item: any) => {
      const id = String(item.id ?? "<missing>");
      if (!item.draft) reasons.push(`Queue item ${id} is missing draft path.`);
      if (!item.verificationProfile) reasons.push(`Queue item ${id} is missing verification profile.`);
      if (!/^(AFK|HITL)$/i.test(String(item.type ?? ""))) reasons.push(`Queue item ${id} is missing HITL/AFK type.`);
      if (!/^P[0-3]$/i.test(String(item.priority ?? ""))) reasons.push(`Queue item ${id} is missing priority P0-P3.`);
      const blockedBy = Array.isArray(item.blockedBy) ? item.blockedBy : [];
      if (!Array.isArray(item.blockedBy)) reasons.push(`Queue item ${id} has invalid blockers.`);
      blockedBy.forEach((dep: any) => {
        if (!ids.has(String(dep))) reasons.push(`Queue item ${id} blocks on unknown issue ${dep}.`);
      });
    });
  }

  const sources = readJsonOptional(path.join(state.paths.workflowDir, "sources.json"));
  for (const source of Array.isArray(sources?.sources) ? sources.sources : []) {
    if (sourceNeedsSummary(source) && !hasSourceSummary(state, source)) {
      reasons.push(`External source ${source.raw ?? source.url ?? source.title ?? "<unknown>"} is missing source-note summary.`);
    }
  }

  if (options.requireExecutionEvidence && isUiFacingWorkflow(state, issueDraftTexts) && !hasVisualEvidence(state, options.evidencePaths)) {
    reasons.push("UI-facing workflow is missing visual evidence such as screenshot, artifact, or browser observation.");
  }

  return reasons;
}

function extractModuleInterfaceNotes(state: WorkflowState): string {
  const prd = readOptionalText(path.join(state.paths.artifactsDir, "prd.draft.md")) ?? "";
  const implementation = sectionBody(prd, "Implementation Decisions");
  const relevant = implementation
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => /\b(modules?|interfaces?|services?|validators?|routes?|components?|deep module|touched)\b/i.test(line));
  return relevant.join("\n") || implementation.slice(0, 500).trim();
}

function queueItemsInDependencyOrder(items: any[]): any[] {
  const byId = new Map<string, any>();
  for (const item of items) byId.set(String(item.id), item);

  const result: any[] = [];
  const temporary = new Set<string>();
  const permanent = new Set<string>();

  function visit(id: string) {
    if (permanent.has(id)) return;
    if (temporary.has(id)) throw new Error(`Cycle detected in triage queue at issue ${id}`);
    const item = byId.get(id);
    if (!item) throw new Error(`Queue item not found: ${id}`);

    temporary.add(id);
    const deps = Array.isArray(item.blockedBy) ? item.blockedBy.map(String) : [];
    for (const dep of deps) {
      if (!byId.has(dep)) throw new Error(`Queue item ${id} blocks on unknown issue ${dep}`);
      visit(dep);
    }
    temporary.delete(id);
    permanent.add(id);
    result.push(item);
  }

  for (const item of items) visit(String(item.id));
  return result;
}

function classifyIssueSlice(markdown: string): { sliceKind: TrackerIssueOperation["sliceKind"]; sliceRationale: string } {
  const verticalMatch = markdown.match(/##\s+Vertical-slice rationale\s*\n([\s\S]*?)(?=^##\s+|(?![\s\S]))/im)
    ?? markdown.match(/##\s+Tracer-bullet rationale\s*\n([\s\S]*?)(?=^##\s+|(?![\s\S]))/im);
  if (verticalMatch?.[1]?.trim()) {
    return { sliceKind: "vertical", sliceRationale: verticalMatch[1].trim() };
  }

  const horizontalMatch = markdown.match(/##\s+(?:Horizontal-slice justification|Horizontal justification)\s*\n([\s\S]*?)(?=^##\s+|(?![\s\S]))/im);
  if (horizontalMatch?.[1]?.trim()) {
    return { sliceKind: "horizontal-justified", sliceRationale: horizontalMatch[1].trim() };
  }

  if (!isHorizontalLayerBatch(markdown)) {
    return {
      sliceKind: "vertical",
      sliceRationale: "Inferred vertical slice: this issue is not classified as a horizontal layer batch and is tied to queue, artifact, and verification feedback.",
    };
  }

  return { sliceKind: "unspecified", sliceRationale: "No vertical-slice rationale or horizontal-slice justification found." };
}

function trackerRefForIssue(issueId: string): string {
  return `local:autopilot-v2-${slugify(issueId) || shortHash(issueId, 8)}`;
}

export function createTrackerIssues(
  state: WorkflowState,
  options: TrackerIssueCreationOptions = {},
): TrackerIssueCreationResult {
  const tracker = options.tracker ?? "local";
  if (tracker !== "local") throw new Error(`Unsupported tracker for local execution: ${tracker}`);

  const approval = state.gates["before-issues"];
  if (approval.status !== "approved") throw new Error("Cannot create tracker issues without before-issues approval.");
  if (!approval.approvalFile || !fs.existsSync(approval.approvalFile)) {
    throw new Error("Cannot create tracker issues without before-issues approval file.");
  }

  const queue = readJsonOptional(state.queue.queueFile);
  const queueItems = Array.isArray(queue?.items) ? queue.items : [];
  if (!queueItems.length) throw new Error("Cannot create tracker issues without queue items.");

  const orderedItems = queueItemsInDependencyOrder(queueItems);
  const now = options.now ?? nowIso();
  const createdDir = path.join(state.paths.issuesDir, "created");
  const parentTrackerRef = "local:autopilot-v2-prd";
  const moduleInterfaceNotes = extractModuleInterfaceNotes(state);
  const operations: TrackerIssueOperation[] = [
    {
      kind: "parent",
      title: `PRD: ${state.source.title}`,
      tracker: "local",
      trackerRef: parentTrackerRef,
      draft: "artifacts/prd.draft.md",
    },
  ];

  const createdIssues = orderedItems.map((item: any) => {
    const issueId = String(item.id);
    const draft = String(item.draft ?? "");
    const draftText = draft ? readOptionalText(workflowPath(state, draft)) ?? "" : "";
    if (!draftText) throw new Error(`Issue ${issueId} is missing source draft ${draft || "<missing>"}.`);
    const { sliceKind, sliceRationale } = classifyIssueSlice(draftText);
    if (sliceKind === "unspecified") throw new Error(`Issue ${issueId} lacks vertical-slice rationale or horizontal-slice justification.`);

    const trackerRef = trackerRefForIssue(issueId);
    operations.push({
      kind: "child",
      issueId,
      title: String(item.title ?? issueId),
      tracker: "local",
      trackerRef,
      draft,
      blockedBy: Array.isArray(item.blockedBy) ? item.blockedBy.map(String) : [],
      sliceKind,
    });

    return {
      version: 2,
      workflowId: state.workflowId,
      issueId,
      title: String(item.title ?? issueId),
      type: /^(AFK|HITL)$/i.test(String(item.type ?? "")) ? String(item.type).toUpperCase() : "AFK",
      priority: /^P[0-3]$/i.test(String(item.priority ?? "")) ? String(item.priority).toUpperCase() : "P2",
      status: item.status ?? "queued",
      tracker: "local",
      trackerRef,
      parentTrackerRef,
      sourceDraft: draft,
      blockedBy: Array.isArray(item.blockedBy) ? item.blockedBy.map(String) : [],
      verificationProfile: item.verificationProfile ?? "normal",
      moduleInterfaceNotes,
      sliceKind,
      sliceRationale,
      createdAt: now,
      url: null,
    };
  });

  const result: TrackerIssueCreationResult = {
    dryRun: Boolean(options.dryRun),
    tracker: "local",
    parentTrackerRef,
    operations,
    createdDir,
    queueFile: state.queue.queueFile,
  };

  if (options.dryRun) return result;

  fs.mkdirSync(createdDir, { recursive: true });
  const parentIssue = {
    version: 2,
    workflowId: state.workflowId,
    status: "created",
    tracker: "local",
    trackerRef: parentTrackerRef,
    url: null,
    title: `PRD: ${state.source.title}`,
    sourceArtifact: "artifacts/prd.draft.md",
    createdAt: now,
  };
  fs.writeFileSync(path.join(state.paths.issuesDir, "parent-prd-issue.json"), `${JSON.stringify(parentIssue, null, 2)}\n`, "utf8");
  fs.writeFileSync(path.join(createdDir, "000-parent-prd.json"), `${JSON.stringify(parentIssue, null, 2)}\n`, "utf8");
  for (const issue of createdIssues) {
    fs.writeFileSync(path.join(createdDir, `${issue.issueId}.json`), `${JSON.stringify(issue, null, 2)}\n`, "utf8");
  }

  const issuesById = new Map(createdIssues.map((issue) => [issue.issueId, issue]));
  const nextQueue = {
    ...queue,
    status: "triaged",
    tracker: "local",
    updatedAt: now,
    items: orderedItems.map((item: any) => {
      const issue = issuesById.get(String(item.id));
      return {
        ...item,
        tracker: "local",
        trackerRef: issue?.trackerRef,
        parentTrackerRef,
        status: item.status ?? "queued",
        priority: /^P[0-3]$/i.test(String(item.priority ?? "")) ? String(item.priority).toUpperCase() : "P2",
        moduleInterfaceNotes,
        sliceKind: issue?.sliceKind,
        sliceRationale: issue?.sliceRationale,
      };
    }),
  };
  fs.writeFileSync(state.queue.queueFile, `${JSON.stringify(nextQueue, null, 2)}\n`, "utf8");

  if (options.archiveDrafts) {
    const draftFiles = listMarkdownFiles(path.join(state.paths.issuesDir, "drafts"))
      .map((filePath) => path.relative(state.paths.workflowDir, filePath));
    fs.writeFileSync(
      path.join(state.paths.issuesDir, "drafts", ".archived.json"),
      `${JSON.stringify({ version: 2, workflowId: state.workflowId, archivedAt: now, status: "archived", draftFiles }, null, 2)}\n`,
      "utf8",
    );
  }

  appendWorkflowEvent(state, "tracker.local_created", {
    parentTrackerRef,
    childIssueCount: createdIssues.length,
    createdDir: path.relative(state.paths.workflowDir, createdDir),
    dryRun: false,
  });

  return result;
}

function limitText(text: string, max = 2000): string {
  const normalized = text.trim();
  if (normalized.length <= max) return normalized;
  return `${normalized.slice(0, max)}\n...[truncated]`;
}

function executionDir(state: WorkflowState): string {
  return path.join(state.paths.issuesDir, "execution");
}

function executionStatePath(state: WorkflowState, issueId: string): string {
  return path.join(executionDir(state), `${issueId}.json`);
}

function loadExecutionState(state: WorkflowState, issueId: string): any | null {
  return readJsonOptional(executionStatePath(state, issueId));
}

function saveExecutionState(state: WorkflowState, executionState: any): void {
  fs.mkdirSync(executionDir(state), { recursive: true });
  fs.writeFileSync(executionStatePath(state, executionState.issueId), `${JSON.stringify(executionState, null, 2)}\n`, "utf8");
}

function readQueue(state: WorkflowState): any {
  const queue = readJsonOptional(state.queue.queueFile);
  if (!queue || !Array.isArray(queue.items)) throw new Error("Execution queue is missing or invalid.");
  return queue;
}

function queueReadyCount(items: any[]): number {
  return items.filter((item) => {
    if (String(item.type ?? "AFK").toUpperCase() !== "AFK") return false;
    if ((item.status ?? "queued") !== "queued") return false;
    const deps: string[] = Array.isArray(item.blockedBy) ? item.blockedBy.map(String) : [];
    return deps.every((dep) => items.find((candidate) => String(candidate.id) === dep)?.status === "done");
  }).length;
}

function requireBeforeExecutionApproval(state: WorkflowState): void {
  const approval = state.gates["before-execution"];
  if (approval.status !== "approved") throw new Error("Cannot ship without before-execution approval.");
  if (!approval.approvalFile || !fs.existsSync(approval.approvalFile)) {
    throw new Error("Cannot ship without before-execution approval file.");
  }
}

function contextPacketDir(state: WorkflowState): string {
  return path.join(state.paths.artifactsDir, "context-packets");
}

function readRelevantSourceNotes(state: WorkflowState): Array<{ path: string; excerpt: string }> {
  const sources = readJsonOptional(path.join(state.paths.workflowDir, "sources.json"));
  const notePaths = new Set<string>();
  for (const source of Array.isArray(sources?.sources) ? sources.sources : []) {
    const notesPath = source?.notesPath ?? source?.notes_path;
    if (typeof notesPath === "string" && notesPath.trim()) notePaths.add(notesPath);
  }

  const sourceNotesDir = path.join(state.paths.artifactsDir, "source-notes");
  for (const filePath of listMarkdownFiles(sourceNotesDir)) {
    notePaths.add(path.relative(state.paths.workflowDir, filePath));
  }

  return [...notePaths].map((notePath) => {
    const text = readOptionalText(workflowPath(state, notePath)) ?? "";
    return { path: notePath, excerpt: limitText(text, 1200) };
  }).filter((entry) => entry.excerpt.length > 0);
}

function recentWorkflowEvidence(state: WorkflowState, limit = 8): Array<Record<string, unknown>> {
  try {
    return fs.readFileSync(state.paths.eventsFile, "utf8")
      .trim()
      .split(/\r?\n/)
      .filter(Boolean)
      .map((line) => JSON.parse(line) as Record<string, unknown>)
      .filter((event) => /approval|tracker|issue|verification|review|failure/i.test(String(event.type ?? "")))
      .slice(-limit);
  } catch {
    return [];
  }
}

export function buildExecutionContextPacket(
  state: WorkflowState,
  issue: any,
  now = nowIso(),
): ExecutionContextPacket {
  const blockerIds = Array.isArray(issue.blockedBy) ? issue.blockedBy.map(String) : [];
  const queue = readQueue(state);
  const blockers = queue.items
    .filter((item: any) => blockerIds.includes(String(item.id)))
    .map((item: any) => ({
      id: item.id,
      title: item.title,
      status: item.status,
      trackerRef: item.trackerRef,
      verificationProfile: item.verificationProfile,
    }));

  const prd = readOptionalText(path.join(state.paths.artifactsDir, "prd.draft.md")) ?? "";
  const glossary = readOptionalText(path.join(state.paths.artifactsDir, "ubiquitous-language.draft.md")) ?? "";
  const checks = readOptionalText(path.join(state.paths.root, "checks.yml")) ?? "";
  const sourceDraft = typeof issue.draft === "string" ? readOptionalText(workflowPath(state, issue.draft)) : null;

  return {
    version: 2,
    workflowId: state.workflowId,
    issueId: String(issue.id),
    generatedAt: now,
    issue: {
      id: issue.id,
      title: issue.title,
      type: issue.type,
      priority: issue.priority,
      trackerRef: issue.trackerRef,
      verificationProfile: issue.verificationProfile,
      draft: issue.draft,
      draftExcerpt: sourceDraft ? limitText(sourceDraft, 2200) : undefined,
      sliceKind: issue.sliceKind,
      sliceRationale: issue.sliceRationale,
    },
    blockers,
    prdExcerpt: limitText([
      sectionBody(prd, "Problem Statement"),
      sectionBody(prd, "Solution"),
      sectionBody(prd, "Implementation Decisions"),
      sectionBody(prd, "Testing Decisions"),
      sectionBody(prd, "Out of Scope"),
    ].filter(Boolean).join("\n\n"), 3200),
    glossaryExcerpt: limitText(glossary, 2000),
    sourceNotes: readRelevantSourceNotes(state),
    moduleInterfaceMap: issue.moduleInterfaceNotes || extractModuleInterfaceNotes(state),
    checks: limitText(checks, 1600),
    recentEvidence: recentWorkflowEvidence(state),
    constraints: [
      "Use TDD red-green-refactor; observe a red test before implementation for testable code changes.",
      "Do not write tests only after the implementation is complete.",
      "Run verification commands and capture evidence.",
      "Run fresh-context automated review before PR handoff.",
      "Open a PR or local PR record only from the isolated branch; never merge automatically.",
      "Route ambiguity to grill-me and design friction to the architecture lane.",
    ],
  };
}

export function buildExecutionPrompt(packet: ExecutionContextPacket): string {
  return [
    `Autopilot execution slice ${packet.issueId}: ${String(packet.issue.title ?? "")}`,
    "",
    "Context packet is bounded. Do not fetch unrelated long planning history unless a blocker requires it.",
    "",
    "Required loop:",
    "1. Restate the active issue and acceptance criteria from the packet.",
    "2. Write or identify the smallest failing test first and record the red result.",
    "3. Implement only enough code to pass that test.",
    "4. Refactor after green while preserving behavior.",
    "5. Run configured verification and record evidence.",
    "6. Write a worker report under the workflow execution evidence directory with red/green evidence, commands, changed files, visual evidence when relevant, and open risks.",
    "7. Prepare a fresh-context review packet. Do not self-review in the depleted implementation context.",
    "",
    "Hard rules:",
    "- Reject test-after-the-fact shortcuts.",
    "- One issue only. Do not start other queued issues.",
    "- Commit/push/open PR only on the isolated branch recorded in execution state.",
    "- If you cannot record red evidence, stop and report why instead of pretending TDD happened.",
    "- Do not merge.",
    "- Ambiguity routes to grill-me; design friction routes to architecture lane.",
    "",
    `Context packet JSON:\n${JSON.stringify(packet, null, 2)}`,
  ].join("\n");
}

export function buildFreshContextReviewPrompt(packet: ExecutionContextPacket): string {
  return [
    `Fresh-context review for Autopilot slice ${packet.issueId}: ${String(packet.issue.title ?? "")}`,
    "",
    "Review as an independent reviewer. Assume the implementation context is discarded.",
    "",
    "Push these standards into review:",
    "- Verify acceptance criteria against changed behavior and tests.",
    "- Confirm red-green-refactor evidence exists for testable changes.",
    "- Confirm no unrelated issue work was started.",
    "- Confirm verification evidence is present and failures are actionable.",
    "- Confirm PR handoff stops before merge.",
    "",
    `Context packet JSON:\n${JSON.stringify(packet, null, 2)}`,
  ].join("\n");
}

export function claimNextExecutionIssue(
  state: WorkflowState,
  options: ExecutionClaimOptions = {},
): ExecutionClaimResult {
  requireBeforeExecutionApproval(state);
  const queue = readQueue(state);
  const running = queue.items.find((item: any) => ["claimed", "running"].includes(String(item.status ?? "")));
  if (running) throw new Error(`Execution already has an active issue: ${running.id}.`);

  const orderedItems = queueItemsInDependencyOrder(queue.items);
  const issue = orderedItems.find((item: any) => {
    if (options.issueId && String(item.id) !== options.issueId) return false;
    if ((item.status ?? "queued") !== "queued") return false;
    const deps: string[] = Array.isArray(item.blockedBy) ? item.blockedBy.map(String) : [];
    return deps.every((dep) => queue.items.find((candidate: any) => String(candidate.id) === dep)?.status === "done");
  });
  if (!issue) throw new Error(options.issueId ? `No ready queued issue found: ${options.issueId}.` : "No ready queued issue found.");

  const now = options.now ?? nowIso();
  const issueId = String(issue.id);
  const branch = `autopilot/${slugify(state.workflowId).slice(0, 24)}/${slugify(issueId)}`;
  const worktreePath = path.join(state.paths.root, "worktrees", state.workflowId, issueId);
  fs.mkdirSync(worktreePath, { recursive: true });
  fs.mkdirSync(contextPacketDir(state), { recursive: true });

  const claimedIssue = {
    ...issue,
    status: "claimed",
    claimedAt: now,
    branch,
    worktreePath,
    executionMode: options.mode ?? "local",
  };
  queue.items = queue.items.map((item: any) => String(item.id) === issueId ? claimedIssue : item);
  queue.status = "approved";
  queue.readyCount = queueReadyCount(queue.items);
  queue.updatedAt = now;
  fs.writeFileSync(state.queue.queueFile, `${JSON.stringify(queue, null, 2)}\n`, "utf8");

  const executionState: IssueExecutionState = {
    version: 2,
    issueId,
    workflowId: state.workflowId,
    tracker: claimedIssue.tracker ?? "local",
    trackerRef: claimedIssue.trackerRef,
    status: "claimed",
    phase: "worktree",
    repoCwd: state.repoCwd,
    createdAt: now,
    updatedAt: now,
    worktreePath,
    branch,
  };
  const mutableExecutionState: any = {
    ...executionState,
    executionMode: options.mode ?? "local",
    failureStreak: 0,
    mergeAuthority: false,
  };
  saveExecutionState(state, mutableExecutionState);

  const contextPacket = buildExecutionContextPacket(state, claimedIssue, now);
  const contextPacketPath = path.join(contextPacketDir(state), `${issueId}.json`);
  const implementationPromptPath = path.join(contextPacketDir(state), `${issueId}.implementation.md`);
  const reviewPromptPath = path.join(contextPacketDir(state), `${issueId}.review.md`);
  fs.writeFileSync(contextPacketPath, `${JSON.stringify(contextPacket, null, 2)}\n`, "utf8");
  fs.writeFileSync(implementationPromptPath, `${buildExecutionPrompt(contextPacket)}\n`, "utf8");
  fs.writeFileSync(reviewPromptPath, `${buildFreshContextReviewPrompt(contextPacket)}\n`, "utf8");

  const nextState: WorkflowState = {
    ...state,
    queue: {
      ...state.queue,
      status: "approved",
      readyCount: queue.readyCount,
    },
    updatedAt: now,
  };
  saveWorkflowState(nextState);
  appendWorkflowEvent(nextState, "issue.claimed", {
    issueId,
    trackerRef: claimedIssue.trackerRef,
    branch,
    worktreePath,
    contextPacketPath: path.relative(state.paths.workflowDir, contextPacketPath),
    actor: options.actor,
  });

  return {
    state: nextState,
    issue: claimedIssue,
    executionState,
    contextPacket,
    contextPacketPath,
    implementationPromptPath,
    reviewPromptPath,
  };
}

export function recordWorkerRun(
  state: WorkflowState,
  issueId: string,
  result: WorkerCommandResult,
): IssueExecutionState {
  const executionState = loadExecutionState(state, issueId);
  if (!executionState) throw new Error(`Execution state not found for issue ${issueId}.`);
  const now = nowIso();
  const evidence: ExecutionEvidence[] = [
    ...(Array.isArray(executionState.evidence) ? executionState.evidence : []),
    {
      kind: "log",
      path: result.logPath,
      command: result.command,
      summary: result.exitCode === 0 ? "Worker command completed." : `Worker command failed with exit code ${result.exitCode}.`,
      exitCode: result.exitCode,
      createdAt: now,
    },
  ];
  const next: any = {
    ...executionState,
    status: result.exitCode === 0 ? "running" : "blocked",
    phase: result.exitCode === 0 ? "tdd" : "blocked",
    updatedAt: now,
    evidence,
    worker: {
      command: result.command,
      cwd: result.cwd,
      logPath: result.logPath,
      exitCode: result.exitCode,
      failureReason: result.failureReason,
      startedAt: result.startedAt,
      finishedAt: result.finishedAt,
    },
  };
  saveExecutionState(state, next);
  appendWorkflowEvent(state, "issue.worker_run_recorded", {
    issueId,
    command: result.command,
    exitCode: result.exitCode,
    logPath: result.logPath,
    failureReason: result.failureReason,
  });
  return next as IssueExecutionState;
}

function executionEvidenceEntries(options: ExecutionVerificationOptions, now: string): ExecutionEvidence[] {
  const entries: ExecutionEvidence[] = [];
  const redEvidence = options.redEvidence ?? (options.redTestObserved ? ["Red test observed before implementation."] : []);
  const greenEvidence = options.greenEvidence ?? (options.checksPassed ? options.evidence : []);
  for (const summary of redEvidence) entries.push({ kind: "red", summary, createdAt: now });
  for (const summary of greenEvidence) entries.push({ kind: "green", summary, createdAt: now });
  for (const summary of options.refactorNotes ?? []) entries.push({ kind: "refactor", summary, createdAt: now });
  for (const summary of options.visualEvidence ?? []) entries.push({ kind: "visual", summary, createdAt: now });
  for (const pathValue of options.evidencePaths ?? []) entries.push({ kind: "log", path: pathValue, summary: pathValue, createdAt: now });
  for (const command of options.commands ?? []) entries.push({ kind: "log", command, summary: command, createdAt: now });
  if (!entries.length) {
    for (const summary of options.evidence) entries.push({ kind: options.checksPassed ? "green" : "red", summary, createdAt: now });
  }
  return entries;
}

function executionHasEvidence(executionState: any, kind: EvidenceKind): boolean {
  return Array.isArray(executionState.evidence) && executionState.evidence.some((item: any) => item?.kind === kind);
}

function isUiFacingExecutionIssue(state: WorkflowState, issueId: string): boolean {
  const queue = readJsonOptional(state.queue.queueFile);
  const item = Array.isArray(queue?.items) ? queue.items.find((candidate: any) => String(candidate.id) === issueId) : null;
  const draftText = item?.draft ? readOptionalText(workflowPath(state, String(item.draft))) ?? "" : "";
  return /\b(UI|frontend|front-end|dashboard|screen|page|component|visual|browser|screenshot)\b/i.test(`${item?.title ?? ""}\n${draftText}`);
}

export function validateExecutionHandoff(state: WorkflowState, issueId: string, options: { requireVisualEvidence?: boolean } = {}): string[] {
  const executionState = loadExecutionState(state, issueId);
  if (!executionState) return [`Execution state not found for issue ${issueId}.`];
  const reasons: string[] = [];
  if (!executionState.verification?.redTestObserved && !executionHasEvidence(executionState, "red")) {
    reasons.push("Missing red test evidence.");
  }
  if (!executionState.verification?.checksPassed && !executionHasEvidence(executionState, "green")) {
    reasons.push("Missing green verification evidence.");
  }
  const needsVisual = options.requireVisualEvidence ?? isUiFacingExecutionIssue(state, issueId);
  if (needsVisual && !executionHasEvidence(executionState, "visual")) {
    reasons.push("Missing visual/browser evidence for UI-facing work.");
  }
  return reasons;
}

export function buildReviewerPacket(state: WorkflowState, issueId: string): Record<string, unknown> {
  const executionState = loadExecutionState(state, issueId);
  if (!executionState) throw new Error(`Execution state not found for issue ${issueId}.`);
  const queue = readJsonOptional(state.queue.queueFile);
  const issue = Array.isArray(queue?.items) ? queue.items.find((item: any) => String(item.id) === issueId) : null;
  const contextPath = path.join(contextPacketDir(state), `${issueId}.json`);
  const contextPacket = readJsonOptional(contextPath);
  return {
    version: 2,
    workflowId: state.workflowId,
    issueId,
    generatedAt: nowIso(),
    issue: issue ?? contextPacket?.issue ?? { id: issueId },
    executionState: {
      branch: executionState.branch,
      worktreePath: executionState.worktreePath,
      worker: executionState.worker,
      verification: executionState.verification,
      evidence: executionState.evidence ?? [],
    },
    acceptanceCriteria: issue?.draft ? sectionBody(readOptionalText(workflowPath(state, String(issue.draft))) ?? "", "Acceptance criteria") : "",
    reviewMode: "report-only",
    mayEditFiles: false,
  };
}

export function recordRepairAttempt(
  state: WorkflowState,
  issueId: string,
  options: { status: "passed" | "blocked"; findings: string[]; maxAttempts?: number; reportPath?: string; now?: string },
): IssueExecutionState {
  const executionState = loadExecutionState(state, issueId);
  if (!executionState) throw new Error(`Execution state not found for issue ${issueId}.`);
  const now = options.now ?? nowIso();
  const maxAttempts = Math.max(1, Math.floor(options.maxAttempts ?? defaultNativeRunnerConfig().maxRepairAttempts));
  const attempts = Number(executionState.repair?.attempts ?? 0) + 1;
  const exhausted = options.status === "blocked" && attempts >= maxAttempts;
  const evidence: ExecutionEvidence[] = [
    ...(Array.isArray(executionState.evidence) ? executionState.evidence : []),
    {
      kind: options.status === "passed" ? "review" : "risk",
      path: options.reportPath,
      summary: options.findings.join("\n") || `Repair ${options.status}.`,
      createdAt: now,
    },
  ];
  const next: any = {
    ...executionState,
    status: options.status === "passed" ? "running" : exhausted ? "blocked" : "running",
    phase: options.status === "passed" ? "verification" : exhausted ? "blocked" : "repair",
    updatedAt: now,
    evidence,
    repair: {
      attempts,
      maxAttempts,
      findings: options.findings,
      status: options.status,
      recordedAt: now,
    },
  };
  saveExecutionState(state, next);
  appendWorkflowEvent(state, "issue.repair_recorded", { issueId, attempts, maxAttempts, status: options.status, exhausted });
  return next as IssueExecutionState;
}

export function settleExecutionIssue(
  state: WorkflowState,
  issueId: string,
  options: { status: "done" | "blocked" | "queued" | "pr-open"; evidencePaths?: string[]; branch?: string; attempts?: number; now?: string },
): any {
  const queue = readQueue(state);
  const now = options.now ?? nowIso();
  let settled: any | null = null;
  queue.items = queue.items.map((item: any) => {
    if (String(item.id) !== issueId) return item;
    settled = {
      ...item,
      status: options.status,
      evidencePaths: [...(Array.isArray(item.evidencePaths) ? item.evidencePaths : []), ...(options.evidencePaths ?? [])],
      branch: options.branch ?? item.branch,
      attempts: options.attempts ?? item.attempts,
      settledAt: now,
    };
    return settled;
  });
  if (!settled) throw new Error(`Queue item not found: ${issueId}`);
  queue.readyCount = queueReadyCount(queue.items);
  queue.updatedAt = now;
  fs.writeFileSync(state.queue.queueFile, `${JSON.stringify(queue, null, 2)}\n`, "utf8");
  appendWorkflowEvent(state, "issue.settled", { issueId, status: options.status, evidencePaths: options.evidencePaths });
  return queue;
}

export async function runSchedulerRound(
  state: WorkflowState,
  options: {
    concurrency?: number;
    executeIssue: (issue: any) => Promise<{ status: "done" | "blocked"; evidencePaths?: string[]; branch?: string; attempts?: number }>;
  },
): Promise<{ selected: any[]; settled: Array<{ issueId: string; status: "done" | "blocked" }> }> {
  const queue = readQueue(state);
  const selected = selectReadyAfkIssues(queue, options.concurrency ?? defaultNativeRunnerConfig().concurrency);
  const settled: Array<{ issueId: string; status: "done" | "blocked" }> = [];
  await Promise.all(selected.map(async (issue: any) => {
    try {
      const result = await options.executeIssue(issue);
      settleExecutionIssue(state, String(issue.id), result);
      settled.push({ issueId: String(issue.id), status: result.status });
    } catch (error) {
      settleExecutionIssue(state, String(issue.id), {
        status: "blocked",
        evidencePaths: [],
        attempts: Number(issue.attempts ?? 0) + 1,
      });
      appendWorkflowEvent(state, "issue.scheduler_failure", { issueId: String(issue.id), message: error instanceof Error ? error.message : String(error) });
      settled.push({ issueId: String(issue.id), status: "blocked" });
    }
  }));
  return { selected, settled };
}

export function recordIntegrationResult(
  state: WorkflowState,
  options: {
    integrationBranch: string;
    completedIssues: string[];
    blockedIssues?: string[];
    status: "passed" | "blocked";
    commands: string[];
    conflicts?: string[];
    verificationOutput?: string;
    evidencePaths?: string[];
    reviewerReports?: string[];
    diffPaths?: string[];
    now?: string;
  },
): { resultPath: string; handoffPath: string; handoff: QAHandoff } {
  const now = options.now ?? nowIso();
  const integrationDir = path.join(state.paths.artifactsDir, "integration");
  fs.mkdirSync(integrationDir, { recursive: true });
  const result = {
    version: 2,
    workflowId: state.workflowId,
    integrationBranch: options.integrationBranch,
    targetBranchMergeAuthority: false,
    status: options.status,
    completedIssues: options.completedIssues,
    blockedIssues: options.blockedIssues ?? [],
    commands: options.commands,
    conflicts: options.conflicts ?? [],
    verificationOutput: options.verificationOutput,
    evidencePaths: options.evidencePaths ?? [],
    recordedAt: now,
  };
  const resultPath = path.join(integrationDir, "result.json");
  fs.writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`, "utf8");

  const queue = readJsonOptional(state.queue.queueFile);
  const issueSummaries = Array.isArray(queue?.items)
    ? queue.items.map((item: any) => ({ issueId: String(item.id), title: String(item.title ?? item.id), status: String(item.status ?? "queued") }))
    : [];
  const evidence: ExecutionEvidence[] = [
    ...options.commands.map((command) => ({ kind: "log" as const, command, summary: command, createdAt: now })),
    ...(options.conflicts ?? []).map((summary) => ({ kind: "risk" as const, summary, createdAt: now })),
    ...(options.evidencePaths ?? []).map((pathValue) => ({ kind: "log" as const, path: pathValue, summary: pathValue, createdAt: now })),
  ];
  const handoff = buildQaHandoff({
    workflowId: state.workflowId,
    generatedAt: now,
    issueSummaries,
    evidence,
    reviewerReports: options.reviewerReports,
    diffPaths: options.diffPaths,
  });
  const handoffPath = path.join(state.paths.artifactsDir, "qa-handoff", "handoff.json");
  fs.mkdirSync(path.dirname(handoffPath), { recursive: true });
  fs.writeFileSync(handoffPath, `${JSON.stringify(handoff, null, 2)}\n`, "utf8");
  appendWorkflowEvent(state, "integration.recorded", {
    integrationBranch: options.integrationBranch,
    status: options.status,
    resultPath: path.relative(state.paths.workflowDir, resultPath),
    handoffPath: path.relative(state.paths.workflowDir, handoffPath),
  });
  return { resultPath, handoffPath, handoff };
}

export function recordExecutionVerification(
  state: WorkflowState,
  issueId: string,
  options: ExecutionVerificationOptions,
): IssueExecutionState {
  const executionState = loadExecutionState(state, issueId);
  if (!executionState) throw new Error(`Execution state not found for issue ${issueId}.`);
  const now = options.now ?? nowIso();
  const evidence: ExecutionEvidence[] = [
    ...(Array.isArray(executionState.evidence) ? executionState.evidence : []),
    ...executionEvidenceEntries(options, now),
  ];
  if (options.redTestObserved && !evidence.some((item) => item.kind === "red")) {
    evidence.push({ kind: "red", summary: "Red test observed before implementation.", createdAt: now });
  }
  if (options.checksPassed && !evidence.some((item) => item.kind === "green")) {
    evidence.push({ kind: "green", summary: "Green verification passed after implementation.", createdAt: now });
  }
  const next: any = {
    ...executionState,
    status: "running",
    phase: "verification",
    updatedAt: now,
    evidence,
    verification: {
      redTestObserved: options.redTestObserved,
      checksPassed: options.checksPassed,
      evidence: options.evidence,
      redEvidence: options.redEvidence,
      greenEvidence: options.greenEvidence,
      refactorNotes: options.refactorNotes,
      commands: options.commands,
      evidencePaths: options.evidencePaths,
      visualEvidence: options.visualEvidence,
      recordedAt: now,
    },
  };
  saveExecutionState(state, next);
  appendWorkflowEvent(state, "issue.verification_recorded", { issueId, verification: next.verification });
  return next as IssueExecutionState;
}

export function recordFreshContextReview(
  state: WorkflowState,
  issueId: string,
  options: FreshReviewRecordOptions,
): IssueExecutionState {
  const executionState = loadExecutionState(state, issueId);
  if (!executionState) throw new Error(`Execution state not found for issue ${issueId}.`);
  const now = options.now ?? nowIso();
  const evidence: ExecutionEvidence[] = [
    ...(Array.isArray(executionState.evidence) ? executionState.evidence : []),
    {
      kind: "review",
      path: options.reportPath,
      summary: options.findings.length ? options.findings.join("\n") : `Review ${options.status}.`,
      createdAt: now,
    },
  ];
  const next: any = {
    ...executionState,
    status: options.status === "passed" ? "running" : "blocked",
    phase: "review",
    updatedAt: now,
    evidence,
    review: {
      status: options.status,
      findings: options.findings,
      reportPath: options.reportPath,
      recordedAt: now,
    },
  };
  saveExecutionState(state, next);
  appendWorkflowEvent(state, "issue.review_recorded", { issueId, review: next.review });
  return next as IssueExecutionState;
}

export function openExecutionPullRequest(
  state: WorkflowState,
  issueId: string,
  options: PullRequestRecordOptions = {},
): IssueExecutionState {
  const executionState = loadExecutionState(state, issueId);
  if (!executionState) throw new Error(`Execution state not found for issue ${issueId}.`);
  const handoffReasons = validateExecutionHandoff(state, issueId);
  if (handoffReasons.length) throw new Error(`Cannot open PR: ${handoffReasons.join(" ")}`);
  if (executionState.review && executionState.review.status !== "passed") throw new Error("Cannot open PR with blocking fresh-context review findings.");
  const now = options.now ?? nowIso();
  const prUrl = `local-pr://${state.workflowId}/${issueId}`;
  const next: any = {
    ...executionState,
    status: "pr-open",
    phase: "awaiting-merge",
    updatedAt: now,
    prUrl,
    pr: {
      title: options.title ?? String(executionState.trackerRef ?? issueId),
      body: options.body ?? "Local PR record. Merge remains manual.",
      mergeAuthority: false,
      openedAt: now,
    },
  };
  saveExecutionState(state, next);
  const prDir = path.join(state.paths.issuesDir, "prs");
  fs.mkdirSync(prDir, { recursive: true });
  fs.writeFileSync(path.join(prDir, `${issueId}.json`), `${JSON.stringify(next.pr, null, 2)}\n`, "utf8");

  const queue = readQueue(state);
  queue.items = queue.items.map((item: any) => String(item.id) === issueId ? { ...item, status: "pr-open", prUrl } : item);
  queue.updatedAt = now;
  fs.writeFileSync(state.queue.queueFile, `${JSON.stringify(queue, null, 2)}\n`, "utf8");
  appendWorkflowEvent(state, "issue.pr_opened", { issueId, prUrl, mergeAuthority: false });
  return next as IssueExecutionState;
}

export function recordExecutionFailure(
  state: WorkflowState,
  issueId: string,
  options: ExecutionFailureOptions,
): IssueExecutionState {
  const executionState = loadExecutionState(state, issueId);
  if (!executionState) throw new Error(`Execution state not found for issue ${issueId}.`);
  const now = options.now ?? nowIso();
  const failureStreak = Number(executionState.failureStreak ?? 0) + 1;
  const route = options.ambiguous ? "grill" : options.designFriction ? "architecture" : failureStreak === 1 ? "debug" : "pause";
  const next: any = {
    ...executionState,
    updatedAt: now,
    failureStreak,
    lastFailure: {
      message: options.message,
      ambiguous: Boolean(options.ambiguous),
      designFriction: Boolean(options.designFriction),
      nonAmbiguous: options.nonAmbiguous ?? (!options.ambiguous && !options.designFriction),
      route,
      recordedAt: now,
    },
    status: route === "debug" ? "running" : "blocked",
    phase: route === "debug" ? "debug" : "blocked",
  };
  saveExecutionState(state, next);
  appendWorkflowEvent(state, "issue.failure_recorded", { issueId, failureStreak, route, message: options.message });
  return next as IssueExecutionState;
}

export function validateWorkflowTransition(
  state: WorkflowState,
  targetPhase: WorkflowPhase,
  options: WorkflowTransitionOptions = {},
): WorkflowTransitionValidation {
  const allowedTargets = getAllowedTransitions(state);
  const reasons: string[] = [];

  if (!options.force && !allowedTargets.includes(targetPhase)) {
    reasons.push(
      `Invalid transition: ${state.phase} -> ${targetPhase}. Allowed: ${allowedTargets.length ? allowedTargets.join(", ") : "none"}.`,
    );
  }

  if (targetPhase === "issue-approval" || targetPhase === "issues-created") {
    const missingAgreement = validateAgreementSnapshot(state);
    if (missingAgreement.length) {
      reasons.push(`Missing human alignment artifacts: ${missingAgreement.join(", ")}.`);
    }
    reasons.push(...validateWorkflowArtifacts(state));
  }

  if (targetPhase === "issues-created" && state.gates["before-issues"].status !== "approved") {
    reasons.push("Missing before-issues approval gate.");
  }

  if (targetPhase === "execution-approval" || targetPhase === "ready-to-execute") {
    const missingExecutionEvidence = validateAgreementSnapshot(state);
    if (missingExecutionEvidence.length) {
      reasons.push(`Missing execution evidence requirements: ${missingExecutionEvidence.join(", ")}.`);
    }
    reasons.push(...validateWorkflowArtifacts(state, {
      requireExecutionEvidence: true,
      evidencePaths: options.evidencePaths,
    }));
  }

  if (targetPhase === "ready-to-execute") {
    if (state.gates["before-issues"].status !== "approved") {
      reasons.push("Missing before-issues approval gate.");
    }
    if (state.gates["before-execution"].status !== "approved") {
      reasons.push("Missing before-execution approval gate.");
    }
    if (state.queue.itemCount < 1 || state.queue.status === "missing") {
      reasons.push("Missing approved or triaged execution queue.");
    }
  }

  return {
    ok: reasons.length === 0,
    from: state.phase,
    to: targetPhase,
    reasons,
    allowedTargets,
  };
}

export function transitionWorkflowPhase(
  state: WorkflowState,
  targetPhase: WorkflowPhase,
  options: WorkflowTransitionOptions = {},
): WorkflowTransitionResult {
  const validation = validateWorkflowTransition(state, targetPhase, options);
  if (!validation.ok) {
    appendWorkflowEvent(state, "workflow.transition_rejected", {
      from: state.phase,
      to: targetPhase,
      actor: options.actor,
      note: options.note,
      reasons: validation.reasons,
      forced: Boolean(options.force),
    });
    return { ok: false, state, validation };
  }

  const next: WorkflowState = {
    ...state,
    phase: targetPhase,
    status: options.status ?? state.status,
    updatedAt: nowIso(),
  };
  if (state.lane === "planning" && targetPhase === "prd-draft") {
    ensureWorkflowSkeleton(next);
  }
  saveWorkflowState(next);
  appendWorkflowEvent(next, "workflow.phase_changed", {
    from: state.phase,
    phase: targetPhase,
    status: next.status,
    actor: options.actor,
    note: options.note,
    evidencePaths: options.evidencePaths,
    forced: Boolean(options.force),
  });
  return { ok: true, state: next, validation };
}

export function loadWorkflowState(repoCwd: string, workflowId: string): WorkflowState | null {
  const stateFile = path.join(getWorkflowDir(repoCwd, workflowId), "state.json");
  if (!fs.existsSync(stateFile)) return null;
  try {
    return JSON.parse(fs.readFileSync(stateFile, "utf8")) as WorkflowState;
  } catch {
    return null;
  }
}

export function findWorkflowState(repoCwd: string, selector: string): WorkflowState | null {
  const workflows = listWorkflowStates(repoCwd);
  return workflows.find((wf) => wf.workflowId === selector)
    ?? workflows.find((wf) => wf.workflowId.startsWith(selector))
    ?? workflows.find((wf) => slugify(wf.source.title).includes(slugify(selector)))
    ?? null;
}

export function listWorkflowStates(repoCwd: string): WorkflowState[] {
  const workflowsDir = path.join(getAutopilotRoot(repoCwd), WORKFLOWS_DIR);
  if (!fs.existsSync(workflowsDir)) return [];
  return fs.readdirSync(workflowsDir)
    .map((entry) => path.join(workflowsDir, entry, "state.json"))
    .filter((stateFile) => fs.existsSync(stateFile))
    .map((stateFile) => {
      try {
        return JSON.parse(fs.readFileSync(stateFile, "utf8")) as WorkflowState;
      } catch {
        return null;
      }
    })
    .filter((state): state is WorkflowState => state !== null)
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
}

export function approveGate(state: WorkflowState, gate: ApprovalGate, approvedBy: string, note?: string): WorkflowState {
  const approvalFile = path.join(state.paths.approvalsDir, `${gate}.json`);
  const approvedAt = nowIso();
  const approval: GateState = {
    status: "approved",
    approvedAt,
    approvedBy,
    approvalFile,
    note,
  };
  const withApproval: WorkflowState = {
    ...state,
    gates: {
      ...state.gates,
      [gate]: approval,
    },
    updatedAt: approvedAt,
  };

  const targetPhase: WorkflowPhase = gate === "before-issues" ? "issues-created" : "ready-to-execute";
  const transition = transitionWorkflowPhase(withApproval, targetPhase, {
    actor: approvedBy,
    note,
    status: gate === "before-execution" ? "ready" : withApproval.status,
  });

  if (!transition.ok) {
    appendWorkflowEvent(state, "approval.rejected", {
      gate,
      approvedBy,
      note,
      targetPhase,
      reasons: transition.validation.reasons,
    });
    return state;
  }

  fs.mkdirSync(path.dirname(approvalFile), { recursive: true });
  fs.writeFileSync(approvalFile, `${JSON.stringify({ version: 2, workflowId: state.workflowId, gate, ...approval }, null, 2)}\n`, "utf8");
  appendWorkflowEvent(transition.state, "approval.granted", { gate, approvedBy, note, approvalFile });
  return transition.state;
}

export function updateWorkflowPhase(state: WorkflowState, phase: WorkflowPhase, status: WorkflowStatus = state.status): WorkflowState {
  const next: WorkflowState = { ...state, phase, status, updatedAt: nowIso() };
  saveWorkflowState(next);
  appendWorkflowEvent(next, "workflow.phase_changed", { phase, status });
  return next;
}

export function summarizeWorkflow(state: WorkflowState): string {
  return [
    `${state.workflowId}`,
    `  lane=${state.lane} status=${state.status} phase=${state.phase}`,
    `  source=${state.source.kind}:${state.source.title}`,
    `  gates=issues:${state.gates["before-issues"].status} execution:${state.gates["before-execution"].status}`,
    `  queue=${state.queue.status} items=${state.queue.itemCount} ready=${state.queue.readyCount}`,
  ].join("\n");
}

export function writeLatestStatus(repoCwd: string, status: unknown): string {
  const statusFile = path.join(getAutopilotRoot(repoCwd), "status", "latest.json");
  fs.mkdirSync(path.dirname(statusFile), { recursive: true });
  fs.writeFileSync(statusFile, `${JSON.stringify(status, null, 2)}\n`, "utf8");
  return statusFile;
}

export function buildPlanningPrompt(state: WorkflowState): string {
  const grillMeSkillPath = path.join(process.env.HOME ?? "", ".pi", "agent", "skills", "grill-me", "SKILL.md");
  const theFoolSkillPath = path.join(process.env.HOME ?? "", ".pi", "agent", "skills", "the-fool", "SKILL.md");
  const opensrcSkillPath = path.join(process.env.HOME ?? "", ".pi", "agent", "skills", "opensrc", "SKILL.md");
  const ubiquitousLanguagePath = path.join(process.env.HOME ?? "", ".pi", "agent", "skills", "ubiquitous-language", "SKILL.md");
  const toIssuesPath = path.join(process.env.HOME ?? "", ".pi", "agent", "skills", "to-issues", "SKILL.md");

  return [
    "Autopilot v2 planning lane is active.",
    `Repo root: ${state.repoCwd}`,
    `Workflow id: ${state.workflowId}`,
    `Workflow dir: ${state.paths.workflowDir}`,
    `Source kind: ${state.source.kind}`,
    `Source: ${state.source.raw}`,
    "",
    "Hard gates:",
    "- Planning is grill-first. Do NOT write PRD, glossary, issue drafts, or triage queue until concept lock.",
    "- During grill, only write confirmed decisions to decisions.md and source notes needed for better questions.",
    "- Do NOT create GitHub or Linear issues until the before-issues approval file exists.",
    "- Do NOT start execution or edit implementation files until before-execution approval exists.",
    "- UI work does not need pre-execution visual approval; collect post-execution browser/visual evidence for human QA.",
    "",
    "Locked artifact paths before concept lock:",
    ...lockedPlanningArtifactPaths(state).map((artifactPath) => `- ${artifactPath}`),
    "",
    "Always-allowed during grill:",
    `- Decisions log: ${path.join(state.paths.workflowDir, "decisions.md")}`,
    `- Source notes: ${path.join(state.paths.artifactsDir, "source-notes")}`,
    "",
    "Workflow:",
    `1. Read ${grillMeSkillPath} and use it to establish shared design concept.`,
    `2. Inspect repo/source/transcript/dependencies first; use ${opensrcSkillPath} when internals affect design.`,
    "3. Ask one useful grill question at a time. Do not ask questions already answered by inspected evidence.",
    "4. Grill must cover product intent, affected modules/interfaces, TDD boundary strategy, verification commands, visual/QA expectations, and open risks.",
    "5. Maintain decisions.md as decisions become confirmed.",
    "6. Stop grilling when shared design concept is locked. Avoid confirmation-theater questions.",
    "7. After concept lock, automatically continue in the same workflow: draft PRD, glossary, vertical-slice issue DAG, and triage queue. Do not require another slash command.",
    `8. Use ${theFoolSkillPath} for a pre-mortem before finalizing drafts.`,
    `9. Use ${ubiquitousLanguagePath} as glossary style reference and ${toIssuesPath} as vertical-slice issue style reference.`,
    "10. Run a vertical-slice self-check before requesting before-issues approval.",
    "",
    "Concept lock checklist:",
    "- Shared design concept is explicit",
    "- Acceptance criteria are explicit",
    "- Verification commands/tests are chosen",
    "- Affected modules/interfaces are named",
    "- TDD boundary strategy is named",
    "- Visual/browser QA expectations are named when relevant",
    "- HITL planning/decision slices and AFK execution slices are labeled",
    "",
    "After concept lock, produce/update:",
    `- PRD summary artifact: ${path.join(state.paths.artifactsDir, "prd.draft.md")}`,
    `- Glossary draft: ${path.join(state.paths.artifactsDir, "ubiquitous-language.draft.md")}`,
    `- Issue drafts: ${path.join(state.paths.issuesDir, "drafts")}/001-*.md`,
    `- Triage queue draft: ${state.queue.queueFile}`,
    "",
    "When drafts pass self-check, update state.json agreement fields if you edit it safely, append a short note to decisions.md, then request inline approval by calling:",
    `  autopilot_transition({ workflowId: \"${state.workflowId}\", gate: \"issues\" })`,
    "Do not tell Ossie to manually run an approval slash command unless the tool is unavailable.",
  ].join("\n");
}

export function buildArchitecturePrompt(state: WorkflowState): string {
  const improvePath = path.join(process.env.HOME ?? "", ".pi", "agent", "skills", "improve-codebase-architecture", "SKILL.md");
  const refactorPath = path.join(process.env.HOME ?? "", ".pi", "agent", "skills", "request-refactor-plan", "SKILL.md");

  return [
    "Autopilot v2 architecture lane is active.",
    `Repo root: ${state.repoCwd}`,
    `Workflow id: ${state.workflowId}`,
    `Workflow dir: ${state.paths.workflowDir}`,
    `Architecture scope: ${state.source.raw}`,
    "",
    "Hard gates:",
    "- Do NOT create tracker issues until before-issues approval exists.",
    "- Do NOT refactor implementation files until before-execution approval exists.",
    "- UI-facing refactors require visual confirmation evidence before execution approval.",
    "",
    "Workflow:",
    `1. Read ${improvePath} completely and use it to find deep-module opportunities.`,
    "2. Present candidates before picking an interface/refactor direction.",
    `3. Use ${refactorPath} as the refactor RFC style reference.`,
    "4. Write local refactor RFC and issue drafts under the workflow directory.",
    "",
    "Required files to produce/update:",
    `- Refactor PRD/RFC draft: ${path.join(state.paths.artifactsDir, "prd.draft.md")}`,
    `- Decisions log: ${path.join(state.paths.workflowDir, "decisions.md")}`,
    `- Issue drafts: ${path.join(state.paths.issuesDir, "drafts")}/001-*.md`,
    `- Triage queue draft: ${state.queue.queueFile}`,
    "",
    "When drafts are ready, request inline approval by calling:",
    `  autopilot_transition({ workflowId: \"${state.workflowId}\", gate: \"issues\" })`,
    "Do not tell Ossie to manually run an approval slash command unless the tool is unavailable.",
  ].join("\n");
}

export function buildApprovalPrompt(state: WorkflowState, gate: ApprovalGate): string {
  if (gate === "before-issues") {
    return [
      `Autopilot v2 approval granted: before-issues for ${state.workflowId}.`,
      `Workflow dir: ${state.paths.workflowDir}`,
      "You may now create/update the parent PRD issue and child GitHub/Linear issues from the approved local drafts.",
      "Rules:",
      "- Preserve the local drafts as source artifacts.",
      "- Create child issues in dependency order.",
      "- Record created tracker refs under issues/created/ and update triage/queue.json.",
      "- Do NOT start implementation. Stop after issue creation/triage and request execution approval with autopilot_transition({ gate: \"execution\" }).",
    ].join("\n");
  }

  return [
    `Autopilot v2 approval granted: before-execution for ${state.workflowId}.`,
    `Workflow dir: ${state.paths.workflowDir}`,
    "You may now execute the approved AFK queue.",
    "Rules:",
    "- One issue = one fresh worktree/branch and one fresh worker process.",
    "- Ready AFK issues may run in parallel up to concurrency 2 unless repo preferences override it.",
    "- Use strict TDD red-green-refactor for each issue and store red/green evidence.",
    "- Run a fresh report-only reviewer after implementation evidence exists.",
    "- Reviewer blockers route to fresh repair workers up to the configured attempt limit, default 2.",
    "- Completed branches merge into an integration branch for final verification and human QA handoff.",
    "- Do not merge into the target branch. Do not mutate external services unless the issue manifest explicitly allows it.",
    "- If ambiguity appears: route back to grill-me. If design friction appears: route to architecture lane.",
  ].join("\n");
}

function deriveTitle(rawInput: string, kind: IntakeKind): string {
  const trimmed = rawInput.trim();
  if (!trimmed) return kind === "repo" ? "Repo planning" : "Autopilot workflow";
  if (kind === "local_plan") return path.basename(trimmed);
  return trimmed.length > 120 ? `${trimmed.slice(0, 117)}...` : trimmed;
}

function writeIfMissing(filePath: string, content: string): void {
  if (fs.existsSync(filePath)) return;
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, "utf8");
}

function decisionsTemplate(state: WorkflowState): string {
  return [
    `# Autopilot Decisions: ${state.source.title}`,
    "",
    `- Workflow: ${state.workflowId}`,
    `- Lane: ${state.lane}`,
    `- Source kind: ${state.source.kind}`,
    `- Created: ${state.createdAt}`,
    "",
    "## Decisions",
    "",
    "- Pending shared design concept.",
    "",
    "## Open questions",
    "",
    "- Pending grill-me interview.",
  ].join("\n");
}

function prdDraftTemplate(state: WorkflowState): string {
  return [
    `# PRD Draft: ${state.source.title}`,
    "",
    `Workflow: ${state.workflowId}`,
    "",
    "## Problem Statement",
    "",
    "TBD.",
    "",
    "## Solution",
    "",
    "TBD.",
    "",
    "## User Stories",
    "",
    "1. TBD.",
    "",
    "## Implementation Decisions",
    "",
    "- TBD.",
    "",
    "## Testing Decisions",
    "",
    "- TBD.",
    "",
    "## Out of Scope",
    "",
    "- TBD.",
  ].join("\n");
}

function glossaryDraftTemplate(state: WorkflowState): string {
  return [
    `# Ubiquitous Language Draft: ${state.source.title}`,
    "",
    `Workflow: ${state.workflowId}`,
    "",
    "## Terms",
    "",
    "| Term | Definition | Aliases to avoid |",
    "| --- | --- | --- |",
    "| TBD | TBD | TBD |",
    "",
    "## Relationships",
    "",
    "- TBD.",
    "",
    "## Flagged ambiguities",
    "",
    "- TBD.",
  ].join("\n");
}

function defaultChecksConfig(): string {
  return [
    "version: 2",
    "profiles:",
    "  normal:",
    "    required:",
    "      - id: git-diff-check",
    "        type: command",
    "        run: git diff --check",
    "  strict:",
    "    required:",
    "      - id: git-diff-check",
    "        type: command",
    "        run: git diff --check",
    "  conservative:",
    "    required:",
    "      - id: git-diff-check",
    "        type: command",
    "        run: git diff --check",
    "overrides: []",
    "",
  ].join("\n");
}
