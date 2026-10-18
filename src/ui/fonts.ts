/**
 * Fonts §2: Onest for the interface, JetBrains Mono for monospace.
 *
 * In a file of their own, not as a list of imports at each entry point: there
 * are two windows (§10), and a second one that forgot half of the faces would
 * be drawn in the system font — of the same density, but the wrong one.
 *
 * The faces are exactly those §2 names: 400/500/600/700 for the text font and
 * 400/500/600 for the monospace. There is no display font. An extra face is an
 * extra file in every load.
 */

import "@fontsource/onest/400.css";
import "@fontsource/onest/500.css";
import "@fontsource/onest/600.css";
import "@fontsource/onest/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/600.css";
