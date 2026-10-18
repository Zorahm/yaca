/**
 * The picture the model looked at, in an opened `ReadImage` row.
 *
 * Not an attachment tile (`Attachment`): those are one size and stand in a row,
 * whereas here there is one picture, shown so that it can be seen — what the
 * model read, the person must be able to check without opening the file. A click
 * opens the same viewer as the attachments.
 */

import { t } from "../i18n";
import { baseName } from "./format";
import { useLook } from "./look";
import { useOpenImage } from "./viewer/context";

export function Thumb({ path }: { path: string }) {
  const look = useLook(path);
  const open = useOpenImage();

  // The file is gone or is not a picture — the row stays with its own text.
  if (look === undefined || look.kind !== "image" || look.preview === null) return null;
  return (
    <button
      className="thumb"
      title={path}
      aria-label={t("viewer.open", { name: baseName(path) })}
      onClick={() => open(path)}
    >
      <img src={look.preview} alt="" />
    </button>
  );
}
