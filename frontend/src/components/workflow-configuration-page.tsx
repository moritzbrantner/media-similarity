import {
  WorkflowWorkbench,
  type WorkflowEditorDocument,
  type WorkflowEditorNode,
} from "@moritzbrantner/workflow-editor";
import { Button } from "@moritzbrantner/ui";
import { AlertCircle, CheckCircle2, Database, Loader2, RotateCcw, Save } from "lucide-react";
import { useEffect, useState } from "react";
import { formatIndexSummary } from "../indexing/summary";
import type {
  IndexResponse,
  MediaWorkflowNodeData,
  WorkflowConfigResponse,
  WorkflowDiagnostic,
  WorkflowEditorLibrary,
} from "../types";
import { Message } from "./status-message";

export function WorkflowConfigurationPage({
  config,
  error,
  indexError,
  indexPending,
  lastIndex,
  loading,
  onIndex,
  onReset,
  onSave,
  onValidate,
  resetPending,
  saveError,
  savePending,
  saveSuccess,
  validateError,
  validatePending,
}: {
  config: WorkflowConfigResponse | null;
  error: Error | null;
  indexError: Error | null;
  indexPending: boolean;
  lastIndex: IndexResponse | null;
  loading: boolean;
  onIndex: () => void;
  onReset: () => void;
  onSave: (library: WorkflowEditorLibrary<MediaWorkflowNodeData>) => void;
  onValidate: (
    library: WorkflowEditorLibrary<MediaWorkflowNodeData>,
  ) => Promise<WorkflowDiagnostic[]>;
  resetPending: boolean;
  saveError: Error | null;
  savePending: boolean;
  saveSuccess: boolean;
  validateError: Error | null;
  validatePending: boolean;
}) {
  const [draft, setDraft] = useState<WorkflowEditorLibrary<MediaWorkflowNodeData> | null>(null);
  const [activeDocumentId, setActiveDocumentId] = useState<string | null>(null);
  const [validationDiagnostics, setValidationDiagnostics] = useState<WorkflowDiagnostic[] | null>(
    null,
  );
  const [configTextByNode, setConfigTextByNode] = useState<Record<string, string>>({});
  const [configErrorByNode, setConfigErrorByNode] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!config) {
      return;
    }
    setDraft(config.library);
    setActiveDocumentId(config.library.activeDocumentId ?? config.library.documents[0]?.id ?? null);
    setValidationDiagnostics(null);
    setConfigTextByNode({});
    setConfigErrorByNode({});
  }, [config]);

  async function validateDraft() {
    if (!draft) {
      return;
    }
    const diagnostics = await onValidate(draft);
    setValidationDiagnostics(diagnostics);
  }

  function selectDocument(documentId: string) {
    setActiveDocumentId(documentId);
    setDraft((current) => (current ? { ...current, activeDocumentId: documentId } : current));
  }

  function updateActiveDocument(document: WorkflowEditorDocument<MediaWorkflowNodeData>) {
    if (!draft || !activeDocumentId) {
      return;
    }
    setDraft({
      ...draft,
      activeDocumentId,
      documents: draft.documents.map((entry) =>
        entry.id === activeDocumentId
          ? { ...entry, document, updatedAt: new Date().toISOString() }
          : entry,
      ),
    });
    setValidationDiagnostics(null);
  }

  function renderInspector({
    readOnly,
    selectedNode,
    updateSelectedNode,
  }: {
    readOnly: boolean;
    selectedNode?: WorkflowEditorNode<MediaWorkflowNodeData>;
    updateSelectedNode: (patch: Partial<WorkflowEditorNode<MediaWorkflowNodeData>>) => void;
  }) {
    if (!selectedNode) {
      return (
        <div className="grid gap-2 p-3 text-sm text-neutral-600">
          <span>Select a processor to edit its workflow settings.</span>
        </div>
      );
    }

    const nodeData = selectedNode.data ?? { processor: selectedNode.kind ?? selectedNode.id };
    const enabled = nodeData.enabled ?? true;
    const configText =
      configTextByNode[selectedNode.id] ?? JSON.stringify(nodeData.config ?? {}, null, 2);
    const configError = configErrorByNode[selectedNode.id];

    return (
      <div className="grid gap-4 p-3">
        <div>
          <h4 className="text-sm font-semibold text-neutral-950">{selectedNode.label}</h4>
          <p className="mt-1 text-xs text-neutral-600">{nodeData.processor}</p>
        </div>

        <label className="flex items-center gap-2 text-sm font-medium text-neutral-800">
          <input
            checked={enabled}
            className="size-4"
            disabled={readOnly || nodeData.locked}
            onChange={(event) =>
              updateSelectedNode({
                data: {
                  ...nodeData,
                  enabled: event.currentTarget.checked,
                },
              })
            }
            type="checkbox"
          />
          Enabled
        </label>

        <label className="grid gap-2 text-sm font-medium text-neutral-800">
          Config
          <textarea
            className="min-h-40 rounded-md border border-neutral-300 bg-white p-2 font-mono text-xs leading-5 text-neutral-900 shadow-sm disabled:bg-neutral-100"
            disabled={readOnly}
            onBlur={() => {
              try {
                const parsed = configText.trim() ? JSON.parse(configText) : {};
                updateSelectedNode({
                  data: {
                    ...nodeData,
                    config: parsed,
                  },
                });
                setConfigErrorByNode((current) => ({ ...current, [selectedNode.id]: "" }));
              } catch (caught) {
                setConfigErrorByNode((current) => ({
                  ...current,
                  [selectedNode.id]:
                    caught instanceof Error ? caught.message : "Config must be valid JSON.",
                }));
              }
            }}
            onChange={(event) =>
              setConfigTextByNode((current) => ({
                ...current,
                [selectedNode.id]: event.currentTarget.value,
              }))
            }
            spellCheck={false}
            value={configText}
          />
          {configError ? <span className="text-xs text-red-700">{configError}</span> : null}
        </label>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="grid min-h-96 place-items-center rounded-lg border border-neutral-300 bg-white text-neutral-600 shadow-sm">
        <Loader2 className="size-7 animate-spin" aria-label="Loading workflow editor" />
      </div>
    );
  }

  if (error) {
    return <Message icon={<AlertCircle className="size-4" />} text={error.message} tone="error" />;
  }

  if (!config || !draft) {
    return null;
  }

  const diagnostics = validationDiagnostics ?? config.diagnostics;
  const canWrite = config.writable && !savePending && !resetPending;
  const activeEntry =
    draft.documents.find((document) => document.id === activeDocumentId) ?? draft.documents[0];
  const activeDocument = activeEntry?.document;
  const editorKey = `${config.workflow_file}:${activeEntry?.id ?? "none"}`;
  const documentStats = activeDocument
    ? `${activeDocument.nodes.length} processor(s), ${activeDocument.edges.length} connection(s)`
    : "No workflow document";

  return (
    <section className="grid min-h-[720px] gap-5 xl:grid-cols-[minmax(0,1fr)_340px]">
      <div className="flex min-w-0 flex-col gap-3">
        <div className="flex flex-col gap-3 rounded-lg border border-neutral-300 bg-white p-4 shadow-sm sm:flex-row sm:items-center sm:justify-between">
          <div className="min-w-0">
            <h2 className="text-lg font-semibold text-neutral-950">Processing Workflows</h2>
            <p className="mt-1 truncate text-sm text-neutral-600" title={config.workflow_file}>
              Stored in {config.workflow_file}
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-not-allowed disabled:opacity-60"
              disabled={validatePending}
              onClick={validateDraft}
              type="button"
              variant="outline"
            >
              {validatePending ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <CheckCircle2 className="size-4" aria-hidden="true" />
              )}
              <span>Validate</span>
            </Button>
            <Button
              className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-not-allowed disabled:opacity-60"
              disabled={resetPending}
              onClick={onReset}
              type="button"
              variant="outline"
            >
              {resetPending ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <RotateCcw className="size-4" aria-hidden="true" />
              )}
              <span>Reset</span>
            </Button>
            <Button
              className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-neutral-300 bg-white px-3 text-sm font-semibold text-neutral-800 transition hover:border-neutral-500 hover:bg-neutral-50 disabled:cursor-wait disabled:opacity-60"
              disabled={indexPending}
              onClick={onIndex}
              type="button"
              variant="outline"
            >
              {indexPending ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Database className="size-4" aria-hidden="true" />
              )}
              <span>Index Sources</span>
            </Button>
            <Button
              className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-emerald-700 px-4 text-sm font-semibold text-white shadow-sm transition hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-60"
              disabled={!canWrite}
              onClick={() => onSave(draft)}
              type="button"
            >
              {savePending ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Save className="size-4" aria-hidden="true" />
              )}
              <span>Save</span>
            </Button>
          </div>
        </div>

        <div className="flex flex-col gap-3 rounded-lg border border-neutral-300 bg-white p-3 shadow-sm sm:flex-row sm:items-center sm:justify-between">
          <label className="flex min-w-64 flex-col gap-1 text-sm font-medium text-neutral-800">
            Workflow
            <select
              aria-label="Workflow document"
              className="h-10 rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-950 shadow-sm"
              onChange={(event) => selectDocument(event.currentTarget.value)}
              value={activeEntry?.id ?? ""}
            >
              {draft.documents.map((document) => (
                <option key={document.id} value={document.id}>
                  {document.name}
                </option>
              ))}
            </select>
          </label>
          <div className="text-sm text-neutral-600">{documentStats}</div>
        </div>

        {activeDocument ? (
          <div className="min-h-[640px] overflow-hidden rounded-lg border border-neutral-300 bg-white shadow-sm">
            <WorkflowWorkbench<MediaWorkflowNodeData>
              className="h-[640px]"
              document={activeDocument}
              key={editorKey}
              nodeTemplates={config.node_templates}
              onDocumentChange={updateActiveDocument}
              readOnly={!config.writable}
              renderInspector={renderInspector}
            />
          </div>
        ) : (
          <Message
            icon={<AlertCircle className="size-4" />}
            text="No workflow documents are available."
            tone="error"
          />
        )}
      </div>

      <aside className="flex h-fit flex-col gap-4">
        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <h3 className="text-sm font-semibold text-neutral-950">Status</h3>
          <div className="mt-3 grid gap-2">
            {!config.writable ? (
              <Message
                icon={<AlertCircle className="size-4" />}
                text="Workflow file is not writable."
                tone="warn"
              />
            ) : savePending ? (
              <Message
                icon={<Loader2 className="size-4 animate-spin" />}
                text="Saving workflows."
                tone="info"
              />
            ) : saveError ? (
              <Message
                icon={<AlertCircle className="size-4" />}
                text={saveError.message}
                tone="error"
              />
            ) : saveSuccess ? (
              <Message
                icon={<CheckCircle2 className="size-4" />}
                text="Saved workflows."
                tone="ok"
              />
            ) : (
              <Message
                icon={<CheckCircle2 className="size-4" />}
                text="Workflow editor ready."
                tone="info"
              />
            )}
            {validateError ? (
              <Message
                icon={<AlertCircle className="size-4" />}
                text={validateError.message}
                tone="error"
              />
            ) : null}
            {indexError ? (
              <Message
                icon={<AlertCircle className="size-4" />}
                text={indexError.message}
                tone="error"
              />
            ) : lastIndex ? (
              <Message
                icon={<CheckCircle2 className="size-4" />}
                text={formatIndexSummary(lastIndex)}
                tone={lastIndex.failed > 0 ? "warn" : "ok"}
              />
            ) : null}
          </div>
        </section>

        <section className="rounded-lg border border-neutral-300 bg-white p-4 shadow-sm">
          <h3 className="text-sm font-semibold text-neutral-950">Diagnostics</h3>
          <div className="mt-3 grid gap-2">
            {diagnostics.length === 0 ? (
              <Message
                icon={<CheckCircle2 className="size-4" />}
                text="No workflow diagnostics."
                tone="ok"
              />
            ) : (
              diagnostics.map((diagnostic, index) => (
                <Message
                  icon={<AlertCircle className="size-4" />}
                  key={`${diagnostic.code}:${diagnostic.document_id ?? "library"}:${index}`}
                  text={diagnostic.message}
                  tone="error"
                />
              ))
            )}
          </div>
        </section>
      </aside>
    </section>
  );
}
