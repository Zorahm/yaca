import { useLayoutEffect, useRef, useSyncExternalStore } from "react";

import { YacaGlyph } from "./Logo";
import "./yak-mascot.css";

const motionQuery = "(prefers-reduced-motion: reduce)";
/** Typing is answered now and then: a reaction, then quiet for this long. */
const reactionGapMs = 3000;
// The reaction waits for `beginElement()`; `fill="remove"` lets the glyph settle
// back to rest when it ends.
const reactionTiming = {
  begin: "indefinite",
  fill: "remove",
  "data-yak-reaction": "",
} as const;

function subscribeToMotion(onChange: () => void) {
  const query = window.matchMedia(motionQuery);
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

function readReducedMotion() {
  return window.matchMedia(motionQuery).matches;
}

/**
 * The mark on the home screen. It has no controls: now and then, while the
 * person types, it reacts with its own motions, and the core glances left and
 * right of the stem. Under reduced motion it stays still.
 */
export function YakMascot({ keystrokes }: { keystrokes: number }) {
  const svg = useRef<SVGSVGElement>(null);
  const lastReaction = useRef(Number.NEGATIVE_INFINITY);
  const reducedMotion = useSyncExternalStore(subscribeToMotion, readReducedMotion, () => true);

  useLayoutEffect(() => {
    if (keystrokes === 0 || reducedMotion) return;
    const now = performance.now();
    if (now - lastReaction.current < reactionGapMs) return;
    lastReaction.current = now;
    // begin="indefinite" with beginElement() rather than begin="0s": the SVG's
    // timeline has been running since the home screen opened, so 0s would
    // already be in the past.
    svg.current?.querySelectorAll<SVGAnimationElement>("[data-yak-reaction]")
      .forEach((animation) => animation.beginElement());
  }, [keystrokes, reducedMotion]);

  // The core keeps its place and size; a reaction makes it glance left and right of the stem.
  const coreAnimation = !reducedMotion && <>
    <animateTransform
      {...reactionTiming} dur="1.2s" attributeName="transform" type="translate"
      values="0 0;-2 0;2 0;0 0" keyTimes="0;0.3;0.7;1" calcMode="spline"
      keySplines="0.05 0.7 0.1 1;0.4 0 0.2 1;0.05 0.7 0.1 1"
    />
  </>;

  return (
    <div className="yak-mascot">
      <svg ref={svg} viewBox="-16 -16 80 80" fill="currentColor" aria-hidden="true" focusable="false">
        <g transform="translate(24 24)">
          <g>
            {!reducedMotion && (
              <animateTransform
                {...reactionTiming} dur="0.7s" attributeName="transform" type="scale"
                values="1;1.06;0.98;1" keyTimes="0;0.35;0.65;1" calcMode="spline"
                keySplines="0.4 0 0.2 1;0.4 0 0.2 1;0.4 0 0.2 1"
              />
            )}
            <g transform="translate(-24 -24)">
              <YacaGlyph
                hornsAnimation={!reducedMotion && (
                  <animateTransform
                    {...reactionTiming} dur="0.7s" attributeName="transform" type="scale"
                    values="1 1;1.1 0.95;1 1" keyTimes="0;0.4;1" calcMode="spline"
                    keySplines="0.4 0 0.2 1;0.4 0 0.2 1"
                  />
                )}
                coreAnimation={coreAnimation}
              />
            </g>
          </g>
        </g>
      </svg>
    </div>
  );
}
