/**
 * The Yaca mark: a square holding a mirrored `C` (§9.1).
 *
 * Drawn rather than shipped as an image: it takes its colours from tokens at
 * paint time, so it follows the theme, and it stays sharp at whatever size the
 * screen hands it.
 *
 * The filled square is the one solid ink fill the home screen gets — the mark
 * is a знак, and the system spends the accent on знак, one button, and the
 * active label.
 *
 * Hidden from assistive technology on purpose: the greeting next to it already
 * says what this screen is, and a second voice saying "C" adds nothing.
 */

const BOX = 44;

export function Mark({ size = BOX }: { size?: number }) {
  return (
    <svg
      className="mark"
      width={size}
      height={size}
      viewBox={`0 0 ${BOX} ${BOX}`}
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <rect
        x="0.5"
        y="0.5"
        width={BOX - 1}
        height={BOX - 1}
        rx="9.5"
        fill="var(--ink)"
      />
      <text
        // Mirrored by the transform, not by a mirrored glyph: the font stays
        // the interface font, and the reflection survives a font change.
        transform={`translate(${BOX} 0) scale(-1 1)`}
        x={BOX / 2}
        y={BOX / 2}
        textAnchor="middle"
        dominantBaseline="central"
        fontFamily="var(--display)"
        fontSize="22"
        fontWeight="700"
        fill="var(--paper)"
      >
        C
      </text>
    </svg>
  );
}
