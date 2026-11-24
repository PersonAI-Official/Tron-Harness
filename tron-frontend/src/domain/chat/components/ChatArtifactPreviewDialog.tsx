import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type * as Ui from "@personai/agent-ui-chat-core";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { requestApiBlob } from "@/shared/api/httpClient";

type PreviewKind = "image" | "pdf" | "html" | "text" | "unsupported";
type PreviewContent = { kind: PreviewKind; url?: string; text?: string };

function artifactExtension(item: Ui.ChatArtifactItem) {
  return item.display.split(".").at(-1)?.toLowerCase() ?? "";
}

function artifactMimeType(item: Ui.ChatArtifactItem) {
  const mediaType = item.mediaType?.split(";")[0]?.trim().toLowerCase();
  if (mediaType && mediaType !== "application/octet-stream") return mediaType;
  switch (artifactExtension(item)) {
    case "html": case "htm": return "text/html";
    case "pdf": return "application/pdf";
    case "png": return "image/png";
    case "jpg": case "jpeg": return "image/jpeg";
    case "webp": return "image/webp";
    case "gif": return "image/gif";
    case "svg": return "image/svg+xml";
    case "txt": case "log": case "md": case "markdown": return "text/plain";
    case "json": return "application/json";
    case "xml": case "csv": case "tsv": case "yaml": case "yml": return "text/plain";
    default: return mediaType ?? "application/octet-stream";
  }
}

function previewKind(mimeType: string): PreviewKind {
  if (mimeType.startsWith("image/")) return "image";
  if (mimeType === "application/pdf") return "pdf";
  if (mimeType === "text/html" || mimeType === "application/xhtml+xml") return "html";
  if (mimeType.startsWith("text/") || mimeType === "application/json" || mimeType.endsWith("+xml")) return "text";
  return "unsupported";
}

export function ChatArtifactPreviewDialog({ open, onOpenChange, sessionCode, workspaceCode, runCode, item }: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sessionCode: string;
  workspaceCode: string;
  runCode: string;
  item: Ui.ChatArtifactItem;
}) {
  const { t } = useTranslation("pages");
  const mimeType = artifactMimeType(item);
  const kind = previewKind(mimeType);
  const [attempt, setAttempt] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [content, setContent] = useState<PreviewContent | null>(null);

  useEffect(() => {
    if (!open || kind === "unsupported") {
      setLoading(false);
      setError(null);
      setContent(null);
      return;
    }
    if (!workspaceCode.trim()) {
      setLoading(false);
      setError(t("chat.artifactPreview.workspaceMissing"));
      setContent(null);
      return;
    }
    if (!sessionCode.trim() || !runCode.trim()) {
      setLoading(false);
      setError(t("chat.artifactPreview.failed"));
      setContent(null);
      return;
    }

    const controller = new AbortController();
    let objectUrl: string | undefined;
    setLoading(true);
    setError(null);
    setContent(null);

    void requestApiBlob("GET /api/v1/chats/sessions/{sessionCode}/artifacts/{runCode}/{itemCode}/content", {
      pathParams: { sessionCode, runCode, itemCode: item.id },
      queryParams: { disposition: "attachment", workspaceCode },
      signal: controller.signal,
    }).then(async (response) => {
      if (controller.signal.aborted) return;
      if (kind === "text") {
        setContent({ kind, text: await response.blob.text() });
      } else {
        objectUrl = URL.createObjectURL(new Blob([response.blob], { type: mimeType }));
        setContent({ kind, url: objectUrl });
      }
    }).catch((cause: unknown) => {
      if (controller.signal.aborted) return;
      setError(cause instanceof Error ? cause.message : t("chat.artifactPreview.failed"));
    }).finally(() => {
      if (!controller.signal.aborted) setLoading(false);
    });

    return () => {
      controller.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [attempt, item.id, kind, mimeType, open, runCode, sessionCode, t, workspaceCode]);

  const retry = () => setAttempt((value) => value + 1);

  return <Dialog open={open} onOpenChange={onOpenChange}>
    <DialogContent className="flex h-[min(88dvh,1000px)] w-[min(96vw,1200px)] max-w-none flex-col overflow-hidden sm:max-w-5xl">
      <DialogHeader>
        <DialogTitle>{t("chat.artifactPreview.title", { name: item.display })}</DialogTitle>
      </DialogHeader>
      <div className="min-h-0 flex-1 overflow-auto">
        {loading ? <p role="status" className="p-4">{t("chat.artifactPreview.loading")}</p> : null}
        {error ? <div className="space-y-3 p-4" role="alert">
          <p>{error}</p>
          <Button variant="outline" onClick={retry}>{t("chat.artifactPreview.retry")}</Button>
        </div> : null}
        {!loading && !error && kind === "unsupported" ? <p className="p-4">{t("chat.artifactPreview.unsupported")}</p> : null}
        {!loading && !error && content?.kind === "image" && content.url
          ? <div className="flex h-full items-center justify-center p-3"><img src={content.url} alt={item.display} className="max-h-full max-w-full object-contain" /></div>
          : null}
        {!loading && !error && content?.kind === "pdf" && content.url
          ? <iframe title={item.display} src={content.url} className="h-full min-h-96 w-full border-0" />
          : null}
        {!loading && !error && content?.kind === "html" && content.url
          ? <iframe title={item.display} src={content.url} sandbox="allow-scripts" referrerPolicy="no-referrer" className="h-full min-h-96 w-full border-0" />
          : null}
        {!loading && !error && content?.kind === "text"
          ? <pre className="h-full overflow-auto whitespace-pre-wrap break-words p-4 font-mono text-sm">{content.text}</pre>
          : null}
      </div>
    </DialogContent>
  </Dialog>;
}
