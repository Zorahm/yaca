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
  return <div className="sent-file">
    {inline && image && !failed && <button className="sent-picture" onClick={() => openImage(sentImagePath(file))} aria-label={t("viewer.open", { name: file.title })}>
      <img src={url} alt={file.title} onError={() => setFailed(true)} />
    </button>}
    {inline && audio && <audio controls preload="metadata" src={url} aria-label={file.title} onError={() => setFailed(true)} />}
    {inline && video && <video controls preload="metadata" src={url} aria-label={file.title} onError={() => setFailed(true)} />}
    <div className="sent-card">
      <File size={22} aria-hidden="true" />
      <div className="sent-details">
        <strong>{file.title}</strong>
        <span>{file.name} · {bytes(file.size)}</span>
      </div>
      {file.renderType === "artifact" && <button className="btn btn-small" onClick={() => onOpen(file)}><ArrowSquareOut size={16} />{t("sent.preview")}</button>}
      <SentDownload root={root} file={file} />
    </div>
    {file.renderType === "attachment" && <code className="sent-hash">SHA-256 {file.sha256}</code>}
    {file.caption && <p className="sent-caption">{file.caption}</p>}
    {failed && <p className="sent-error" role="alert">{t("sent.unavailable")}</p>}
  </div>;
}
