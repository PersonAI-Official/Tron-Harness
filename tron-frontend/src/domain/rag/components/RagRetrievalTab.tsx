import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { ShieldCheck, Search, ListFilter, FileSearch, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useToast } from "@/components/ui/toast";
import { InfoTooltip } from "@/components/info-tooltip";
import { Card, CardContent } from "@/components/ui/card";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { SelectInput } from "@/components/forms";
import { SelectItem } from "@/components/ui/select";
import { Table, TableHeader, TableHead, TableBody, TableCell, TableRow } from "@/components/ui/table";
import { searchRag } from "../api/ragApi";
import type { RagCorpus, RagSearchResult } from "../model/RagModel";
import { RagFlowSteps } from "./RagFlowSteps";
import { RagEvidenceContent } from "./RagEvidenceContent";

const retrievalSteps = [
  { stage: "authorize", icon: ShieldCheck },
  { stage: "recall", icon: Search },
  { stage: "fusion", icon: ListFilter },
  { stage: "evidence", icon: FileSearch },
];

export function RagRetrievalTab({ corpora, selected, onSelect }: { corpora: RagCorpus[]; selected: string; onSelect: (id: string) => void }) {
  const { t } = useTranslation("pages"); const navigate = useNavigate();
  const toast = useToast();
  const channelText = (channel: string) => t(`ragUnified.channels.${channel.replace(/\./g, "_")}`, { defaultValue: channel });
  const [query, setQuery] = useState(""); const [limitDraft, setLimitDraft] = useState("8"); const limit = limitDraft === "" ? NaN : Number(limitDraft);
  const [result, setResult] = useState<RagSearchResult | null>(null); const [pending, setPending] = useState(false); const [notices, setNotices] = useState<string[]>([]);
  const clearResult = () => setResult(null);
  const search = async () => {
    setPending(true); clearResult(); setNotices([]);
    try {
      const response = await searchRag({ query, corpusIds: selected ? [selected] : [], limit });
      setResult(response);
      const failures = response.stages.filter((stage) => stage.error && !response.warnings.some((warning) => warning.includes(stage.error!)))
        .map((stage) => `${channelText(stage.channel)}: ${stage.error}`);
      setNotices([...new Set([...response.warnings, ...failures])]);
    }
    catch (cause) { toast.error(cause instanceof Error ? cause.message : String(cause)); }
    finally { setPending(false); }
  };
  return <div className="grid gap-4">
    <div className="hidden lg:block"><RagFlowSteps label={t("ragUnified.retrievalFlow")} steps={retrievalSteps} /></div>
    <Card><CardContent className="grid gap-4 pt-6">
      <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_10rem]">
        <Field><FieldLabel>{t("ragUnified.corpus")}</FieldLabel><SelectInput aria-label={t("ragUnified.corpus")} disabled={pending} value={selected || "__all__"} onValueChange={(value) => { onSelect(value === "__all__" ? "" : String(value)); clearResult(); }}><SelectItem value="__all__">{t("ragUnified.allAuthorized")}</SelectItem>{corpora.map((item) => <SelectItem key={item.id} value={item.id}>{item.name}</SelectItem>)}</SelectInput></Field>
        <Field><FieldLabel htmlFor="rag-limit">{t("ragUnified.limit")}</FieldLabel><Input id="rag-limit" type="text" inputMode="numeric" value={limitDraft} onChange={(event) => { if (/^\d*$/.test(event.target.value)) setLimitDraft(event.target.value); }} /></Field>
      </div>
      <Field><FieldLabel htmlFor="rag-question">{t("ragUnified.question")}</FieldLabel><Textarea id="rag-question" value={query} onChange={(event) => setQuery(event.target.value)} /></Field>
      <div className="flex justify-end"><Button disabled={pending || !query.trim() || !Number.isInteger(limit) || limit < 1 || limit > 100} onClick={() => void search()}>{t(pending ? "ragUnified.searching" : "ragUnified.search")}</Button></div>
    </CardContent></Card>
    {result ? <>
      <Card><CardContent className="grid gap-3 pt-6"><div className="flex flex-wrap items-center gap-2"><strong>{t("ragUnified.authorizedRange")}</strong>{result.authorizedCorpusIds.map((id) => <Badge key={id} variant="secondary">{corpora.find((item) => item.id === id)?.name ?? id}</Badge>)}<span>{result.durationMs} ms</span>{result.stages.some((stage) => stage.channel.startsWith("graph") && stage.hitCount > 0 && !["FAILED", "INVALIDATED"].includes(stage.status)) ? <Button variant="outline" onClick={() => navigate(`/knowledge-graph?${new URLSearchParams({ mode: "rag", query: result.query, ...(selected ? { corpus: selected } : {}) }).toString()}`)}>{t("knowledgeGraph.ragOpen")}</Button> : null}</div>
        {notices.length ? <div role="status" className="flex items-start gap-2 rounded-md border border-warning/40 bg-warning/8 px-3 py-2 text-sm"><TriangleAlert aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-warning" /><div className="grid gap-1">{notices.map((notice) => <p key={notice}>{notice}</p>)}</div></div> : null}
        <Table><TableHeader><TableRow>{["corpus", "channel", "state", "hits", "duration"].map((key) => <TableHead key={key}>{t(`ragUnified.${key}`)}</TableHead>)}</TableRow></TableHeader><TableBody>{result.stages.map((stage, i) => <TableRow key={i}><TableCell>{corpora.find((item) => item.id === stage.corpusId)?.name ?? stage.corpusId}</TableCell><TableCell>{channelText(stage.channel)}</TableCell><TableCell><span className="inline-flex items-center gap-1.5">{t(`ragUnified.states.${stage.status}`)}{stage.error ? <InfoTooltip ariaLabel={t("ragUnified.stageFailureHelp", { channel: channelText(stage.channel) })}>{stage.error}</InfoTooltip> : null}</span></TableCell><TableCell>{stage.hitCount}</TableCell><TableCell>{stage.durationMs} ms</TableCell></TableRow>)}</TableBody></Table>
      </CardContent></Card>
      {result.evidence.map((hit) => <Card key={`${hit.corpusId}:${hit.chunkCode}`}><CardContent className="grid gap-3 pt-6"><div className="flex flex-wrap items-center justify-between gap-3"><h3 className="font-medium">{hit.title ?? hit.sourceCode}</h3><span className="text-muted-foreground">{hit.score.toFixed(4)}</span></div><div className="flex gap-2">{hit.channels.map((channel) => <Badge key={channel} variant="secondary">{channelText(channel)}</Badge>)}<Badge variant="outline">{hit.sourceVersion}</Badge></div><RagEvidenceContent hit={hit} libraryId={corpora.find((item) => item.id === hit.corpusId)?.knowledgeBaseId ?? undefined} />{corpora.find((item) => item.id === hit.corpusId)?.knowledgeBaseId ? <Button variant="outline" className="justify-self-start" onClick={() => navigate(`/settings/knowledge-base/${encodeURIComponent(hit.corpusId)}?documentId=${encodeURIComponent(String(hit.source.documentId ?? ""))}`)}>{t("ragUnified.viewSource")}</Button> : null}</CardContent></Card>)}
    </> : null}
  </div>;
}
