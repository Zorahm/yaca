(function () {
  "use strict";

  const dict = window.YACA_I18N;
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));

  function store(key, value) {
    try {
      if (value === undefined) return localStorage.getItem(key);
      localStorage.setItem(key, value);
    } catch (e) {
      return null;
    }
  }

  // ---------- i18n ----------

  let lang = store("yaca-lang") || ((navigator.language || "ru").toLowerCase().startsWith("ru") ? "ru" : "en");
  if (!dict[lang]) lang = "ru";
  const t = (key) => dict[lang][key] ?? dict.ru[key] ?? key;
  const listeners = [];

  function applyI18n() {
    document.documentElement.lang = lang;
    $$("[data-i18n]").forEach((el) => { el.textContent = t(el.dataset.i18n); });
    $$("[data-i18n-attr]").forEach((el) => {
      el.dataset.i18nAttr.split(";").forEach((pair) => {
        const [attr, key] = pair.split(":");
        el.setAttribute(attr, t(key));
      });
    });
    $("#lang-code").textContent = lang === "ru" ? "EN" : "RU";
    listeners.forEach((fn) => fn());
  }

  $("#lang-toggle").addEventListener("click", () => {
    lang = lang === "ru" ? "en" : "ru";
    store("yaca-lang", lang);
    applyI18n();
    replayTitle();
  });

  // ---------- theme ----------

  $("#theme-toggle").addEventListener("click", () => {
    const root = document.documentElement;
    const isDark = root.dataset.theme
      ? root.dataset.theme === "dark"
      : window.matchMedia("(prefers-color-scheme: dark)").matches;
    root.dataset.theme = isDark ? "light" : "dark";
    store("yaca-theme", root.dataset.theme);
    syncThemeButton();
  });

  function syncThemeButton() {
    const root = document.documentElement;
    const isDark = root.dataset.theme
      ? root.dataset.theme === "dark"
      : window.matchMedia("(prefers-color-scheme: dark)").matches;
    $("#theme-toggle").setAttribute("aria-pressed", String(isDark));
  }
  syncThemeButton();

  // ---------- scroll: progress line, nav border, reveal ----------

  const progress = $(".progress span");
  const nav = $(".nav");
  function onScroll() {
    const max = document.documentElement.scrollHeight - innerHeight;
    progress.style.transform = `scaleX(${max > 0 ? scrollY / max : 0})`;
    nav.classList.toggle("scrolled", scrollY > 8);
  }
  addEventListener("scroll", onScroll, { passive: true });
  onScroll();

  // Stagger siblings that reveal together.
  $$(".principle-list, .feature-grid").forEach((group) => {
    $$(".reveal", group).forEach((el, i) => el.style.setProperty("--d", `${i * 0.07}s`));
  });
  $$(".hero-copy .reveal").forEach((el, i) => el.style.setProperty("--d", `${i * 0.08}s`));

  const revealer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (!entry.isIntersecting) return;
      entry.target.classList.add("in");
      revealer.unobserve(entry.target);
    });
  }, { rootMargin: "0px 0px -8% 0px", threshold: 0.08 });
  $$(".reveal").forEach((el) => revealer.observe(el));

  // ---------- hero title: the red pencil correction ----------

  const title = $(".hero-title");
  function replayTitle() {
    title.classList.remove("play");
    void title.offsetWidth;
    title.classList.add("play");
  }
  replayTitle();

  // ---------- demo session ----------

  const feed = $("#demo-feed");
  const ring = $("#status-ring");
  let runId = 0;

  class Cancelled extends Error {}

  function el(tag, cls, html) {
    const node = document.createElement(tag);
    if (cls) node.className = cls;
    if (html !== undefined) node.innerHTML = html;
    return node;
  }

  function scrollFeed() {
    feed.scrollTo({ top: feed.scrollHeight, behavior: reducedMotion ? "auto" : "smooth" });
  }

  function append(node) {
    feed.appendChild(node);
    scrollFeed();
    return node;
  }

  const escapeHtml = (s) => s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]);

  async function playDemo() {
    const id = ++runId;
    const fast = reducedMotion;
    const wait = (ms) => new Promise((resolve, reject) => {
      setTimeout(() => (id === runId ? resolve() : reject(new Cancelled())), fast ? 0 : ms);
    });

    async function type(node, text, speed = 18) {
      const cursor = el("span", "cursor");
      node.textContent = "";
      node.appendChild(cursor);
      for (let i = 0; i < text.length; i++) {
        cursor.before(text[i]);
        // Keep a line that wraps while typing in view above the composer.
        if (feed.scrollHeight - feed.scrollTop > feed.clientHeight) feed.scrollTop = feed.scrollHeight;
        if (!fast) await wait(speed + Math.random() * speed);
      }
      cursor.remove();
    }

    function tool(group, verb, arg) {
      const row = el("div", "f-tool", `<span class="t-icon running"></span><span>${escapeHtml(verb)}</span><code>${escapeHtml(arg)}</code>`);
      group.appendChild(row);
      scrollFeed();
      return {
        done: (state = "done") => { row.querySelector(".t-icon").className = `t-icon ${state}`; },
      };
    }

    feed.innerHTML = "";
    ring.className = "status-ring";

    try {
      await wait(500);
      append(el("div", "f-human", escapeHtml(t("demo.human"))));
      ring.className = "status-ring busy";
      await wait(700);

      await type(append(el("div", "f-agent")), t("demo.agent.1"));
      await wait(300);

      const group = append(el("div", "f-tools"));
      let r = tool(group, t("demo.tool.read"), "src/cart.rs");
      await wait(650); r.done();
      r = tool(group, t("demo.tool.grep"), "total_counts_quantity");
      await wait(600); r.done();
      r = tool(group, t("demo.tool.bash"), "cargo test cart");
      await wait(1000); r.done("failed");

      append(el("div", "f-output",
        `running 12 tests\n<span class="bad">${escapeHtml(t("demo.fail"))}</span>\n  left: 1200, right: 3600`));
      await wait(1100);

      const diff = append(el("div", "f-diff",
        `<div class="f-diff-head"><b>src/cart.rs</b><span><span class="stat-add">+1</span> <span class="stat-del">−1</span></span></div>`));
      const lines = [
        ["", "41", '<span class="kw">pub fn</span> total(items: &amp;[Item]) -&gt; <span class="kw">u64</span> {'],
        ["del", "42", "    items.iter().map(|i| i.price).sum()"],
        ["add", "42", "    items.iter().map(|i| i.price * i.qty).sum()"],
        ["", "43", "}"],
      ];
      for (const [kind, ln, code] of lines) {
        diff.appendChild(el("div", `d-line ${kind}`,
          `<span class="ln">${ln}</span><span class="sg">${kind === "del" ? "−" : kind === "add" ? "+" : " "}</span><span class="tx">${code}</span>`));
        scrollFeed();
        await wait(260);
      }
      await wait(500);

      ring.className = "status-ring";
      const ask = append(el("div", "f-ask",
        `<div class="f-ask-q">${escapeHtml(t("demo.ask.q"))}</div><div class="f-ask-opts">` +
        [1, 2, 3].map((n) => `<div class="f-ask-opt"><kbd>${n}</kbd>${escapeHtml(t(`demo.ask.${n}`))}</div>`).join("") +
        "</div>"));
      await wait(1700);
      ask.querySelector(".f-ask-opt").classList.add("picked");
      await wait(600);
      ask.classList.add("resolved");
      ring.className = "status-ring busy";

      append(el("div", "f-checkpoint", `<span>${escapeHtml(t("demo.checkpoint"))} · <u>${escapeHtml(t("demo.rollback"))}</u></span>`));
      await wait(500);

      const group2 = append(el("div", "f-tools"));
      r = tool(group2, t("demo.tool.edit"), "src/cart.rs");
      await wait(450); r.done();
      r = tool(group2, t("demo.tool.bash"), "cargo test cart");
      await wait(1100); r.done();
      append(el("div", "f-output", `<span class="good">${escapeHtml(t("demo.ok"))}</span>`));
      await wait(600);

      await type(append(el("div", "f-agent")), t("demo.agent.2"));
      ring.className = "status-ring idle";

      // Under reduced motion the finished session simply stays on screen.
      if (fast) return;
      await wait(6000);
      playDemo();
    } catch (e) {
      if (!(e instanceof Cancelled)) throw e;
    }
  }

  $("#demo-replay").addEventListener("click", playDemo);

  // Play only while the demo is on screen and the tab is visible; resume from the start.
  let demoOnScreen = false;
  let demoStale = true;
  const demoCanPlay = () => demoOnScreen && document.visibilityState === "visible";
  function syncDemo() {
    if (demoCanPlay()) {
      if (demoStale) { demoStale = false; playDemo(); }
    } else if (!demoStale) {
      demoStale = true;
      runId++;
    }
  }
  new IntersectionObserver(([entry]) => { demoOnScreen = entry.isIntersecting; syncDemo(); }, { threshold: 0.15 })
    .observe($(".hero-demo"));
  document.addEventListener("visibilitychange", syncDemo);
  listeners.push(() => { if (demoCanPlay()) playDemo(); else demoStale = true; });

  // ---------- trust modes ----------

  // Mirrors the matrix in crates/yaca-core/src/permission/policy.rs; Plan is read-only.
  const modes = ["plan", "manual", "edits", "auto"];
  const matrix = {
    read:    { plan: "allow", manual: "allow", edits: "allow", auto: "allow" },
    edit:    { plan: "deny",  manual: "ask",   edits: "allow", auto: "allow" },
    delete:  { plan: "deny",  manual: "ask",   edits: "ask",   auto: "allow" },
    command: { plan: "deny",  manual: "ask",   edits: "ask",   auto: "ask" },
    git:     { plan: "deny",  manual: "ask",   edits: "ask",   auto: "ask" },
  };
  let mode = "manual";
  const modeTabs = $$("#mode-tabs button");
  const thumb = $(".segmented-thumb");

  function renderModes(animate) {
    const index = modes.indexOf(mode);
    thumb.style.transform = `translateX(${index * 100}%)`;
    modeTabs.forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.mode === mode)));
    $("#mode-desc").textContent = t(`modes.${mode}.desc`);
    const list = $("#mode-matrix");
    const previous = Object.fromEntries($$("li", list).map((li) => [li.dataset.row, li.dataset.v]));
    list.innerHTML = "";
    Object.entries(matrix).forEach(([row, verdicts]) => {
      const v = verdicts[mode];
      const li = el("li", "", `<span>${escapeHtml(t(`modes.row.${row}`))}</span>`);
      li.dataset.row = row;
      li.dataset.v = v;
      const chip = el("span", `verdict ${v}${animate && previous[row] && previous[row] !== v ? " flip" : ""}`, escapeHtml(t(`modes.v.${v}`)));
      li.appendChild(chip);
      list.appendChild(li);
    });
  }
  modeTabs.forEach((b) => b.addEventListener("click", () => { mode = b.dataset.mode; renderModes(true); }));
  listeners.push(() => renderModes(false));

  // ---------- checkpoints ----------

  // `at`: the point after which the change was made, so rolling back to it or earlier undoes it.
  const files = [
    { name: "src/main.rs", at: 0, kind: "base" },
    { name: "src/cart.rs", at: 2, kind: "modified" },
    { name: "Cargo.toml", at: 3, kind: "modified" },
    { name: "Cargo.lock", at: 3, kind: "modified" },
    { name: "tests/cart_test.rs", at: 4, kind: "added" },
  ];
  let activePoint = 5;
  const timeline = $("#cp-timeline");

  function renderCheckpoints() {
    timeline.innerHTML = "";
    const fill = el("span", "rail-fill");
    timeline.appendChild(fill);
    for (let p = 1; p <= 5; p++) {
      const li = el("li");
      const btn = el("button", `cp-point${p === activePoint ? " active" : ""}${p > activePoint ? " after" : ""}`,
        `<span class="node"></span><span class="cp-name">${p === 5 ? "HEAD" : "#" + p}</span><span class="cp-what">${escapeHtml(t(`cp.p${p}`))}</span>`);
      btn.addEventListener("click", () => { activePoint = p; renderCheckpoints(); });
      li.appendChild(btn);
      timeline.appendChild(li);
    }
    requestAnimationFrame(positionRail);

    $("#cp-state").textContent = activePoint === 5 ? t("cp.state.now") : `${t("cp.state.rolled")} #${activePoint}`;
    const list = $("#cp-file-list");
    list.innerHTML = "";
    files.forEach((f) => {
      const undone = f.at > 0 && f.at >= activePoint && activePoint < 5;
      const mark = f.kind === "added" ? "A" : f.kind === "modified" ? "M" : "·";
      list.appendChild(el("li", undone ? "undone" : f.kind, `<span class="st">${mark}</span><span class="nm">${f.name}</span>`));
    });
  }
  function positionRail() {
    const fill = $(".rail-fill", timeline);
    const nodes = $$(".cp-point .node", timeline);
    if (!fill || nodes.length < 2) return;
    const first = nodes[0].getBoundingClientRect();
    const last = nodes[nodes.length - 1].getBoundingClientRect();
    const active = $(".cp-point.active .node", timeline).getBoundingClientRect();
    const span = last.top - first.top;
    fill.style.top = `${first.top - timeline.getBoundingClientRect().top + first.height / 2}px`;
    fill.style.height = `${span}px`;
    fill.style.transform = `scaleY(${span > 0 ? (active.top - first.top) / span : 0})`;
  }

  listeners.push(renderCheckpoints);
  let railFrame = 0;
  addEventListener("resize", () => {
    cancelAnimationFrame(railFrame);
    railFrame = requestAnimationFrame(positionRail);
  });

  // ---------- model router ----------

  const examples = [1, 2, 3];
  let example = 0;
  let routerRun = 0;
  const wires = $$(".r-wire");
  const nodes = $$(".r-node");
  const dots = $("#router-dots");

  function renderRouterDots() {
    dots.innerHTML = "";
    examples.forEach((n, i) => {
      const b = el("button", i === example ? "on" : "");
      b.setAttribute("aria-label", t(`router.ex${n}.msg`));
      b.setAttribute("aria-pressed", String(i === example));
      b.addEventListener("click", () => { example = i; playRouter(); });
      dots.appendChild(b);
    });
  }

  async function playRouter() {
    const id = ++routerRun;
    const wait = (ms) => new Promise((resolve, reject) =>
      setTimeout(() => (id === routerRun ? resolve() : reject(new Cancelled())), reducedMotion ? 0 : ms));
    const n = examples[example];
    renderRouterDots();
    wires.forEach((w) => w.classList.remove("go"));
    nodes.forEach((x) => x.classList.remove("lit"));
    $("#r-msg").textContent = t(`router.ex${n}.msg`);
    $("#r-qa").innerHTML = [1, 2].map((q) =>
      `<dt>${escapeHtml(t(`router.ex${n}.q${q}`))}</dt><dd>${escapeHtml(t(`router.ex${n}.a${q}`))}</dd>`).join("");
    const rule = $("#r-rule");
    rule.textContent = t(`router.ex${n}.rule`);
    rule.classList.remove("on");
    $("#r-models").innerHTML = examples.map((m) => `<li>${escapeHtml(t(`router.ex${m}.model`))}</li>`).join("");

    try {
      nodes[0].classList.add("lit");
      await wait(700);
      wires[0].classList.add("go");
      await wait(500);
      nodes[0].classList.remove("lit");
      nodes[1].classList.add("lit");
      for (const dd of $$("#r-qa dd")) { await wait(500); dd.classList.add("on"); }
      await wait(500);
      wires[1].classList.add("go");
      await wait(500);
      nodes[1].classList.remove("lit");
      nodes[2].classList.add("lit");
      rule.classList.add("on");
      await wait(800);
      wires[2].classList.add("go");
      await wait(500);
      nodes[2].classList.remove("lit");
      nodes[3].classList.add("lit");
      $$("#r-models li")[example].classList.add("on");
      if (reducedMotion) return;
      await wait(3200);
      example = (example + 1) % examples.length;
      playRouter();
    } catch (e) {
      if (!(e instanceof Cancelled)) throw e;
    }
  }

  // Run the router only while it is on screen.
  let routerVisible = false;
  new IntersectionObserver(([entry]) => {
    if (entry.isIntersecting && !routerVisible) { routerVisible = true; playRouter(); }
    else if (!entry.isIntersecting && routerVisible) { routerVisible = false; routerRun++; }
  }, { threshold: 0.3 }).observe($("#router-board"));
  document.addEventListener("visibilitychange", () => {
    if (!routerVisible) return;
    if (document.visibilityState === "visible") playRouter();
    else routerRun++;
  });
  listeners.push(() => { if (routerVisible) playRouter(); else renderRouterDots(); });

  // ---------- get started ----------

  let tab = /Win/i.test(navigator.platform || navigator.userAgent) ? "windows" : "manual";
  const tabs = $$("#start-tabs button");
  function renderTabs() {
    tabs.forEach((b) => {
      const on = b.dataset.tab === tab;
      b.setAttribute("aria-selected", String(on));
      b.tabIndex = on ? 0 : -1;
    });
    $$(".term-panel").forEach((p) => { p.hidden = p.dataset.panel !== tab; });
  }
  tabs.forEach((b, i) => {
    b.addEventListener("click", () => { tab = b.dataset.tab; renderTabs(); });
    b.addEventListener("keydown", (e) => {
      const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
      const edge = { Home: 0, End: tabs.length - 1 }[e.key];
      if (step === undefined && edge === undefined) return;
      e.preventDefault();
      const next = tabs[edge ?? (i + step + tabs.length) % tabs.length];
      tab = next.dataset.tab;
      renderTabs();
      next.focus();
    });
  });
  renderTabs();

  const copyBtn = $("#copy-btn");
  copyBtn.addEventListener("click", async () => {
    const code = $(`.term-panel[data-panel="${tab}"] code`);
    const text = code.innerText.split("\n").map((l) => l.replace(/^[>$]\s*/, "")).join("\n");
    try {
      await navigator.clipboard.writeText(text);
      copyBtn.textContent = t("start.copied");
      copyBtn.classList.add("done");
      setTimeout(() => { copyBtn.textContent = t("start.copy"); copyBtn.classList.remove("done"); }, 1600);
    } catch (e) {
      copyBtn.textContent = t("start.copyFailed");
      setTimeout(() => { copyBtn.textContent = t("start.copy"); }, 1600);
    }
  });

  applyI18n();
})();
