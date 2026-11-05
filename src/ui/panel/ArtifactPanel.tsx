import { useEffect, useMemo, useState } from "react";
import type { SentFile } from "../../core/bindings/SentFile";
import { sentFileUrl, sentPreview } from "../../core/client";
import { t } from "../../i18n";
import { CodeBlock } from "../CodeBlock";
import { Markdown } from "../Markdown";
import { SentDownload } from "../SentFile";
import { parseCsv } from "../sentCsv";
import { Panel } from "./Panel";

export function ArtifactPanel({ file, root }: { file: SentFile; root: string }) {
  const [code, setCode] = useState(false);
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const mime = file.mime;
  const office = /\.(docx|xlsx|pptx)$/i.test(file.name);
  const isText = office || mime.startsWith("text/") || mime === "application/json";
  useEffect(() => {
    let active = true;
    setText(null); setError(null); setCode(false);
    if (isText) void sentPreview(root, file.id).then((text) => { if (active) setText(text); }, (error: unknown) => { if (active) setError(String(error)); });
    return () => { active = false; };
  }, [root, file.id, isText]);
  const url = sentFileUrl(root, file.id);
  const tabs = !office && isText;
  return <Panel>
    <div className="artifact-toolbar">
      {tabs && <div role="tablist" aria-label={file.title}>
        <button className="btn btn-small" role="tab" aria-selected={!code} onClick={() => setCode(false)}>{t("sent.preview")}</button>
        <button className="btn btn-small" role="tab" aria-selected={code} onClick={() => setCode(true)}>{t("file.view_code")}</button>
      </div>}
      <SentDownload root={root} file={file} />
    </div>
    <div className="artifact-body">
      {office && <p className="sent-caption">{t("sent.office_text")}</p>}
      {error ? <p role="alert" className="sent-error">{error}</p> : isText && text === null ? <p>{t("viewer.loading")}</p> :
        text !== null ? code ? <CodeBlock text={text} info={file.name.split(".").pop()} /> : <TextPreview mime={mime} text={text} /> :
        mime === "application/pdf" ? <iframe className="artifact-frame" title={file.title} src={url} /> :
        mime.startsWith("image/") ? <img className="artifact-image" src={url} alt={file.title} /> :
        mime.startsWith("audio/") ? <audio controls src={url} /> :
        mime.startsWith("video/") ? <video controls className="artifact-image" src={url} /> : <p>{t("sent.unavailable")}</p>}
    </div>
  </Panel>;
}

function TextPreview({ mime, text }: { mime: string; text: string }) {
  if (mime === "text/html") {
    // An opaque origin blocks parent access; CSP blocks external subresources.
    const policy = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; media-src data: blob:; font-src data:; connect-src 'none'";
    return <iframe className="artifact-frame" title={t("sent.preview")} sandbox="allow-scripts" srcDoc={`<!doctype html><meta http-equiv="Content-Security-Policy" content="${policy}">${text}`} />;
  }
  if (mime === "text/csv") return <CsvPreview text={text} />;
  if (mime === "text/markdown") return <Markdown text={text} />;
  if (mime === "application/json") {
    let pretty = text;
    try { pretty = JSON.stringify(JSON.parse(text), null, 2); } catch { /* Invalid JSON remains inspectable as source. */ }
    return <CodeBlock text={pretty} info="json" />;
  }
  return <pre className="artifact-text">{text}</pre>;
}

function CsvPreview({ text }: { text: string }) {
  const [sort, setSort] = useState<{ column: number; descending: boolean } | null>(null);
  const [page, setPage] = useState(0);
  const parsed = useMemo(() => {
    try { return { rows: parseCsv(text), error: null }; }
    catch (error) { return { rows: [], error: String(error) }; }
  }, [text]);
  const header = parsed.rows[0] ?? [];
  const rows = useMemo(() => {
    const rows = parsed.rows.slice(1);
    if (sort) rows.sort((a, b) => (a[sort.column] ?? "").localeCompare(b[sort.column] ?? "", undefined, { numeric: true }) * (sort.descending ? -1 : 1));
    return rows;
  }, [parsed, sort]);
  if (parsed.error) return <p role="alert" className="sent-error">{parsed.error}</p>;
  return <>
    <div className="artifact-table"><table><thead><tr>{header.map((cell, column) => <th key={column} aria-sort={sort?.column === column ? sort.descending ? "descending" : "ascending" : "none"}>
      <button onClick={() => { setSort({ column, descending: sort?.column === column && !sort.descending }); setPage(0); }}>{cell}{sort?.column === column ? sort.descending ? " ↓" : " ↑" : ""}</button>
    </th>)}</tr></thead><tbody>{rows.slice(page * 100, (page + 1) * 100).map((row, i) => <tr key={i}>{Array.from({ length: Math.max(header.length, row.length) }, (_, col) => <td key={col}>{row[col] ?? ""}</td>)}</tr>)}</tbody></table></div>
    <div className="artifact-toolbar">
      <button className="btn btn-small" disabled={page === 0} onClick={() => setPage(page - 1)}>{t("viewer.prev")}</button>
      <span>{page + 1} / {Math.max(1, Math.ceil(rows.length / 100))}</span>
      <button className="btn btn-small" disabled={(page + 1) * 100 >= rows.length} onClick={() => setPage(page + 1)}>{t("viewer.next")}</button>
    </div>
  </>;
}
