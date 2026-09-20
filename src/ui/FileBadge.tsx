/**
 * Бейдж состояния файла §6.2.
 *
 * Состояний осталось четыре: правки уезжают в дерево сразу, и «принят» с
 * «отклонён» вместе с конфликтом применения исчезли вместе с кнопкой (§16.37).
 * Бейдж говорит о том, что случилось с файлом, а не о решении по нему.
 */

import type { FileFact } from "../core/client";
import { t } from "../i18n";

export function FileBadge({ file }: { file: FileFact }) {
  switch (file.kind) {
    case "created":
      return <span className="badge badge-created">{t("file.created")}</span>;
    case "deleted":
      return <span className="badge badge-rejected">{t("file.deleted")}</span>;
    case "moved":
      return <span className="badge badge-modified">{t("file.moved")}</span>;
    default:
      return <span className="badge badge-modified">{t("file.modified")}</span>;
  }
}
