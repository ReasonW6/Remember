import type { Flow, FlowSummary, SavedFlow } from "./types";

interface BuildFlowOptionsInput {
  flow: Flow;
  flowSummaries: FlowSummary[];
  selectedFileName: string;
  retainedDraft: SavedFlow | null;
}

export function buildFlowOptions({
  flow,
  flowSummaries,
  selectedFileName,
  retainedDraft,
}: BuildFlowOptionsInput): FlowSummary[] {
  const options = flowSummaries.some(
    (summary) => summary.fileName === selectedFileName,
  )
    ? [...flowSummaries]
    : [flowSummaryFromFlow(selectedFileName, flow), ...flowSummaries];

  if (
    retainedDraft &&
    !options.some((summary) => summary.fileName === retainedDraft.fileName)
  ) {
    return [flowSummaryFromSavedFlow(retainedDraft), ...options];
  }

  return options;
}

function flowSummaryFromSavedFlow(savedFlow: SavedFlow): FlowSummary {
  return {
    fileName: savedFlow.fileName,
    name: savedFlow.flow.name,
    displayName: savedFlow.flow.displayName,
    stepCount: savedFlow.flow.steps.length,
    savedAt: savedFlow.savedAt,
    isValid: true,
    error: null,
  };
}

function flowSummaryFromFlow(fileName: string, flow: Flow): FlowSummary {
  return {
    fileName,
    name: flow.name,
    displayName: flow.displayName,
    stepCount: flow.steps.length,
    savedAt: 0,
    isValid: true,
    error: null,
  };
}
