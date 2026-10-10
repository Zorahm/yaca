/**
 * Плитка вложения (§7.2, §5.1) — одна на всё, что прикладывают: картинку,
 * файл, папку.
 *
 * Размер у плитки один, каким бы ни было содержимое. Ряд из картинок разной
 * ширины читается как коллаж, а не как список приложенного: взгляд цепляется
 * за самую широкую, и узкий скриншот рядом с ней теряется.
 *
 * Картинка занимает плитку целиком и открывается в просмотрщике (`viewer/`).
 * У файла и папки — значок и имя; путь целиком — в подсказке. Того, чего на
 * диске уже нет, плитка не прячет: вложение было, и карточка сообщения §5.1
 * говорит о том, что было приложено, а не о том, что лежит там сейчас.
 *
 * Один компонент на композер и на карточку: это одно и то же вложение до
 * отправки и после. Разница одна — `onRemove`: убрать можно только то, что
 * ещё не отправлено.
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
          {/* Пока ответ едет, плитка уже на месте и уже с именем: ряд не
              прыгает, а чем она окажется, видно через мгновение. */}
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
