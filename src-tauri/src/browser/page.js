// Helpers the browser runs inside a page, in an isolated world of its own:
// the page's DOM is shared, its globals are not, so a page cannot read or
// break what is here, and nothing here shows up among the page's variables.
//
// Elements are named for the model by refs (`e12`) handed out by `snapshot`
// and kept for as long as the document lives. A new document starts over.
(() => {
  if (globalThis.__yaca) return;

  const refs = new Map();
  const ids = new WeakMap();
  let next = 1;

  const refOf = (el) => {
    let ref = ids.get(el);
    if (!ref) {
      ref = "e" + next++;
      ids.set(el, ref);
      refs.set(ref, new WeakRef(el));
    }
    return ref;
  };

  const clip = (text, max) => {
    const flat = (text || "").replace(/\s+/g, " ").trim();
    return flat.length > max ? flat.slice(0, max - 1) + "…" : flat;
  };

  const visible = (el) => {
    const style = getComputedStyle(el);
    if (style.display === "none" || style.visibility === "hidden") return false;
    const rect = el.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };

  const INPUT_ROLES = {
    button: "button", submit: "button", reset: "button", image: "button",
    checkbox: "checkbox", radio: "radio", range: "slider", file: "button",
    search: "searchbox", number: "spinbutton", color: "button",
  };

  const role = (el) => {
    const explicit = el.getAttribute("role");
    if (explicit) return explicit.split(/\s+/)[0];
    const tag = el.tagName.toLowerCase();
    switch (tag) {
      case "a": return el.hasAttribute("href") ? "link" : null;
      case "button": case "summary": return "button";
      case "select": return el.multiple ? "listbox" : "combobox";
      case "textarea": return "textbox";
      case "img": return el.getAttribute("alt") ? "img" : null;
      case "h1": case "h2": case "h3": case "h4": case "h5": case "h6": return "heading";
      case "input": {
        const type = (el.getAttribute("type") || "text").toLowerCase();
        if (type === "hidden") return null;
        return INPUT_ROLES[type] || "textbox";
      }
      case "nav": return "navigation";
      case "main": return "main";
      case "dialog": return "dialog";
      case "option": return "option";
    }
    if (el.isContentEditable && !el.parentElement?.isContentEditable) return "textbox";
    return null;
  };

  const INTERACTIVE = new Set([
    "button", "link", "checkbox", "radio", "slider", "textbox", "searchbox", "spinbutton",
    "combobox", "listbox", "option", "tab", "menuitem", "menuitemcheckbox", "menuitemradio",
    "switch", "treeitem",
  ]);

  const byIds = (list) =>
    list.split(/\s+/).map((id) => document.getElementById(id)?.innerText || "").join(" ");

  const nameOf = (el) => {
    const aria = el.getAttribute("aria-label");
    if (aria) return clip(aria, 80);
    const labelledBy = el.getAttribute("aria-labelledby");
    if (labelledBy) return clip(byIds(labelledBy), 80);
    if (el.labels && el.labels.length) return clip(el.labels[0].innerText, 80);
    const alt = el.getAttribute("alt");
    if (alt) return clip(alt, 80);
    const tag = el.tagName.toLowerCase();
    if (tag === "input" && ["button", "submit", "reset"].includes(el.type)) return clip(el.value, 80);
    const text = clip(el.innerText, 80);
    if (text) return text;
    return clip(el.getAttribute("title") || el.getAttribute("placeholder") || "", 80);
  };

  const describe = (el) => {
    const r = role(el) || el.tagName.toLowerCase();
    const name = nameOf(el);
    return name ? `${r} "${name}"` : r;
  };

  const state = (el, r) => {
    const out = [];
    if (r === "heading") out.push("level=" + (el.getAttribute("aria-level") || el.tagName[1]));
    if (r === "checkbox" || r === "radio" || r === "switch") {
      const checked = el.checked ?? el.getAttribute("aria-checked") === "true";
      out.push(checked ? "checked" : "unchecked");
    }
    if (el.disabled || el.getAttribute("aria-disabled") === "true") out.push("disabled");
    if (el.getAttribute("aria-expanded")) out.push("expanded=" + el.getAttribute("aria-expanded"));
    if (["textbox", "searchbox", "spinbutton"].includes(r) && "value" in el) {
      out.push(`value="${clip(el.value, 60)}"`);
    }
    if (r === "combobox" && el.tagName === "SELECT") {
      out.push(`value="${clip(el.selectedOptions[0]?.text || "", 60)}"`);
    }
    if (r === "link") {
      const href = el.getAttribute("href") || "";
      if (href && !href.startsWith("javascript:")) out.push("href=" + clip(href, 100));
    }
    return out.length ? " " + out.join(" ") : "";
  };

  // Text that belongs to this element itself rather than to a child that will
  // be listed on its own.
  const ownText = (el) => {
    let text = "";
    for (const node of el.childNodes) {
      if (node.nodeType === Node.TEXT_NODE) text += node.textContent;
    }
    return clip(text, 160);
  };

  const SKIP = new Set(["SCRIPT", "STYLE", "NOSCRIPT", "TEMPLATE", "SVG", "HEAD"]);

  const root = (selector) => {
    if (!selector) return document.body || document.documentElement;
    const el = document.querySelector(selector);
    if (!el) throw new Error(`nothing matches the selector ${selector}`);
    return el;
  };

  const snapshot = ({ selector, limit }) => {
    const lines = [];
    let more = 0;
    const walk = (el, depth) => {
      if (SKIP.has(el.tagName.toUpperCase())) return;
      // Only `display: none` hides a whole subtree. A box of no size can
      // still hold positioned children that show, so it only goes unlisted.
      const style = getComputedStyle(el);
      if (style.display === "none") return;
      const r = visible(el) ? role(el) : null;
      let shown = false;
      if (r && (INTERACTIVE.has(r) || r === "heading" || r === "img")) {
        const ref = INTERACTIVE.has(r) ? ` [ref=${refOf(el)}]` : "";
        const line = `${"  ".repeat(depth)}- ${r} "${nameOf(el)}"${ref}${state(el, r)}`;
        if (lines.length < limit) lines.push(line); else more++;
        shown = true;
        // A control's own content is its name; listing it again is noise.
        if (INTERACTIVE.has(r) || r === "heading") return;
      } else if (r === "navigation" || r === "main" || r === "dialog") {
        if (lines.length < limit) lines.push(`${"  ".repeat(depth)}- ${r}:`); else more++;
        shown = true;
      } else if (style.visibility !== "hidden") {
        const text = ownText(el);
        if (text) {
          if (lines.length < limit) lines.push(`${"  ".repeat(depth)}- text "${text}"`); else more++;
        }
      }
      for (const child of el.children) walk(child, shown ? depth + 1 : depth);
      if (el.shadowRoot) for (const child of el.shadowRoot.children) walk(child, depth);
    };
    walk(root(selector), 0);
    return { text: lines.join("\n"), more };
  };

  const html = ({ selector }) => {
    const copy = root(selector).cloneNode(true);
    copy.querySelectorAll("script,style,noscript,template,svg,iframe,canvas").forEach((el) => el.remove());
    return copy.outerHTML;
  };

  const find = (target) => {
    if (target.ref) {
      const el = refs.get(target.ref)?.deref();
      if (!el || !el.isConnected) {
        throw new Error(`ref ${target.ref} is gone (the page changed): take a new read_page snapshot`);
      }
      return el;
    }
    if (target.selector) {
      const el = document.querySelector(target.selector);
      if (!el) throw new Error(`nothing matches the selector ${target.selector}`);
      return el;
    }
    throw new Error("name the element by ref or selector");
  };

  // The element brought into view, with its centre in viewport pixels.
  const point = (target) => {
    const el = find(target);
    el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
    const rect = el.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) throw new Error(`${describe(el)} is not visible`);
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    const top = document.elementFromPoint(x, y);
    const covered = top && top !== el && !el.contains(top) && !top.contains(el) ? describe(top) : null;
    return {
      x, y,
      rect: { x: rect.left, y: rect.top, width: rect.width, height: rect.height },
      label: describe(el),
      covered,
      disabled: !!(el.disabled || el.getAttribute("aria-disabled") === "true"),
    };
  };

  // Focus for typing; `clear` selects what is there so typing replaces it.
  const focus = (target, clear) => {
    const el = find(target);
    el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
    el.focus();
    if (clear) {
      if (typeof el.select === "function") {
        el.select();
      } else if (el.isContentEditable) {
        const range = document.createRange();
        range.selectNodeContents(el);
        const selection = getSelection();
        selection.removeAllRanges();
        selection.addRange(range);
      }
    }
    const editable = el.isContentEditable || ["INPUT", "TEXTAREA"].includes(el.tagName);
    const rect = el.getBoundingClientRect();
    return {
      label: describe(el),
      editable,
      rect: { x: rect.left, y: rect.top, width: rect.width, height: rect.height },
    };
  };

  const choose = (target, values) => {
    const el = find(target);
    if (el.tagName !== "SELECT") throw new Error(`${describe(el)} is not a <select>`);
    const wanted = Array.isArray(values) ? values : [values];
    const picked = [];
    for (const option of el.options) {
      const hit = wanted.includes(option.value) || wanted.includes(option.text.trim());
      if (el.multiple) option.selected = hit;
      else if (hit && !picked.length) option.selected = true;
      if (hit) picked.push(option.text.trim());
    }
    if (!picked.length) throw new Error(`no option of ${describe(el)} is ${wanted.join(", ")}`);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
    const rect = el.getBoundingClientRect();
    return {
      label: describe(el),
      picked,
      rect: { x: rect.left, y: rect.top, width: rect.width, height: rect.height },
    };
  };

  const present = ({ selector, text }) => {
    if (selector) {
      const el = document.querySelector(selector);
      return !!el && visible(el);
    }
    return (document.body?.innerText || "").includes(text);
  };

  // An element's box in document pixels, for a screenshot of it alone.
  const box = (target) => {
    const el = find(target);
    el.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "instant" });
    const rect = el.getBoundingClientRect();
    return {
      x: rect.left + scrollX, y: rect.top + scrollY,
      width: rect.width, height: rect.height,
      label: describe(el),
    };
  };

  globalThis.__yaca = { snapshot, html, point, focus, choose, present, box };
})();
