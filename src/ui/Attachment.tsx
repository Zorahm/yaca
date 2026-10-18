/**
 * Attachment tile (§7.2, §5.1) — one for everything that is attached: a picture,
 * a file, a folder.
 *
 * The tile has one size, whatever the content. A row of pictures of different
 * widths reads as a collage, not as a list of what was attached: the eye catches
 * on the widest, and a narrow screenshot next to it gets lost.
 *
 * A picture fills the tile and opens in the viewer (`viewer/`). A file or a
 * folder has an icon and a name; the full path is in the hint. The tile does not
 * hide what is already gone from disk: the attachment was there, and the message
 * card (§5.1) speaks of what was attached, not of what lies there now.
 *
 * One component for the composer and the card: it is the same attachment before
 * and after sending. There is one difference — `onRemove`: only what has not been
 * sent yet can be removed.
 */

import { FileDashed, FileText, Folder, X } from "@phosphor-icons/react";

import { t } from "../i18n";
import { baseName } from "./format";
import { useLook } from "./look";
import { useOpenImage } from "./viewer/context";

export function Attachment({ path, onRemove }: { path: string; onRemove?: () => void }) {
  const look = useLook(path);
  const open = useOpenImage();
  const name = baseName(path);
  const small = onRemove !== undefined;

  return (
    <span className={small ? "tile tile-small" : "tile"}>
      {look?.kind === "image" && look.preview !== null ? (
        <button
          className="tile-face tile-image"
          title={path}
          aria-label={t("viewer.open", { name })}
          onClick={() => open(path)}
        >
          <img src={look.preview} alt="" />
        </button>
      ) : (
        <span
          className={look?.kind === "missing" ? "tile-face tile-gone" : "tile-face"}
          title={look?.kind === "missing" ? t("attachment.gone", { path }) : path}
        >
          {/* While the answer is on its way the tile is already in place and
              already named: the row does not jump, and what it turns out to be
              shows a moment later. */}
          {look !== undefined && <Icon kind={look.kind} size={small ? 16 : 20} />}
          <span className="tile-name">{name}</span>
        </span>
      )}

      {onRemove && (
        <button
          className="tile-drop"
          aria-label={t("composer.detach")}
          title={t("composer.detach")}
          onClick={onRemove}
        >
          <X size={10} weight="bold" />
        </button>
      )}
    </span>
  );
}

function Icon({ kind, size }: { kind: string; size: number }) {
  switch (kind) {
    case "folder":
      return <Folder size={size} />;
    case "missing":
      return <FileDashed size={size} />;
    default:
      return <FileText size={size} />;
  }
}
