import type { Flow, FlowStep } from "./types";

export function selectExistingStepId(
  flow: Flow,
  preferredStepId: number | null,
) {
  if (
    preferredStepId !== null &&
    flow.steps.some((step) => step.id === preferredStepId)
  ) {
    return preferredStepId;
  }

  return flow.steps[0]?.id ?? null;
}

export function updateStepDelayMs(
  flow: Flow,
  stepId: number,
  delayMs: number,
): Flow {
  const normalizedDelayMs = Math.max(0, Math.round(delayMs));
  return updateFlowStep(flow, stepId, (step) => {
    if (step.type === "wait") {
      return {
        ...step,
        delayMs: normalizedDelayMs,
        durationMs: normalizedDelayMs,
      };
    }

    return {
      ...step,
      delayMs: normalizedDelayMs,
    };
  });
}

export function updateStepClickCoordinates(
  flow: Flow,
  stepId: number,
  x: number,
  y: number,
): Flow {
  const normalizedX = Math.round(x);
  const normalizedY = Math.round(y);

  return updateFlowStep(flow, stepId, (step) => {
    if (step.type !== "click") return step;

    return {
      ...step,
      x: normalizedX,
      y: normalizedY,
      target: `(${normalizedX}, ${normalizedY}) [屏幕绝对]`,
    };
  });
}

export function updateStepText(flow: Flow, stepId: number, text: string): Flow {
  return updateFlowStep(flow, stepId, (step) => {
    if (step.type !== "type") return step;

    return {
      ...step,
      text,
    };
  });
}

export function updateStepHotkeyText(
  flow: Flow,
  stepId: number,
  hotkeyText: string,
): Flow {
  const keys = hotkeyText
    .split("+")
    .map((key) => key.trim())
    .filter(Boolean);

  return updateFlowStep(flow, stepId, (step) => {
    if (step.type !== "hotkey") return step;

    return {
      ...step,
      keys,
    };
  });
}

export function updateTargetWindowMatched(flow: Flow, matched: boolean): Flow {
  return {
    ...flow,
    targetWindow: {
      ...flow.targetWindow,
      matched,
    },
  };
}

export function insertWaitStepAfter(
  flow: Flow,
  afterStepId: number | null,
  durationMs = 500,
) {
  const normalizedDurationMs = Math.max(0, Math.round(durationMs));
  const nextStepId =
    flow.steps.reduce((maxId, step) => Math.max(maxId, step.id), 0) + 1;
  const waitStep: FlowStep = {
    id: nextStepId,
    type: "wait",
    action: "等待",
    durationMs: normalizedDurationMs,
    delayMs: normalizedDurationMs,
    note: "插入等待",
  };
  const selectedIndex =
    afterStepId === null
      ? -1
      : flow.steps.findIndex((step) => step.id === afterStepId);
  const insertIndex = selectedIndex === -1 ? flow.steps.length : selectedIndex + 1;

  return {
    flow: {
      ...flow,
      steps: [
        ...flow.steps.slice(0, insertIndex),
        waitStep,
        ...flow.steps.slice(insertIndex),
      ],
    },
    selectedStepId: waitStep.id,
  };
}

export function deleteStep(flow: Flow, stepId: number) {
  const deletedIndex = flow.steps.findIndex((step) => step.id === stepId);
  if (deletedIndex === -1) {
    return {
      flow,
      selectedStepId: flow.steps[0]?.id ?? null,
    };
  }

  const nextSteps = flow.steps.filter((step) => step.id !== stepId);
  const fallbackStep = nextSteps[deletedIndex] ?? nextSteps[deletedIndex - 1];

  return {
    flow: {
      ...flow,
      steps: nextSteps,
    },
    selectedStepId: fallbackStep?.id ?? null,
  };
}

function updateFlowStep(
  flow: Flow,
  stepId: number,
  update: (step: FlowStep) => FlowStep,
): Flow {
  return {
    ...flow,
    steps: flow.steps.map((step) => (step.id === stepId ? update(step) : step)),
  };
}
