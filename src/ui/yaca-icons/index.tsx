/**
 * Yaca's own icons: the ones Phosphor does not have, drawn to stand next to
 * it.
 *
 * They follow Phosphor's regular weight so that a row of buttons reads as one
 * set: a 16-unit box, a one-unit stroke with round joins, `currentColor`.
 * An icon goes here only when it has to say something no stock glyph says —
 * otherwise it is Phosphor, and there is one stroke in the whole window.
 *
 * Every icon is decorative: the button around it carries the name.
 */

import type { ReactNode } from "react";

function Icon({
  size,
  mirrored = false,
  children,
}: {
  size: number;
  /** Drawn left to right and flipped: one drawing for both sides of the window. */
  mirrored?: boolean;
  children: ReactNode;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {mirrored ? <g transform="translate(16 0) scale(-1 1)">{children}</g> : children}
    </svg>
  );
}

/**
 * The window with a column at one side: the column is filled while it is
 * shown and an outline while it is hidden, so the icon is the state and not
 * only the way to change it. Drawn with the column on the right.
 */
function SideColumn({
  size,
  open,
  mirrored,
}: {
  size: number;
  open: boolean;
  mirrored: boolean;
}) {
  return (
    <Icon size={size} mirrored={mirrored}>
      <rect x="1.5" y="2.5" width="13" height="11" rx="2" />
      {open ? (
        <path d="M10 2.5h2.5a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H10z" fill="currentColor" />
      ) : (
        <path d="M10 2.5v11" />
      )}
    </Icon>
  );
}

/** The right panel, shown or hidden. */
export function PanelRight({ size, open }: { size: number; open: boolean }) {
  return <SideColumn size={size} open={open} mirrored={false} />;
}

/** The sidebar on the left, shown or hidden. */
export function PanelLeft({ size, open }: { size: number; open: boolean }) {
  return <SideColumn size={size} open={open} mirrored />;
}
