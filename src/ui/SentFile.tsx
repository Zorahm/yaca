import { DownloadSimple, File, ArrowSquareOut } from "@phosphor-icons/react";
import { useContext, useState } from "react";
import type { SentFile as FileDelivery } from "../core/bindings/SentFile";
import { downloadSentFile, sentFileUrl } from "../core/client";
import { t } from "../i18n";
import { DeliveryRoot, useOpenImage } from "./viewer/context";
import { sentImagePath } from "../core/sentImage";
import { bytes } from "./format";
import "./sent.css";

export function SentDownload({ file, root }: { file: FileDelivery; root: string }) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  return <>
    <button className="btn btn-small" disabled={busy} title={t("sent.download")} onClick={() => {
      setBusy(true); setError(null);
      void downloadSentFile(root, file.id, file.name).catch((error: unknown) => setError(String(error))).finally(() => setBusy(false));
    }}><DownloadSimple size={16} />{t("sent.download")}</button>
    {error && <span className="sent-error" role="alert">{error}</span>}
  </>;
}

export function SentFile({ file, onOpen }: { file: FileDelivery; onOpen: (file: FileDelivery) => void }) {
  const root = useContext(DeliveryRoot);
  const openImage = useOpenImage();
  const [failed, setFailed] = useState(false);
  if (root === null) return <p role="alert">{t("file.unavailable")}</p>;
  const url = sentFileUrl(root, file.id);
  const image = file.mime.startsWith("image/");
  const audio = file.mime.startsWith("audio/");
  const video = file.mime.startsWith("video/");
  const inline = file.renderType === "inline";
  const format = /.+\.([a-z\d]{1,8})$/i.exec(file.name)?.[1]?.toUpperCase();
  return <article className="sent-card" aria-label={file.title}>
    <div className="sent-header">
      <span className="sent-format">
        {format ?? <File size={18} aria-hidden="true" />}
      </span>
      <div className="sent-details">
        <strong className="sent-title">{file.title}</strong>
        {file.caption && <p className="sent-caption">{file.caption}</p>}
        <span className="sent-name" title={file.name}>{file.name}</span>
      </div>
    </div>
    {inline && image && !failed && <button className="sent-picture" onClick={() => openImage(sentImagePath(file))} aria-label={t("viewer.open", { name: file.title })}>
      <img src={url} alt={file.title} onError={() => setFailed(true)} />
    </button>}
    {inline && audio && <audio controls preload="metadata" src={url} aria-label={file.title} onError={() => setFailed(true)} />}
    {inline && video && <video controls preload="metadata" src={url} aria-label={file.title} onError={() => setFailed(true)} />}
    {file.renderType === "attachment" && <code className="sent-hash">SHA-256 {file.sha256}</code>}
    {failed && <p className="sent-error" role="alert">{t("sent.unavailable")}</p>}
    <div className="sent-footer">
      <span className="sent-size">{bytes(file.size)}</span>
      <div className="sent-actions">
        {file.renderType === "artifact" && <button className="btn btn-small" onClick={() => onOpen(file)}><ArrowSquareOut size={16} />{t("sent.preview")}</button>}
        <SentDownload root={root} file={file} />
      </div>
    </div>
  </article>;
}
