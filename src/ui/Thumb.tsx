/**
 * Картинка, на которую посмотрела модель, — в раскрытой строке `ReadImage`.
 *
 * Не плитка вложения (`Attachment`): та одного размера и стоит в ряду, а
 * здесь картинка одна и показана так, чтобы её было видно, — то, что прочла
 * модель, человек должен мочь проверить, не открывая файл. Щелчок открывает
 * тот же просмотрщик, что и вложения.
 */

import { t } from "../i18n";
import { baseName } from "./format";
import { useLook } from "./look";
import { useOpenImage } from "./viewer/context";

export function Thumb({ path }: { path: string }) {
  const look = useLook(path);
  const open = useOpenImage();

  // Файла уже нет или это не картинка — строка остаётся при своём тексте.
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
