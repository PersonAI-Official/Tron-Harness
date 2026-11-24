import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type * as Ui from "@personai/agent-ui-chat-core";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { requestApiBlob } from "@/shared/api/httpClient";
import { resolveArtifactPreview } from "@tron/chat-agent-ui-adapter";

type PreviewKind = ReturnType<typeof resolveArtifactPreview>["kind"];
type PreviewContent = { kind: PreviewKind; url?: string; text?: string };

function themedHtml(html: string) {
  const rootStyle = window.getComputedStyle(document.documentElement);
  const resolveColor = (variable: string, fallback: string) => {
    const value = rootStyle.getPropertyValue(variable).trim();
    return value && CSS.supports("color", value) ? value : fallback;
  };
  const background = resolveColor("--color-surface", "Canvas");
  const foreground = resolveColor("--color-on-surface", "CanvasText");
  const colorScheme = document.documentElement.dataset.theme === "matrix-dark" ? "dark" : "light";
  const style = `<style data-tron-artifact-preview-theme>html,body{background-color:${background}!important;color:${foreground}!important;color-scheme:${colorScheme}!important}body{min-height:100vh}</style>`;
  const head = /<head\b[^>]*>/i.exec(html);
  if (head?.index !== undefined) {
    const insertAt = head.index + head[0].length;
    return `${html.slice(0, insertAt)}${style}${html.slice(insertAt)}`;
  }
  const root = /<html\b[^>]*>/i.exec(html);
  if (root?.index !== undefined) {
    const insertAt = root.index + root[0].length;
    return `${html.slice(0, insertAt)}<head>${style}</head>${html.slice(insertAt)}`;
  }
  return `${style}${html}`;
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
  const { kind, mimeType } = resolveArtifactPreview(item);
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
      if (kind === "text" || kind === "html") {
        const text = await response.blob.text();
        if (kind === "html") {
          objectUrl = URL.createObjectURL(new Blob([themedHtml(text)], { type: mimeType }));
          setContent({ kind, url: objectUrl });
        } else {
          setContent({ kind, text });
        }
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
      <div className="min-h-0 flex-1 overflow-auto rounded-md bg-[var(--color-surface)] text-[var(--color-on-surface)]">
        {loading ? <p role="status" className="p-4">{t("chat.artifactPreview.loading")}</p> : null}
        {error ? <div className="space-y-3 p-4" role="alert">
          <p>{error}</p>
          <Button variant="outline" onClick={retry}>{t("chat.artifactPreview.retry")}</Button>
        </div> : null}
        {!loading && !error && kind === "unsupported" ? <p className="p-4">{t("chat.artifactPreview.unsupported")}</p> : null}
        {!loading && !error && content?.kind === "image" && content.url
          ? <div className="flex h-full items-center justify-center bg-[var(--color-surface)] p-3"><img src={content.url} alt={item.display} className="max-h-full max-w-full object-contain" /></div>
          : null}
        {!loading && !error && content?.kind === "pdf" && content.url
          ? <iframe title={item.display} src={content.url} className="h-full min-h-96 w-full border-0 bg-[var(--color-surface)]" />
          : null}
        {!loading && !error && content?.kind === "html" && content.url
          ? <iframe title={item.display} src={content.url} sandbox="allow-scripts" referrerPolicy="no-referrer" className="h-full min-h-96 w-full border-0 bg-[var(--color-surface)]" />
          : null}
        {!loading && !error && content?.kind === "text"
          ? <pre className="h-full overflow-auto whitespace-pre-wrap break-words p-4 font-mono text-sm">{content.text}</pre>
          : null}
      </div>
    </DialogContent>
  </Dialog>;
}
