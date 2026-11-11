// Oops settings & onboarding window.
(function () {
  "use strict";
  const T = window.__TAURI__ || window.__OOPS_MOCK__;
  const invoke = (cmd, args) => T.core.invoke(cmd, args);
  const listen = (name, cb) => T.event.listen(name, (e) => cb(e.payload));
  const root = document.getElementById("app");
  const $ = (sel) => root.querySelector(sel);

  const INSTAGRAM = "https://www.instagram.com/nadav.tech_/";
  const GITHUB = "https://github.com/NadavM-ux/oops";
  const DEFAULT_HK = { code: "KeyO", ctrl: false, alt: true, shift: false, meta: false };

  let S = null; // latest state from the native side
  let view = null; // "move" | "onboarding" | "settings"
  let step = 0; // onboarding step index
  let cleanup = []; // timers/listeners of the mounted view
  let recording = false;
  let finishing = false; // onboarding just ended; the window is about to close

  // ---------- text ----------
  const lang = () => (S && S.settings.language === "en" ? "en" : "he");
  const isMac = () => !S || S.platform === "macos";
  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  function raw(key) {
    const plat = `${key}.${isMac() ? "mac" : "win"}`;
    const d = I18N[lang()];
    return d[plat] ?? d[key] ?? I18N.he[plat] ?? I18N.he[key] ?? key;
  }
  /** Translated HTML: text is escaped, {hk} becomes keycaps, other {vars} are escaped values. */
  function t(key, vars = {}) {
    return esc(raw(key)).replace(/\{(\w+)\}/g, (_, k) => (k === "hk" ? hkInline(vars.hk || S.settings.hotkey) : k in vars ? (vars[k] instanceof Html ? vars[k].s : esc(vars[k])) : `{${k}}`));
  }
  function Html(s) {
    if (!(this instanceof Html)) return new Html(s);
    this.s = s;
  }

  // ---------- shortcut helpers ----------
  const MAC_MODS = [["ctrl", "⌃", "control"], ["alt", "⌥", "option"], ["shift", "⇧", "shift"], ["meta", "⌘", "command"]];
  const WIN_MODS = [["ctrl", "Ctrl"], ["alt", "Alt"], ["shift", "Shift"], ["meta", "Win"]];
  const PUNCT = { Backquote: "`", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Backslash: "\\", Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/", IntlBackslash: "§" };
  function keyLabel(code) {
    if (code.startsWith("Key")) return code.slice(3);
    if (code.startsWith("Digit")) return code.slice(5);
    return PUNCT[code] || code;
  }
  function modsOf(hk) {
    return (isMac() ? MAC_MODS : WIN_MODS).filter(([m]) => hk[m]);
  }
  function hkInline(hk) {
    const parts = modsOf(hk).map((m) => m[1]).concat(keyLabel(hk.code));
    return `<span class="hk" dir="ltr">${parts.map((p) => `<kbd>${esc(p)}</kbd>`).join("")}</span>`;
  }
  const sameHk = (a, b) => a.code === b.code && ["ctrl", "alt", "shift", "meta"].every((m) => !!a[m] === !!b[m]);
  const HEB = { q: "/", w: "'", e: "ק", r: "ר", t: "א", y: "ט", u: "ו", i: "ן", o: "ם", p: "פ", a: "ש", s: "ד", d: "ג", f: "כ", g: "ע", h: "י", j: "ח", k: "ל", l: "ך", z: "ז", x: "ס", c: "ב", v: "ה", b: "נ", n: "מ", m: "צ" };
  const modKey = (m) => `<span class="key mod"><span class="lg">${esc(isMac() ? `${m[1]} ${m[2]}` : m[1])}</span></span>`;
  /** The shortcut as big keycaps; a letter key shows both of its legends, like the real key. */
  function comboHtml(hk) {
    const label = keyLabel(hk.code);
    const main = hk.code.startsWith("Key")
      ? `<span class="key" data-active="lat"><span class="lg l">${esc(label)}</span><span class="lg h">${esc(HEB[label.toLowerCase()] || "")}</span></span>`
      : `<span class="key mod"><span class="lg">${esc(label)}</span></span>`;
    return modsOf(hk).map(modKey).concat(main).join("");
  }

  // ---------- small building blocks ----------
  const logo = `<span class="key" data-active="heb"><span class="lg l">A</span><span class="lg h">ש</span></span>`;
  const langBtn = () => `<button class="langswitch" data-act="lang" type="button">${esc(raw("lang.other"))}</button>`;
  const sw = (id, on, label) => `<button class="switch" type="button" role="switch" id="${id}" aria-checked="${on}" aria-label="${esc(label)}"></button>`;
  function row(id, on, title, hint) {
    return `<div class="row"><div class="txt"><b id="${id}-l">${t(title)}</b>${hint ? `<span>${t(hint)}</span>` : ""}</div>${sw(id, on, raw(title))}</div>`;
  }
  function on(el, ev, fn, opts) {
    el.addEventListener(ev, fn, opts);
    cleanup.push(() => el.removeEventListener(ev, fn, opts));
  }
  function later(fn, ms) {
    const id = setTimeout(fn, ms);
    cleanup.push(() => clearTimeout(id));
  }
  function every(fn, ms) {
    const id = setInterval(fn, ms);
    cleanup.push(() => clearInterval(id));
  }

  async function patch(p) {
    try {
      S = await invoke("update_settings", { patch: p });
    } catch (e) {
      return e;
    }
    return null;
  }

  // ---------- routing ----------
  function wantedView() {
    if (S.translocated) return "move";
    if (!S.settings.onboarded) return "onboarding";
    return "settings";
  }
  function render(force) {
    const html = document.documentElement;
    html.lang = lang();
    html.dir = lang() === "he" ? "rtl" : "ltr";
    html.dataset.platform = S.platform;
    const next = wantedView();
    if (force || next !== view) {
      view = next;
      mount();
    } else {
      update();
    }
  }
  function mount() {
    cleanup.forEach((f) => f());
    cleanup = [];
    if (recording) stopRecording();
    if (view === "move") mountMove();
    else if (view === "onboarding") mountOnboarding();
    else mountSettings();
    root.querySelectorAll('[data-act="lang"]').forEach((b) =>
      on(b, "click", async () => {
        await patch({ language: lang() === "he" ? "en" : "he" });
        render(true);
      })
    );
  }
  function update() {
    if (view === "onboarding") updateOnboarding();
    else if (view === "settings") updateSettings();
  }

  // ---------- macOS: running from the disk image ----------
  function mountMove() {
    root.innerHTML = `
      <div class="topline"><div class="brand">${logo}Oops</div>${langBtn()}</div>
      <section class="onb fade-in">
        <h1>${t("move.title")}</h1>
        <p class="lead">${t("move.body")}</p>
        <ol class="lead"><li>${t("move.s1")}</li><li>${t("move.s2")}</li></ol>
        <div class="grow"></div>
        <div class="actions">
          <button class="btn" type="button" id="open">${t("move.open")}</button>
          <button class="linkbtn" type="button" id="quit">${t("move.quit")}</button>
        </div>
      </section>`;
    on($("#open"), "click", () => invoke("show_applications_folder"));
    on($("#quit"), "click", () => invoke("quit"));
  }

  // ---------- onboarding ----------
  const steps = () => (isMac() ? ["welcome", "permission", "try", "done"] : ["welcome", "try", "done"]);
  function mountOnboarding() {
    const names = steps();
    step = Math.min(step, names.length - 1);
    const name = names[step];
    const bar = names.map((_, i) => `<i class="${i <= step ? "on" : ""}"></i>`).join("");
    root.innerHTML = `
      <div class="topline"><div class="brand">${logo}Oops</div>${langBtn()}</div>
      <div class="steps" aria-hidden="true">${bar}</div>
      <section class="onb fade-in" id="onb">${STEP[name].html()}</section>`;
    STEP[name].wire();
  }
  function go(delta) {
    step = Math.max(0, Math.min(steps().length - 1, step + delta));
    mount();
  }
  function updateOnboarding() {
    const name = steps()[step];
    if (STEP[name].update) STEP[name].update();
  }

  const STEP = {
    welcome: {
      html: () => `
        <div class="stage"><div class="keyword" id="kw"></div></div>
        <h1>${t("welcome.title")}</h1>
        <p class="lead">${t("welcome.body")}</p>
        <div class="grow"></div>
        <div class="actions"><button class="btn" type="button" id="next">${t("welcome.cta")}</button><span class="small">${t("welcome.time")}</span></div>`,
      wire() {
        const kw = new KeyWord($("#kw"), "akuo");
        later(() => kw.play("heb"), 900);
        every(() => kw.play(), 3200);
        on($("#next"), "click", () => go(1));
        $("#next").focus();
      },
    },

    permission: {
      html: () => `
        <h1>${t("perm.title")}</h1>
        <p class="lead">${t("perm.body")}</p>
        <div class="actions" style="margin-top:6px"><button class="btn" type="button" id="perm">${t("perm.cta")}</button></div>
        <p style="margin-top:22px">${t("perm.how")}</p>
        <div class="permshot" aria-hidden="true"><img src="icon.png" alt=""><span>Oops</span><span class="switch" aria-checked="true"></span></div>
        <div id="permstatus"></div>
        <p class="small" style="margin-top:14px">${t("perm.help")}</p>
        <div class="grow"></div>
        <div class="actions"><button class="btn" type="button" id="next">${t("perm.continue")}</button></div>`,
      wire() {
        on($("#perm"), "click", () => invoke("request_permission"));
        on($("#next"), "click", () => go(1));
        STEP.permission.update();
      },
      update() {
        const granted = S.permission !== "missing";
        const box = $("#permstatus");
        const wasGranted = box.dataset.granted === "1";
        box.dataset.granted = granted ? "1" : "0";
        box.innerHTML = granted ? `<span class="granted">✓ ${t("perm.granted")}</span>` : `<span class="waiting"><span class="spin"></span>${t("perm.waiting")}</span>`;
        $("#next").disabled = !granted;
        if (granted && !wasGranted && box.dataset.seen === "1") {
          invoke("ui_ready"); // bring the window back from System Settings
          later(() => go(1), 900);
        }
        box.dataset.seen = "1";
      },
    },

    try: {
      html: () => `
        <h1>${t("try.title")}</h1>
        <p class="lead">${t("try.body", { word: Html('<b class="ltr">akuo</b>') })}</p>
        ${practiceHtml()}
        <p style="margin-top:6px"><button class="linkbtn" type="button" id="stuck">${t("try.stuck")}</button></p>
        <p class="small" id="stuckmsg" hidden>${t("try.help")}</p>
        <div class="grow"></div>
        <div class="actions"><button class="btn" type="button" id="next">${t("try.next")}</button><button class="linkbtn" type="button" id="skip">${t("try.skip")}</button></div>`,
      wire() {
        wirePractice();
        on($("#stuck"), "click", () => ($("#stuckmsg").hidden = false));
        on($("#next"), "click", () => go(1));
        on($("#skip"), "click", () => go(1));
        $("#practice").focus();
      },
    },

    done: {
      html: () => `
        <h1>${t("done.title")}</h1>
        <p class="lead">${t("done.body")}</p>
        ${trayShot()}
        <div class="card" style="margin-top:18px">${row("autostart", S.settings.launchAtLogin, "done.autostart", "done.autostart.hint")}</div>
        <div class="grow"></div>
        <div class="actions"><button class="btn" type="button" id="finish">${t("done.cta")}</button></div>`,
      wire() {
        const toggle = $("#autostart");
        on(toggle, "click", () => toggle.setAttribute("aria-checked", toggle.getAttribute("aria-checked") !== "true"));
        on($("#finish"), "click", async () => {
          step = 0;
          finishing = true;
          S = await invoke("finish_onboarding", { launchAtLogin: toggle.getAttribute("aria-checked") === "true" });
          // The window closes itself; show settings only if it is still open.
          setTimeout(() => {
            finishing = false;
            render();
          }, 1500);
        });
        $("#finish").focus();
      },
    },
  };

  /** A small picture of the menu bar / taskbar with the Oops icon circled. */
  function trayShot() {
    const me = `<span class="me"><img src="tray-${isMac() ? "mac" : "win"}.png" alt=""><span class="ring"></span></span>`;
    const dots = '<i class="ico"></i><i class="ico"></i>';
    const bar = isMac()
      ? `<span class="apple"></span><span>Finder</span><span class="spacer"></span>${me}${dots}<span>9:41</span>`
      : `<span class="spacer"></span><span>^</span>${me}${dots}<span>9:41</span>`;
    return `<div class="trayshot" aria-hidden="true"><div class="bar">${bar}</div></div>`;
  }

  // ---------- practice box (onboarding + settings) ----------
  function practiceHtml() {
    return `<div class="practice"><textarea id="practice" rows="2" spellcheck="false" autocomplete="off" autocorrect="off" autocapitalize="off" placeholder="${esc(raw("try.placeholder"))}"></textarea><div class="result" id="result" role="status"></div></div>`;
  }
  function showResult(kind) {
    const box = $("#result");
    if (!box) return;
    box.className = "result show" + (kind === "nothing" ? " bad" : "");
    box.innerHTML = kind === "fixed" ? `✓ ${t("try.success")}` : kind === "undo" ? t("try.undone") : t("try.nothing");
  }
  function wirePractice() {
    const ta = $("#practice");
    // If the global shortcut isn't active (e.g. no permission yet), still let people try it here.
    on(ta, "keydown", async (e) => {
      const hk = S.settings.hotkey;
      const match = e.code === hk.code && e.altKey === !!hk.alt && e.ctrlKey === !!hk.ctrl && e.shiftKey === !!hk.shift && e.metaKey === !!hk.meta;
      if (!match) return;
      e.preventDefault();
      const fixed = await invoke("convert_text", { text: ta.value });
      if (fixed) {
        ta.value = fixed;
        showResult("fixed");
      } else showResult("nothing");
    });
    on(ta, "input", () => {
      const box = $("#result");
      if (box) box.className = "result";
    });
  }

  // ---------- settings ----------
  function mountSettings() {
    const s = S.settings;
    root.innerHTML = `
      <div class="topline">
        <div class="brand">${logo}Oops<span class="pill" id="pill"></span></div>
        <div style="display:flex;gap:8px;align-items:center"><button class="btn small ghost" type="button" id="pause"></button>${langBtn()}</div>
      </div>
      <div id="banners"></div>
      <div class="card">
        <div class="shortcut">
          <div><b>${t("set.shortcut")}</b><div class="small" style="max-width:30ch">${t("set.shortcut.hint")}</div></div>
          <div class="combo" id="combo"></div>
        </div>
        <div style="display:flex;gap:14px;align-items:center;margin-top:14px">
          <button class="btn small ghost" type="button" id="change">${t("set.change")}</button>
          <button class="linkbtn" type="button" id="reset"></button>
        </div>
        <div id="hkmsg" aria-live="polite"></div>
      </div>
      <h2>${t("set.practice")}</h2>
      ${practiceHtml()}
      <h2>${t("set.behavior")}</h2>
      <div class="card">
        ${row("switchLayout", s.switchLayout, "set.switchLayout", "set.switchLayout.hint")}
        ${row("sounds", s.sounds, "set.sounds")}
      </div>
      <h2>${t("set.general")}</h2>
      <div class="card">
        ${row("launchAtLogin", s.launchAtLogin, "set.launchAtLogin")}
        ${row("checkUpdates", s.checkUpdates, "set.checkUpdates", "set.checkUpdates.hint")}
        <div class="row"><div class="txt"><b>${t("set.language")}</b></div>
          <div class="segmented" role="group"><button type="button" data-lang="he" aria-pressed="${lang() === "he"}">עברית</button><button type="button" data-lang="en" aria-pressed="${lang() === "en"}">English</button></div>
        </div>
      </div>
      <div class="foot">
        <span class="count" id="count"></span>
        <span class="links">
          <button class="linkbtn" type="button" id="howto">${t("foot.howTo")}</button>
          <a href="#" id="ig">${t("foot.by")} <span class="ltr">@nadav.tech_</span></a>
          <a href="#" id="gh">GitHub</a>
          <button class="linkbtn" type="button" id="quit">${t("foot.quit")}</button>
        </span>
        <span>${t("foot.version", { v: S.version })}</span>
      </div>`;

    for (const key of ["switchLayout", "sounds", "launchAtLogin", "checkUpdates"]) {
      const el = $("#" + key);
      on(el, "click", async () => {
        const next = el.getAttribute("aria-checked") !== "true";
        el.setAttribute("aria-checked", next);
        await patch({ [key]: next });
      });
    }
    root.querySelectorAll("[data-lang]").forEach((b) =>
      on(b, "click", async () => {
        if (b.dataset.lang === lang()) return;
        await patch({ language: b.dataset.lang });
        render(true);
      })
    );
    on($("#pause"), "click", () => patch({ enabled: !S.settings.enabled }));
    on($("#change"), "click", () => (recording ? stopRecording() : startRecording()));
    on($("#reset"), "click", async () => {
      await patch({ hotkey: DEFAULT_HK });
      showHkMsg("ok", t("set.saved"));
      updateSettings();
    });
    on($("#howto"), "click", async () => {
      step = 0;
      S = await invoke("restart_onboarding");
      render();
    });
    on($("#ig"), "click", (e) => (e.preventDefault(), invoke("open_url", { url: INSTAGRAM })));
    on($("#gh"), "click", (e) => (e.preventDefault(), invoke("open_url", { url: GITHUB })));
    on($("#quit"), "click", () => invoke("quit"));
    wirePractice();
    updateSettings();
  }

  function updateSettings() {
    const s = S.settings;
    const pill = $("#pill");
    pill.dataset.s = S.status;
    pill.innerHTML = `<i></i>${t("status." + S.status)}`;
    $("#pause").textContent = raw(s.enabled ? "set.pause" : "set.resume");
    if (!recording) $("#combo").innerHTML = comboHtml(s.hotkey);
    const reset = $("#reset");
    reset.hidden = sameHk(s.hotkey, DEFAULT_HK);
    reset.innerHTML = t("set.reset", { hk: DEFAULT_HK });
    for (const key of ["switchLayout", "sounds", "launchAtLogin", "checkUpdates"]) $("#" + key).setAttribute("aria-checked", !!s[key]);
    $("#count").innerHTML = t("foot.count", { n: Html(`<b>${Number(s.fixCount).toLocaleString(lang() === "he" ? "he-IL" : "en-US")}</b>`) });

    const banners = [];
    if (S.status === "needsPermission") banners.push(`<div class="banner"><span>${t("set.permBanner")}</span><button class="btn small" type="button" data-banner="perm">${t("set.permFix")}</button></div>`);
    else if (S.status === "error") banners.push(`<div class="banner"><span>${t("set.errorBanner")}</span></div>`);
    else if (S.status === "paused") banners.push(`<div class="banner info"><span>${t("set.pausedBanner")}</span><button class="btn small" type="button" data-banner="resume">${t("set.resume")}</button></div>`);
    if (S.update) banners.push(`<div class="banner info"><span>${t("set.update", { v: S.update.version })}</span><button class="btn small" type="button" data-banner="update">${t("set.updateCta")}</button></div>`);
    if (!S.hebrewLayout) banners.push(`<div class="banner"><span>${t("set.noHebrew")}<br><b>${t("set.noHebrew.how")}</b></span></div>`);
    const holder = $("#banners");
    const html = banners.join("");
    if (holder.dataset.html !== html) {
      holder.dataset.html = html;
      holder.innerHTML = html;
      holder.querySelectorAll("[data-banner]").forEach((b) =>
        b.addEventListener("click", () => {
          const what = b.dataset.banner;
          if (what === "perm") invoke("request_permission");
          if (what === "resume") patch({ enabled: true });
          if (what === "update") invoke("open_url", { url: S.update.url });
        })
      );
    }
  }

  // ---------- shortcut recorder ----------
  const MODIFIER_CODES = /^(Shift|Control|Alt|Meta|OS)(Left|Right)$|^(CapsLock|Fn|FnLock|AltGraph)$/;
  function showHkMsg(kind, html) {
    const box = $("#hkmsg");
    if (!box) return;
    box.className = kind === "ok" ? "hint-ok" : "hint-err";
    box.innerHTML = html;
  }
  function drawRecording(mods) {
    const combo = $("#combo");
    combo.classList.add("recording");
    const held = modsOf(mods || {});
    combo.innerHTML = held.length
      ? held.map(modKey).join("") + `<span class="placeholder">+ …</span>`
      : `<span class="placeholder">${t("set.recording")}</span>`;
  }
  function startRecording() {
    recording = true;
    invoke("set_recording", { on: true });
    $("#change").textContent = raw("set.cancel");
    showHkMsg("ok", "");
    drawRecording(null);
    window.addEventListener("keydown", onRecordKey, true);
    window.addEventListener("keyup", onRecordKeyUp, true);
    window.addEventListener("blur", stopRecording);
  }
  function stopRecording() {
    if (!recording) return;
    recording = false;
    invoke("set_recording", { on: false });
    window.removeEventListener("keydown", onRecordKey, true);
    window.removeEventListener("keyup", onRecordKeyUp, true);
    window.removeEventListener("blur", stopRecording);
    const combo = $("#combo");
    if (combo) {
      combo.classList.remove("recording");
      combo.innerHTML = comboHtml(S.settings.hotkey);
    }
    const change = $("#change");
    if (change) change.textContent = raw("set.change");
  }
  const modsFromEvent = (e) => ({ ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey });
  function onRecordKeyUp(e) {
    if (!recording) return;
    e.preventDefault();
    if (MODIFIER_CODES.test(e.code)) drawRecording(modsFromEvent(e));
  }
  async function onRecordKey(e) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    const mods = modsFromEvent(e);
    if (MODIFIER_CODES.test(e.code) || e.repeat) {
      drawRecording(mods);
      return;
    }
    if (e.code === "Escape" && !mods.ctrl && !mods.alt && !mods.meta && !mods.shift) {
      stopRecording();
      return;
    }
    const hotkey = { code: e.code, ...mods };
    try {
      await invoke("validate_hotkey", { hotkey });
    } catch (err) {
      showHkMsg("err", t("err." + err));
      drawRecording(null);
      return;
    }
    const failed = await patch({ hotkey });
    stopRecording();
    if (failed) showHkMsg("err", t("err." + failed));
    else {
      showHkMsg("ok", t("set.saved", { hk: hotkey }));
      updateSettings();
    }
  }

  // ---------- events from the native side ----------
  function onEngine(ev) {
    if (!document.hasFocus()) return;
    if (ev.type === "fixed") showResult(ev.undo ? "undo" : "fixed");
    else if (ev.type === "nothingToFix") showResult("nothing");
  }

  async function start() {
    S = await invoke("get_state");
    render(true);
    listen("oops://state", (s) => {
      S = s;
      if (!finishing) render();
    });
    listen("oops://engine", onEngine);
    requestAnimationFrame(() => invoke("ui_ready"));
  }
  start();
})();
