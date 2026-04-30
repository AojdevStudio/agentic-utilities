import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createJiti } from "@mariozechner/jiti";

const jiti = createJiti(import.meta.url);
const v2 = await jiti.import("./v2.ts");
const runnerModule = await jiti.import("./execution-runner.ts");

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function readEvents(file) {
  return fs
    .readFileSync(file, "utf8")
    .trim()
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

function writeCompletePlanningArtifacts(workflow, options = {}) {
  fs.mkdirSync(workflow.paths.artifactsDir, { recursive: true });
  fs.mkdirSync(path.join(workflow.paths.artifactsDir, "source-notes"), { recursive: true });
  fs.mkdirSync(path.join(workflow.paths.artifactsDir, "screenshots"), { recursive: true });
  fs.mkdirSync(path.join(workflow.paths.issuesDir, "drafts"), { recursive: true });

  fs.writeFileSync(
    path.join(workflow.paths.artifactsDir, "prd.draft.md"),
    `# PRD Draft: Smoke Workflow

Workflow: ${workflow.workflowId}

## Problem Statement

Users need a validated workflow with enough implementation detail to avoid ambiguous execution.

## Solution

Build a vertical transition validator that checks files, queue state, source summaries, and visual evidence before gated execution.

## User Stories

1. As Ossie, I can approve only complete artifacts.

## Implementation Decisions

- Affected modules/interfaces: agent/extensions/autopilot/v2.ts validateWorkflowTransition and validateWorkflowArtifacts.
- Deep module: v2 workflow validator owns file-system artifact checks.
${options.uiFacing ? "- UI-facing surface: dashboard workflow requires screenshot evidence.\n" : ""}
## Testing Decisions

- Verify complete drafts pass and incomplete drafts fail.

## Out of Scope

- External tracker mutation.
`,
    "utf8",
  );

  fs.writeFileSync(
    path.join(workflow.paths.artifactsDir, "ubiquitous-language.draft.md"),
    `# Ubiquitous Language Draft: Smoke Workflow

Workflow: ${workflow.workflowId}

## Terms

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| Workflow | Durable Autopilot planning state with artifacts, gates, and queue. | Session |
| Vertical slice | End-to-end issue that produces observable feedback. | Horizontal batch |

## Relationships

- A Workflow has a triage queue and issue drafts.

## Flagged ambiguities

- None.
`,
    "utf8",
  );

  const issueBody = options.horizontalIssue
    ? `# Create schema layer only

Type: AFK

## What to build

Add schema-only database migration tables for the future feature.

## Acceptance criteria

- [ ] Schema tables exist.
`
    : `# Validate one vertical workflow slice

Type: AFK

## What to build

Implement a tracer-bullet vertical slice that validates PRD, glossary, queue, and issue draft evidence end to end.

## Acceptance criteria

- [ ] PRD validation fails on template content.
- [ ] Glossary validation fails on template terms.
- [ ] Queue validation checks blockers and verification profile.

## Verification

\`\`\`bash
npm run check
node extensions/autopilot/v2-smoke-test.mjs
\`\`\`

## Affected modules/interfaces

- extensions/autopilot/v2.ts validateWorkflowArtifacts and transition validation.

## Vertical-slice rationale

This crosses artifact files, queue JSON, and transition validation so it produces observable feedback.
`;
  fs.writeFileSync(path.join(workflow.paths.issuesDir, "drafts", "001-smoke.md"), issueBody, "utf8");

  fs.writeFileSync(
    workflow.queue.queueFile,
    JSON.stringify(
      {
        version: 2,
        workflowId: workflow.workflowId,
        status: "drafted",
        items: [
          {
            id: "001",
            title: "Validate one vertical workflow slice",
            type: "AFK",
            draft: "issues/drafts/001-smoke.md",
            blockedBy: [],
            verificationProfile: "normal",
            priority: "P0",
          },
        ],
      },
      null,
      2,
    ) + "\n",
    "utf8",
  );

  if (options.sourceNotes) {
    fs.writeFileSync(
      path.join(workflow.paths.artifactsDir, "source-notes", "external.md"),
      "# Source Notes\n\nExternal source summarized for execution.\n",
      "utf8",
    );
    fs.writeFileSync(
      path.join(workflow.paths.workflowDir, "sources.json"),
      JSON.stringify(
        {
          sources: [{ ...workflow.source, notesPath: "artifacts/source-notes/external.md" }],
        },
        null,
        2,
      ) + "\n",
      "utf8",
    );
  }

  if (options.visualEvidence) {
    fs.writeFileSync(
      path.join(workflow.paths.artifactsDir, "screenshots", "ui-evidence.txt"),
      "browser evidence placeholder\n",
      "utf8",
    );
  }
}

function makeExecutionApprovedState(workflow) {
  const issuesApprovalFile = path.join(workflow.paths.approvalsDir, "before-issues.json");
  const executionApprovalFile = path.join(workflow.paths.approvalsDir, "before-execution.json");
  fs.mkdirSync(workflow.paths.approvalsDir, { recursive: true });
  fs.writeFileSync(
    issuesApprovalFile,
    JSON.stringify({ gate: "before-issues", status: "approved" }, null, 2) + "\n",
    "utf8",
  );
  fs.writeFileSync(
    executionApprovalFile,
    JSON.stringify({ gate: "before-execution", status: "approved" }, null, 2) + "\n",
    "utf8",
  );
  return {
    ...workflow,
    phase: "ready-to-execute",
    agreement: {
      prd: "drafted",
      glossary: "drafted",
      acceptance: "explicit",
      verification: "chosen",
      modulesInterfaces: "named",
      hitlAfk: "labeled",
    },
    gates: {
      "before-issues": { status: "approved", approvalFile: issuesApprovalFile },
      "before-execution": { status: "approved", approvalFile: executionApprovalFile },
    },
    queue: {
      ...workflow.queue,
      status: "triaged",
      itemCount: 1,
      readyCount: 1,
    },
  };
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-"));
try {
  const workflow = v2.createWorkflow({
    repoCwd: tmp,
    lane: "planning",
    rawInput: "smoke transition workflow",
  });

  const invalid = v2.transitionWorkflowPhase(workflow, "ready-to-execute", {
    actor: "smoke-test",
    note: "invalid transition smoke test",
  });
  assert.equal(invalid.ok, false);
  assert.match(invalid.validation.reasons.join("\n"), /Invalid transition/);
  assert.equal(readJson(workflow.paths.stateFile).phase, "grill");
  assert.equal(workflow.phase, "grill");
  assert.equal(fs.existsSync(path.join(workflow.paths.artifactsDir, "prd.draft.md")), false);
  assert.equal(
    v2.isPlanningArtifactLockedPath(workflow, path.join(workflow.paths.artifactsDir, "prd.draft.md")).locked,
    true,
  );
  assert.equal(
    v2.isPlanningArtifactLockedPath(workflow, path.join(workflow.paths.workflowDir, "decisions.md")).locked,
    false,
  );
  assert.match(v2.buildPlanningPrompt(workflow), /After concept lock, automatically continue/);
  const conceptLocked = v2.recordConceptLock(workflow, {
    summary: "Shared design concept accepted for smoke workflow.",
    acceptedBy: "smoke-test",
    now: "2026-01-01T00:00:00.000Z",
  });
  assert.equal(conceptLocked.phase, "prd-draft");
  assert.equal(fs.existsSync(path.join(conceptLocked.paths.artifactsDir, "concept-lock.json")), true);
  assert.equal(fs.existsSync(path.join(conceptLocked.paths.artifactsDir, "prd.draft.md")), true);
  assert.match(v2.buildArtifactDraftingPrompt(conceptLocked), /PRD as a summary of decisions\.md/);

  const workerCommand = v2.buildWorkerCommand("cd {{WORKTREE_PATH}} && pi -p @{{PROMPT_PATH}}", {
    repoCwd: "/repo root",
    worktreePath: "/repo root/worktrees/001",
    promptPath: "/repo root/prompts/001.md",
    workflowId: "wf-1",
    issueId: "001",
  });
  assert.match(workerCommand, /^cd '\/repo root\/worktrees\/001' && pi -p @'\/repo root\/prompts\/001.md'$/);
  assert.equal(v2.defaultNativeRunnerConfig().concurrency, 2);
  assert.equal(v2.defaultNativeRunnerConfig().maxRepairAttempts, 2);
  assert.equal(v2.defaultNativeRunnerConfig().envAllowlist.includes("PATH"), true);
  assert.deepEqual(
    v2
      .selectReadyAfkIssues(
        {
          items: [
            { id: "001", type: "AFK", priority: "P1", status: "queued", blockedBy: [] },
            { id: "002", type: "AFK", priority: "P0", status: "queued", blockedBy: [] },
            { id: "003", type: "HITL", priority: "P0", status: "queued", blockedBy: [] },
          ],
        },
        2,
      )
      .map((item) => item.id),
    ["002", "001"],
  );

  process.env.PI_SMOKE_ALLOWED = "yes";
  const runner = runnerModule.createNativeRunner({
    config: { commandTemplate: "echo {{ISSUE_ID}}", idleTimeoutSeconds: 5, envAllowlist: ["PI_*"] },
    executor: async (command, options) => ({
      command,
      cwd: options.cwd,
      envAllowed: options.env?.PI_SMOKE_ALLOWED,
      envDenied: options.env?.PATH,
      startedAt: "2026-01-01T00:00:00.000Z",
      finishedAt: "2026-01-01T00:00:00.001Z",
      exitCode: 0,
      stdout: "001\n",
      stderr: "",
      logPath: options.logPath,
    }),
  });
  const runnerResult = await runner.runWorker({
    repoCwd: "/repo",
    worktreePath: "/repo/worktrees/001",
    promptPath: "/repo/prompts/001.md",
    workflowId: "wf-1",
    issueId: "001",
  });
  assert.equal(runner.config.concurrency, 2);
  assert.equal(runnerResult.command, "echo '001'");
  assert.equal(runnerResult.envAllowed, "yes");
  assert.equal(runnerResult.envDenied, undefined);

  const handoff = v2.buildQaHandoff({
    workflowId: "wf-1",
    generatedAt: "2026-01-01T00:00:00.000Z",
    issueSummaries: [{ issueId: "001", title: "Issue", status: "done" }],
    evidence: [
      { kind: "red", summary: "red failed", createdAt: "2026-01-01T00:00:00.000Z" },
      { kind: "green", summary: "green passed", createdAt: "2026-01-01T00:00:00.000Z" },
      {
        kind: "visual",
        path: "artifacts/browser/001.png",
        summary: "browser evidence",
        createdAt: "2026-01-01T00:00:00.000Z",
      },
      { kind: "risk", summary: "inspect copy", createdAt: "2026-01-01T00:00:00.000Z" },
    ],
  });
  assert.deepEqual(handoff.highlights.tddEvidence, ["red failed", "green passed"]);
  assert.deepEqual(handoff.highlights.visualEvidence, ["artifacts/browser/001.png"]);
  assert.deepEqual(handoff.highlights.openRisks, ["inspect copy"]);

  const statusTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-status-"));
  try {
    const statusWorkflow = v2.createWorkflow({
      repoCwd: statusTmp,
      lane: "planning",
      rawInput: "smoke status workflow",
    });
    const statusState = makeExecutionApprovedState(statusWorkflow);
    writeCompletePlanningArtifacts(statusState);
    fs.writeFileSync(
      statusState.queue.queueFile,
      JSON.stringify(
        {
          version: 2,
          workflowId: statusState.workflowId,
          status: "triaged",
          items: [
            {
              id: "001",
              title: "Status slice",
              type: "AFK",
              priority: "P0",
              status: "done",
              draft: "issues/drafts/001-smoke.md",
              blockedBy: [],
              verificationProfile: "strict",
              evidencePaths: ["execution/evidence/001/report.md"],
              prUrl: "local-pr://status/001",
              updatedAt: "2026-01-01T00:00:00.000Z",
            },
          ],
        },
        null,
        2,
      ) + "\n",
      "utf8",
    );
    v2.saveWorkflowState(statusState);
    const missingReport = v2.buildWorkflowStatusReport(statusState, "2026-01-01T00:00:00.000Z");
    assert.equal(missingReport.slices[0].computedState, "not-done");
    assert.match(missingReport.slices[0].missingEvidence.join("\n"), /execution\/evidence\/001\/report\.md/);
    assert.match(missingReport.slices[0].missingEvidence.join("\n"), /issues\/execution\/001\.json/);
    assert.match(missingReport.slices[0].missingEvidence.join("\n"), /issues\/prs\/001\.json/);

    fs.mkdirSync(path.join(statusState.paths.workflowDir, "execution", "evidence", "001"), { recursive: true });
    fs.writeFileSync(
      path.join(statusState.paths.workflowDir, "execution", "evidence", "001", "report.md"),
      "status evidence\n",
      "utf8",
    );
    fs.mkdirSync(path.join(statusState.paths.issuesDir, "execution"), { recursive: true });
    fs.writeFileSync(
      path.join(statusState.paths.issuesDir, "execution", "001.json"),
      JSON.stringify({ issueId: "001", updatedAt: "2026-01-01T00:00:01.000Z" }, null, 2) + "\n",
      "utf8",
    );
    fs.mkdirSync(path.join(statusState.paths.issuesDir, "prs"), { recursive: true });
    fs.writeFileSync(
      path.join(statusState.paths.issuesDir, "prs", "001.json"),
      JSON.stringify({ prUrl: "local-pr://status/001" }, null, 2) + "\n",
      "utf8",
    );

    const completeReport = v2.buildWorkflowStatusReport(statusState, "2026-01-01T00:00:02.000Z");
    assert.equal(completeReport.slices[0].computedState, "done");
    assert.equal(completeReport.counts.done, 1);
    const formattedStatus = v2.formatWorkflowStatusReport(
      completeReport,
      v2.buildWorkflowStatusOverview([statusState]),
    );
    assert.match(formattedStatus, /001 done evidence=complete/);
    assert.match(formattedStatus, /workflows:/);
  } finally {
    fs.rmSync(statusTmp, { recursive: true, force: true });
  }

  const transitioned = v2.transitionWorkflowPhase(workflow, "discovery", {
    actor: "smoke-test",
    note: "valid transition smoke test",
    evidencePaths: ["artifacts/source-notes/example.md"],
  });
  assert.equal(transitioned.ok, true);
  assert.equal(transitioned.state.phase, "discovery");
  assert.equal(readJson(transitioned.state.paths.stateFile).phase, "discovery");

  const phaseEvent = readEvents(transitioned.state.paths.eventsFile).at(-1);
  assert.equal(phaseEvent.type, "workflow.phase_changed");
  assert.equal(phaseEvent.data.phase, "discovery");
  assert.deepEqual(phaseEvent.data.evidencePaths, ["artifacts/source-notes/example.md"]);

  const missingAgreementTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-missing-agreement-"));
  try {
    const missingAgreementWorkflow = v2.createWorkflow({
      repoCwd: missingAgreementTmp,
      lane: "planning",
      rawInput: "smoke missing agreement workflow",
    });
    const incomplete = {
      ...missingAgreementWorkflow,
      phase: "agreement",
    };
    v2.saveWorkflowState(incomplete);
    const rejected = v2.transitionWorkflowPhase(incomplete, "issue-approval", {
      actor: "smoke-test",
      note: "missing PRD/glossary rejection",
    });
    assert.equal(rejected.ok, false);
    assert.match(rejected.validation.reasons.join("\n"), /PRD draft/);
    assert.match(rejected.validation.reasons.join("\n"), /glossary draft/);

    const readyForIssueApproval = {
      ...missingAgreementWorkflow,
      phase: "agreement",
      agreement: {
        prd: "drafted",
        glossary: "drafted",
        acceptance: "explicit",
        verification: "chosen",
        modulesInterfaces: "named",
        hitlAfk: "labeled",
      },
    };
    writeCompletePlanningArtifacts(readyForIssueApproval);
    v2.saveWorkflowState(readyForIssueApproval);
    const issueApproval = v2.transitionWorkflowPhase(readyForIssueApproval, "issue-approval", {
      actor: "smoke-test",
      note: "complete artifacts advance to issue approval",
    });
    assert.equal(issueApproval.ok, true);
    assert.equal(issueApproval.state.phase, "issue-approval");
  } finally {
    fs.rmSync(missingAgreementTmp, { recursive: true, force: true });
  }

  const missingExecutionEvidenceTmp = fs.mkdtempSync(
    path.join(os.tmpdir(), "pi-autopilot-v2-missing-execution-evidence-"),
  );
  try {
    const missingExecutionEvidenceWorkflow = v2.createWorkflow({
      repoCwd: missingExecutionEvidenceTmp,
      lane: "planning",
      rawInput: "smoke missing execution evidence workflow",
    });
    const incompleteExecution = {
      ...missingExecutionEvidenceWorkflow,
      phase: "triage",
      agreement: {
        prd: "drafted",
        glossary: "drafted",
        acceptance: "explicit",
        verification: "chosen",
        modulesInterfaces: "missing",
        hitlAfk: "labeled",
      },
      gates: {
        "before-issues": { status: "approved" },
        "before-execution": { status: "approved" },
      },
      queue: {
        ...missingExecutionEvidenceWorkflow.queue,
        status: "triaged",
        itemCount: 1,
        readyCount: 1,
      },
    };
    writeCompletePlanningArtifacts(incompleteExecution);
    v2.saveWorkflowState(incompleteExecution);
    const rejectedExecution = v2.transitionWorkflowPhase(incompleteExecution, "execution-approval", {
      actor: "smoke-test",
      note: "missing module/interface evidence rejection",
    });
    assert.equal(rejectedExecution.ok, false);
    assert.match(rejectedExecution.validation.reasons.join("\n"), /affected modules\/interfaces/);
  } finally {
    fs.rmSync(missingExecutionEvidenceTmp, { recursive: true, force: true });
  }

  const templatePrdTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-template-prd-"));
  try {
    const templateWorkflow = v2.createWorkflow({
      repoCwd: templatePrdTmp,
      lane: "planning",
      rawInput: "smoke template prd workflow",
    });
    const templateState = {
      ...templateWorkflow,
      phase: "agreement",
      agreement: {
        prd: "drafted",
        glossary: "drafted",
        acceptance: "explicit",
        verification: "chosen",
        modulesInterfaces: "named",
        hitlAfk: "labeled",
      },
      queue: {
        ...templateWorkflow.queue,
        status: "drafted",
        itemCount: 1,
        readyCount: 1,
      },
    };
    v2.saveWorkflowState(templateState);
    const rejectedTemplate = v2.transitionWorkflowPhase(templateState, "issue-approval", {
      actor: "smoke-test",
      note: "template PRD rejection",
    });
    assert.equal(rejectedTemplate.ok, false);
    assert.match(rejectedTemplate.validation.reasons.join("\n"), /PRD draft/);
  } finally {
    fs.rmSync(templatePrdTmp, { recursive: true, force: true });
  }

  const completeDraftsTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-complete-drafts-"));
  try {
    const completeWorkflow = v2.createWorkflow({
      repoCwd: completeDraftsTmp,
      lane: "planning",
      rawInput: "smoke complete drafts workflow",
    });
    const completeState = {
      ...completeWorkflow,
      phase: "agreement",
      agreement: {
        prd: "drafted",
        glossary: "drafted",
        acceptance: "explicit",
        verification: "chosen",
        modulesInterfaces: "named",
        hitlAfk: "labeled",
      },
      queue: {
        ...completeWorkflow.queue,
        status: "drafted",
        itemCount: 1,
        readyCount: 1,
      },
    };
    writeCompletePlanningArtifacts(completeState);
    v2.saveWorkflowState(completeState);
    const acceptedComplete = v2.transitionWorkflowPhase(completeState, "issue-approval", {
      actor: "smoke-test",
      note: "complete drafts accepted",
    });
    assert.equal(acceptedComplete.ok, true);
  } finally {
    fs.rmSync(completeDraftsTmp, { recursive: true, force: true });
  }

  const horizontalIssueTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-horizontal-issue-"));
  try {
    const horizontalWorkflow = v2.createWorkflow({
      repoCwd: horizontalIssueTmp,
      lane: "planning",
      rawInput: "smoke horizontal issue workflow",
    });
    const horizontalState = {
      ...horizontalWorkflow,
      phase: "agreement",
      agreement: {
        prd: "drafted",
        glossary: "drafted",
        acceptance: "explicit",
        verification: "chosen",
        modulesInterfaces: "named",
        hitlAfk: "labeled",
      },
      queue: {
        ...horizontalWorkflow.queue,
        status: "drafted",
        itemCount: 1,
        readyCount: 1,
      },
    };
    writeCompletePlanningArtifacts(horizontalState, { horizontalIssue: true });
    v2.saveWorkflowState(horizontalState);
    const rejectedHorizontal = v2.transitionWorkflowPhase(horizontalState, "issue-approval", {
      actor: "smoke-test",
      note: "horizontal issue rejection",
    });
    assert.equal(rejectedHorizontal.ok, false);
    assert.match(rejectedHorizontal.validation.reasons.join("\n"), /horizontal/i);
  } finally {
    fs.rmSync(horizontalIssueTmp, { recursive: true, force: true });
  }

  const externalSourceTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-external-source-"));
  try {
    const externalWorkflow = v2.createWorkflow({
      repoCwd: externalSourceTmp,
      lane: "planning",
      rawInput: "https://example.com/spec",
    });
    const externalState = {
      ...externalWorkflow,
      phase: "triage",
      agreement: {
        prd: "drafted",
        glossary: "drafted",
        acceptance: "explicit",
        verification: "chosen",
        modulesInterfaces: "named",
        hitlAfk: "labeled",
      },
      gates: {
        "before-issues": { status: "approved" },
        "before-execution": { status: "approved" },
      },
      queue: {
        ...externalWorkflow.queue,
        status: "triaged",
        itemCount: 1,
        readyCount: 1,
      },
    };
    writeCompletePlanningArtifacts(externalState);
    v2.saveWorkflowState(externalState);
    const rejectedSource = v2.transitionWorkflowPhase(externalState, "execution-approval", {
      actor: "smoke-test",
      note: "missing source note rejection",
    });
    assert.equal(rejectedSource.ok, false);
    assert.match(rejectedSource.validation.reasons.join("\n"), /source/i);
  } finally {
    fs.rmSync(externalSourceTmp, { recursive: true, force: true });
  }

  const uiEvidenceTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-ui-evidence-"));
  try {
    const uiWorkflow = v2.createWorkflow({
      repoCwd: uiEvidenceTmp,
      lane: "planning",
      rawInput: "Add dashboard UI flow",
    });
    const uiState = {
      ...uiWorkflow,
      phase: "triage",
      agreement: {
        prd: "drafted",
        glossary: "drafted",
        acceptance: "explicit",
        verification: "chosen",
        modulesInterfaces: "named",
        hitlAfk: "labeled",
      },
      gates: {
        "before-issues": { status: "approved" },
        "before-execution": { status: "approved" },
      },
      queue: {
        ...uiWorkflow.queue,
        status: "triaged",
        itemCount: 1,
        readyCount: 1,
      },
    };
    writeCompletePlanningArtifacts(uiState, { uiFacing: true });
    v2.saveWorkflowState(uiState);
    const rejectedUi = v2.transitionWorkflowPhase(uiState, "execution-approval", {
      actor: "smoke-test",
      note: "missing UI evidence rejection",
    });
    assert.equal(rejectedUi.ok, false);
    assert.match(rejectedUi.validation.reasons.join("\n"), /visual|screenshot|UI/i);
  } finally {
    fs.rmSync(uiEvidenceTmp, { recursive: true, force: true });
  }

  const trackerBlockedTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-tracker-blocked-"));
  try {
    const trackerBlockedWorkflow = v2.createWorkflow({
      repoCwd: trackerBlockedTmp,
      lane: "planning",
      rawInput: "smoke tracker blocked workflow",
    });
    writeCompletePlanningArtifacts(trackerBlockedWorkflow);
    assert.throws(() => v2.createTrackerIssues(trackerBlockedWorkflow, { dryRun: true }), /before-issues approval/i);
  } finally {
    fs.rmSync(trackerBlockedTmp, { recursive: true, force: true });
  }

  const trackerTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-tracker-"));
  try {
    const trackerWorkflow = v2.createWorkflow({
      repoCwd: trackerTmp,
      lane: "planning",
      rawInput: "smoke tracker workflow",
    });
    const trackerState = {
      ...trackerWorkflow,
      phase: "issue-approval",
      agreement: {
        prd: "drafted",
        glossary: "drafted",
        acceptance: "explicit",
        verification: "chosen",
        modulesInterfaces: "named",
        hitlAfk: "labeled",
      },
      queue: {
        ...trackerWorkflow.queue,
        status: "drafted",
        itemCount: 2,
        readyCount: 1,
      },
    };
    writeCompletePlanningArtifacts(trackerState);
    fs.writeFileSync(
      path.join(trackerState.paths.issuesDir, "drafts", "002-dependent.md"),
      `# Dependent vertical slice

Type: HITL

## What to build

Add a dependent issue after the base validator exists.

## Acceptance criteria

- [ ] Dependent issue sees base tracker ref.

## Verification

\`\`\`bash
npm run check
\`\`\`

## Affected modules/interfaces

- extensions/autopilot/v2.ts tracker issue creation.

## Vertical-slice rationale

This keeps the queue DAG intact while crossing tracker refs and issue metadata.
`,
      "utf8",
    );
    fs.writeFileSync(
      trackerState.queue.queueFile,
      JSON.stringify(
        {
          version: 2,
          workflowId: trackerState.workflowId,
          status: "drafted",
          items: [
            {
              id: "002",
              title: "Dependent vertical slice",
              type: "HITL",
              draft: "issues/drafts/002-dependent.md",
              blockedBy: ["001"],
              verificationProfile: "strict",
              priority: "P1",
            },
            {
              id: "001",
              title: "Validate one vertical workflow slice",
              type: "AFK",
              draft: "issues/drafts/001-smoke.md",
              blockedBy: [],
              verificationProfile: "normal",
              priority: "P0",
            },
          ],
        },
        null,
        2,
      ) + "\n",
      "utf8",
    );
    v2.saveWorkflowState(trackerState);
    const approvedTracker = v2.approveGate(trackerState, "before-issues", "smoke-test", "tracker creation approved");
    const dryRun = v2.createTrackerIssues(approvedTracker, { dryRun: true });
    assert.deepEqual(
      dryRun.operations.map((op) => op.kind),
      ["parent", "child", "child"],
    );
    assert.deepEqual(
      dryRun.operations.filter((op) => op.kind === "child").map((op) => op.issueId),
      ["001", "002"],
    );
    assert.equal(fs.existsSync(path.join(approvedTracker.paths.issuesDir, "created", "001.json")), false);

    const created = v2.createTrackerIssues(approvedTracker, { archiveDrafts: true });
    assert.equal(created.dryRun, false);
    assert.equal(fs.existsSync(path.join(approvedTracker.paths.issuesDir, "created", "000-parent-prd.json")), true);
    assert.equal(readJson(path.join(approvedTracker.paths.issuesDir, "created", "001.json")).sliceKind, "vertical");
    assert.equal(readJson(path.join(approvedTracker.paths.issuesDir, "created", "002.json")).type, "HITL");
    const updatedQueue = readJson(approvedTracker.queue.queueFile);
    assert.equal(updatedQueue.status, "triaged");
    assert.equal(updatedQueue.items[0].id, "001");
    assert.equal(updatedQueue.items[0].trackerRef, "local:autopilot-v2-001");
    assert.equal(updatedQueue.items[0].moduleInterfaceNotes.includes("validateWorkflowTransition"), true);
    assert.equal(fs.existsSync(path.join(approvedTracker.paths.issuesDir, "drafts", ".archived.json")), true);
  } finally {
    fs.rmSync(trackerTmp, { recursive: true, force: true });
  }

  const executionBlockedTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-execution-blocked-"));
  try {
    const executionBlockedWorkflow = v2.createWorkflow({
      repoCwd: executionBlockedTmp,
      lane: "planning",
      rawInput: "smoke execution blocked workflow",
    });
    writeCompletePlanningArtifacts(executionBlockedWorkflow, { sourceNotes: true });
    assert.throws(() => v2.claimNextExecutionIssue(executionBlockedWorkflow), /before-execution approval/i);
  } finally {
    fs.rmSync(executionBlockedTmp, { recursive: true, force: true });
  }

  const executionTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-execution-"));
  try {
    const executionWorkflow = v2.createWorkflow({
      repoCwd: executionTmp,
      lane: "planning",
      rawInput: "smoke execution workflow with TRANSCRIPT_SHOULD_NOT_APPEAR in raw source",
    });
    const executionState = makeExecutionApprovedState(executionWorkflow);
    writeCompletePlanningArtifacts(executionState, { sourceNotes: true });
    v2.saveWorkflowState(executionState);

    const claim = v2.claimNextExecutionIssue(executionState, { issueId: "001", actor: "smoke-test" });
    assert.equal(claim.issue.id, "001");
    assert.equal(claim.executionState.status, "claimed");
    assert.equal(fs.existsSync(claim.executionState.worktreePath), true);
    assert.match(claim.executionState.branch, /^autopilot\//);
    assert.equal(fs.existsSync(claim.contextPacketPath), true);
    assert.equal(fs.existsSync(claim.implementationPromptPath), true);
    assert.equal(fs.existsSync(claim.reviewPromptPath), true);

    const packetText = fs.readFileSync(claim.contextPacketPath, "utf8");
    assert.equal(packetText.includes("TRANSCRIPT_SHOULD_NOT_APPEAR"), false);
    assert.match(packetText, /Source Notes/);

    const implementationPrompt = fs.readFileSync(claim.implementationPromptPath, "utf8");
    assert.match(implementationPrompt, /red-green-refactor/i);
    assert.match(implementationPrompt, /Reject test-after-the-fact shortcuts/i);

    const reviewPrompt = fs.readFileSync(claim.reviewPromptPath, "utf8");
    assert.match(reviewPrompt, /Fresh-context review/i);

    const workerRecorded = v2.recordWorkerRun(executionState, "001", {
      command: "npm run check",
      cwd: claim.executionState.worktreePath,
      startedAt: "2026-01-01T00:00:00.000Z",
      finishedAt: "2026-01-01T00:00:01.000Z",
      exitCode: 0,
      stdout: "ok",
      stderr: "",
      logPath: "artifacts/worker-logs/001.log",
    });
    assert.equal(workerRecorded.worker.command, "npm run check");

    const reviewerPacket = v2.buildReviewerPacket(executionState, "001");
    assert.equal(reviewerPacket.reviewMode, "report-only");
    assert.equal(reviewerPacket.mayEditFiles, false);

    const queueAfterClaim = readJson(executionState.queue.queueFile);
    assert.equal(queueAfterClaim.items.filter((item) => item.status === "claimed").length, 1);

    const failedPr = () => v2.openExecutionPullRequest(executionState, "001");
    assert.throws(failedPr, /red test evidence/i);

    const verified = v2.recordExecutionVerification(executionState, "001", {
      redTestObserved: true,
      checksPassed: true,
      evidence: ["green: smoke test passed after implementation"],
      redEvidence: ["red: smoke test failed before implementation"],
      greenEvidence: ["green: smoke test passed after implementation"],
      refactorNotes: ["Refactored after green without behavior change."],
      commands: ["node extensions/autopilot/v2-smoke-test.mjs"],
      evidencePaths: ["artifacts/worker-logs/001.log"],
    });
    assert.equal(verified.phase, "verification");
    assert.deepEqual(v2.validateExecutionHandoff(executionState, "001"), []);
    const reviewed = v2.recordFreshContextReview(executionState, "001", {
      status: "passed",
      findings: [],
      reportPath: "artifacts/reviews/001.md",
    });
    assert.equal(reviewed.review.status, "passed");
    const pr = v2.openExecutionPullRequest(executionState, "001", { title: "Smoke PR" });
    assert.equal(pr.status, "pr-open");
    assert.match(pr.prUrl, /^local-pr:\/\//);
    assert.equal(readJson(path.join(executionState.paths.issuesDir, "prs", "001.json")).mergeAuthority, false);
  } finally {
    fs.rmSync(executionTmp, { recursive: true, force: true });
  }

  const failureTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-failure-"));
  try {
    const failureWorkflow = v2.createWorkflow({
      repoCwd: failureTmp,
      lane: "planning",
      rawInput: "smoke repeated failure workflow",
    });
    const failureState = makeExecutionApprovedState(failureWorkflow);
    writeCompletePlanningArtifacts(failureState);
    v2.saveWorkflowState(failureState);
    v2.claimNextExecutionIssue(failureState, { issueId: "001", actor: "smoke-test" });

    const repairOne = v2.recordRepairAttempt(failureState, "001", {
      status: "blocked",
      findings: ["Reviewer found missing edge-case evidence."],
      maxAttempts: 2,
    });
    assert.equal(repairOne.phase, "repair");
    const repairTwo = v2.recordRepairAttempt(failureState, "001", {
      status: "blocked",
      findings: ["Reviewer finding still unresolved."],
      maxAttempts: 2,
    });
    assert.equal(repairTwo.phase, "blocked");

    const firstFailure = v2.recordExecutionFailure(failureState, "001", {
      nonAmbiguous: true,
      message: "Typecheck still fails after implementation.",
    });
    assert.equal(firstFailure.phase, "debug");
    assert.equal(firstFailure.status, "running");

    const secondFailure = v2.recordExecutionFailure(failureState, "001", {
      nonAmbiguous: true,
      message: "Same typecheck failure after debug pass.",
    });
    assert.equal(secondFailure.phase, "blocked");
    assert.equal(secondFailure.status, "blocked");

    const ambiguousFailure = v2.recordExecutionFailure(failureState, "001", {
      ambiguous: true,
      message: "Acceptance criteria unclear.",
    });
    assert.equal(ambiguousFailure.lastFailure.route, "grill");

    const designFailure = v2.recordExecutionFailure(failureState, "001", {
      designFriction: true,
      message: "Module boundary is wrong.",
    });
    assert.equal(designFailure.lastFailure.route, "architecture");
  } finally {
    fs.rmSync(failureTmp, { recursive: true, force: true });
  }

  const schedulerTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-scheduler-"));
  try {
    const schedulerWorkflow = v2.createWorkflow({
      repoCwd: schedulerTmp,
      lane: "planning",
      rawInput: "smoke scheduler workflow",
    });
    const schedulerState = makeExecutionApprovedState(schedulerWorkflow);
    writeCompletePlanningArtifacts(schedulerState);
    fs.writeFileSync(
      path.join(schedulerState.paths.issuesDir, "drafts", "002-smoke.md"),
      fs
        .readFileSync(path.join(schedulerState.paths.issuesDir, "drafts", "001-smoke.md"), "utf8")
        .replace("Validate one vertical workflow slice", "Validate sibling vertical workflow slice"),
      "utf8",
    );
    fs.writeFileSync(
      path.join(schedulerState.paths.issuesDir, "drafts", "003-smoke.md"),
      fs
        .readFileSync(path.join(schedulerState.paths.issuesDir, "drafts", "001-smoke.md"), "utf8")
        .replace("Validate one vertical workflow slice", "Validate dependent vertical workflow slice"),
      "utf8",
    );
    fs.writeFileSync(
      schedulerState.queue.queueFile,
      JSON.stringify(
        {
          version: 2,
          workflowId: schedulerState.workflowId,
          status: "triaged",
          items: [
            {
              id: "001",
              title: "Ready P1",
              type: "AFK",
              priority: "P1",
              status: "queued",
              draft: "issues/drafts/001-smoke.md",
              blockedBy: [],
              verificationProfile: "strict",
            },
            {
              id: "002",
              title: "Ready P0",
              type: "AFK",
              priority: "P0",
              status: "queued",
              draft: "issues/drafts/002-smoke.md",
              blockedBy: [],
              verificationProfile: "strict",
            },
            {
              id: "003",
              title: "Dependent",
              type: "AFK",
              priority: "P0",
              status: "queued",
              draft: "issues/drafts/003-smoke.md",
              blockedBy: ["001"],
              verificationProfile: "strict",
            },
            {
              id: "004",
              title: "HITL skip",
              type: "HITL",
              priority: "P0",
              status: "queued",
              draft: "issues/drafts/003-smoke.md",
              blockedBy: [],
              verificationProfile: "strict",
            },
          ],
        },
        null,
        2,
      ) + "\n",
      "utf8",
    );
    v2.saveWorkflowState(schedulerState);
    const round = await v2.runSchedulerRound(schedulerState, {
      concurrency: 2,
      executeIssue: async (issue) => {
        if (issue.id === "002") throw new Error("sibling failed");
        return {
          status: "done",
          evidencePaths: [`evidence/${issue.id}.md`],
          branch: `autopilot/${issue.id}`,
          attempts: 1,
        };
      },
    });
    assert.deepEqual(
      round.selected.map((item) => item.id),
      ["002", "001"],
    );
    const schedulerQueue = readJson(schedulerState.queue.queueFile);
    assert.equal(schedulerQueue.items.find((item) => item.id === "001").status, "done");
    assert.equal(schedulerQueue.items.find((item) => item.id === "002").status, "blocked");
    assert.equal(schedulerQueue.items.find((item) => item.id === "003").status, "queued");
    assert.equal(schedulerQueue.readyCount, 1);

    const integration = v2.recordIntegrationResult(schedulerState, {
      integrationBranch: "autopilot/integration/smoke",
      completedIssues: ["001"],
      blockedIssues: ["002"],
      status: "blocked",
      commands: ["git merge autopilot/001", "npm run check"],
      conflicts: ["002 blocked before integration"],
      evidencePaths: ["artifacts/integration/result.log"],
      reviewerReports: ["artifacts/reviews/001.md"],
      diffPaths: ["artifacts/diffs/001.diff"],
    });
    assert.equal(fs.existsSync(integration.resultPath), true);
    assert.equal(readJson(integration.resultPath).targetBranchMergeAuthority, false);
    assert.equal(fs.existsSync(integration.handoffPath), true);
    assert.match(integration.handoff.highlights.openRisks.join("\n"), /002 blocked/);
  } finally {
    fs.rmSync(schedulerTmp, { recursive: true, force: true });
  }

  const approvalTmp = fs.mkdtempSync(path.join(os.tmpdir(), "pi-autopilot-v2-approval-"));
  try {
    const approvalWorkflow = v2.createWorkflow({
      repoCwd: approvalTmp,
      lane: "planning",
      rawInput: "smoke approval workflow",
    });
    const rejectedApproval = v2.approveGate(
      approvalWorkflow,
      "before-issues",
      "smoke-test",
      "should reject from grill",
    );
    assert.equal(rejectedApproval.phase, "grill");
    assert.equal(fs.existsSync(path.join(approvalWorkflow.paths.approvalsDir, "before-issues.json")), false);

    const readyForIssues = {
      ...approvalWorkflow,
      phase: "issue-approval",
      agreement: {
        prd: "drafted",
        glossary: "drafted",
        acceptance: "explicit",
        verification: "chosen",
        modulesInterfaces: "named",
        hitlAfk: "labeled",
      },
      queue: {
        ...approvalWorkflow.queue,
        status: "drafted",
        itemCount: 1,
        readyCount: 1,
      },
    };
    writeCompletePlanningArtifacts(readyForIssues);
    v2.saveWorkflowState(readyForIssues);
    const approved = v2.approveGate(readyForIssues, "before-issues", "smoke-test", "valid approval smoke test");
    assert.equal(approved.phase, "issues-created");
    assert.equal(approved.gates["before-issues"].status, "approved");
    assert.equal(fs.existsSync(path.join(approvalWorkflow.paths.approvalsDir, "before-issues.json")), true);
  } finally {
    fs.rmSync(approvalTmp, { recursive: true, force: true });
  }
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}

console.log("v2 transition smoke tests passed");
