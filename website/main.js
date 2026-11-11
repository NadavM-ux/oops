// Oops website: language, OS-aware downloads, the keycap demo and the try-it box.
(function () {
  "use strict";
  const html = document.documentElement;
  const REPO = "NadavM-ux/oops";
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const store = {
    get(k) { try { return localStorage.getItem(k); } catch { return null; } },
    set(k, v) { try { localStorage.setItem(k, v); } catch { /* private mode */ } },
  };

  let release = null; // latest GitHub release: { version, sizes }

  // ---------- language ----------
  function setLang(lang) {
    html.dataset.lang = lang;
    html.lang = lang;
    html.dir = lang === "he" ? "rtl" : "ltr";
    document.title = lang === "he" ? "Oops: הקלדתם בשפה הלא נכונה? Alt+O וזה מתוקן" : "Oops: typed in the wrong language? Alt+O fixes it";
    store.set("oops-lang", lang);
    renderMeta();
  }
  $$("[data-toggle-lang]").forEach((b) => b.addEventListener("click", () => setLang(html.dataset.lang === "he" ? "en" : "he")));
  const he = () => html.dataset.lang === "he";

  // ---------- operating system ----------
  const ua = navigator.userAgent;
  const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  const isPhone = /iPhone|iPad|iPod|Android/i.test(ua) || (/Mac/.test(platform) && navigator.maxTouchPoints > 1);
  const os = isPhone ? "mobile" : /Mac/i.test(platform) ? "mac" : /Win/i.test(platform) ? "win" : "other";
  html.dataset.os = os;
  const isMac = os === "mac" || (os !== "win" && /Mac|iPhone|iPad/.test(ua));

  // The shortcut as tiny keycaps, the way this visitor's keyboard labels it.
  const hk = isMac ? ["⌥", "O"] : ["Alt", "O"];
  $$("[data-hk]").forEach((el) => {
    el.setAttribute("dir", "ltr");
    el.replaceChildren(...hk.map((k) => Object.assign(document.createElement("kbd"), { textContent: k })));
    el.setAttribute("aria-label", isMac ? "Option + O" : "Alt + O");
  });
  if (os === "mobile") $("[data-phone]").hidden = false;
  const urlLang = new URLSearchParams(location.search).get("lang");
  setLang(urlLang === "en" || urlLang === "he" ? urlLang : store.get("oops-lang") || "he");

  // ---------- download tabs ----------
  function pickTab(which) {
    $$("[data-tab]").forEach((t) => t.setAttribute("aria-selected", String(t.dataset.tab === which)));
    $("#panel-mac").hidden = which !== "mac";
    $("#panel-win").hidden = which !== "win";
  }
  pickTab(os === "win" ? "win" : "mac");
  $$("[data-tab]").forEach((t) =>
    t.addEventListener("click", () => pickTab(t.dataset.tab))
  );
  $(".tabs").addEventListener("keydown", (e) => {
    if (!["ArrowLeft", "ArrowRight"].includes(e.key)) return;
    const next = $("#tab-mac").getAttribute("aria-selected") === "true" ? "win" : "mac";
    pickTab(next);
    $("#tab-" + next).focus();
  });
  $$("[data-pick]").forEach((a) => a.addEventListener("click", () => pickTab(a.dataset.pick)));

  // ---------- latest release: version + sizes ----------
  function mb(bytes) {
    return (bytes / 1024 / 1024).toFixed(1) + " MB";
  }
  function renderMeta() {
    const meta = $("[data-meta]");
    if (!meta) return;
    const parts = [];
    if (release) parts.push((he() ? "גרסה " : "Version ") + release.version);
    if (os === "mac") parts.push(he() ? "macOS 11 ומעלה" : "macOS 11 or later");
    else if (os === "win") parts.push(he() ? "Windows 10 ו־11" : "Windows 10 & 11");
    const size = release && release.sizes[os === "win" ? "win" : "mac"];
    if (size && os !== "mobile") parts.push(mb(size));
    meta.textContent = os === "mobile" ? "" : parts.join(" · ");
    if (release) {
      for (const k of ["mac", "win"]) {
        const el = $(`[data-size="${k}"]`);
        if (el && release.sizes[k]) el.textContent = "· " + mb(release.sizes[k]);
      }
    }
  }
  async function loadRelease() {
    try {
      const cached = sessionStorage.getItem("oops-release");
      if (cached) return JSON.parse(cached);
    } catch { /* ignore */ }
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: "application/vnd.github+json" } });
    if (!res.ok) return null;
    const data = await res.json();
    const size = (name) => (data.assets || []).find((a) => a.name === name)?.size;
    const info = { version: String(data.tag_name || "").replace(/^v/, ""), sizes: { mac: size("Oops-mac.dmg"), win: size("Oops-windows-setup.exe") } };
    try { sessionStorage.setItem("oops-release", JSON.stringify(info)); } catch { /* ignore */ }
    return info;
  }
  loadRelease().then((r) => { release = r; renderMeta(); }).catch(() => renderMeta());
  renderMeta();

  // ---------- hero: the fix, played on real keycaps ----------
  const EXAMPLES = ["akuo", "htkkv", "xccv", ",usv", "cuer"];
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const wordEl = $("#heroWord");
  const word = new KeyWord(wordEl, EXAMPLES[0]);
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  let idx = 0;
  let gen = 0; // bumping it stops the running loop
  let userTookOver = false;

  async function swapTo(i) {
    idx = (i + EXAMPLES.length) % EXAMPLES.length;
    await wordEl.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 160, fill: "forwards" }).finished;
    word.set(EXAMPLES[idx]);
    wordEl.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 220, fill: "forwards" });
  }
  async function loop(my) {
    while (my === gen) {
      await sleep(1100);
      if (my !== gen) return;
      await word.play("heb");
      await sleep(2400);
      if (my !== gen) return;
      await swapTo(idx + 1);
    }
  }
  const startAuto = () => loop(++gen);
  const stopAuto = () => { gen++; };

  if (reduce) word.play("heb");
  else {
    // Only animate while the hero is on screen.
    new IntersectionObserver(([entry]) => {
      if (entry.isIntersecting && !userTookOver) startAuto();
      else stopAuto();
    }).observe(wordEl);
  }
  $("#heroReplay").addEventListener("click", () => { userTookOver = true; stopAuto(); word.play(); });
  $("#heroNext").addEventListener("click", async () => {
    userTookOver = true;
    stopAuto();
    await swapTo(idx + 1);
    setTimeout(() => word.play("heb"), 450);
  });

  // Pressing the real shortcut on the page plays the demo (and fixes text in the try box).
  const isShortcut = (e) => e.code === "KeyO" && e.altKey && !e.ctrlKey && !e.metaKey;
  document.addEventListener("keydown", (e) => {
    if (!isShortcut(e) || e.target === $("#tryText")) return;
    e.preventDefault();
    userTookOver = true;
    stopAuto();
    word.play();
  });

  // ---------- try it ----------
  const ta = $("#tryText");
  const msg = $("#tryMsg");
  function fixTry() {
    const before = ta.value;
    const after = window.oopsConvert(before);
    if (!before.trim() || after === before) {
      msg.className = "try-msg bad";
      msg.textContent = he() ? "הקלידו קודם משהו באנגלית, למשל akuo." : "Type something in English first, like akuo.";
      return;
    }
    ta.value = after;
    msg.className = "try-msg";
    msg.textContent = he() ? "✓ תוקן. זה מה ש־Oops עושה, בכל אפליקציה." : "✓ Fixed. That's what Oops does, in every app.";
  }
  ta.addEventListener("keydown", (e) => {
    if (isShortcut(e)) {
      e.preventDefault();
      fixTry();
    }
  });
  ta.addEventListener("input", () => (msg.textContent = ""));
  $("#tryFix").addEventListener("click", fixTry);

  // ---------- copy buttons & sending the link from a phone ----------
  $$("[data-copy]").forEach((btn) =>
    btn.addEventListener("click", async () => {
      const text = btn.parentElement.querySelector("code").textContent.trim();
      try {
        await navigator.clipboard.writeText(text);
        const original = btn.innerHTML;
        btn.textContent = he() ? "הועתק ✓" : "Copied ✓";
        setTimeout(() => (btn.innerHTML = original), 1600);
      } catch {
        const range = document.createRange();
        range.selectNodeContents(btn.parentElement.querySelector("code"));
        getSelection().removeAllRanges();
        getSelection().addRange(range);
      }
    })
  );
  const share = $("[data-share]");
  if (share) {
    share.addEventListener("click", async () => {
      const url = "https://nadavm-ux.github.io/oops/";
      if (navigator.share) {
        try {
          await navigator.share({ title: "Oops", text: he() ? "Oops: מתקן טקסט שהוקלד בשפה הלא נכונה" : "Oops: fixes text typed in the wrong language", url });
          return;
        } catch { /* cancelled */ }
      }
      try {
        await navigator.clipboard.writeText(url);
        $("[data-shared]").hidden = false;
      } catch { /* nothing else to do */ }
    });
  }
})();
