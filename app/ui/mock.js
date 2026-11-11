// Browser preview only: stands in for the native app when window.__TAURI__ is missing.
// Query params: ?view=settings|move  &platform=windows  &lang=en  &status=needsPermission|paused|error
//               &update=1  &nohe=1
(function () {
  "use strict";
  if (window.__TAURI__) return;
  const q = new URLSearchParams(location.search);
  const status = q.get("status") || "active";
  const state = {
    settings: {
      hotkey: { code: "KeyO", ctrl: false, alt: true, shift: false, meta: false },
      enabled: status !== "paused",
      switchLayout: true,
      sounds: true,
      launchAtLogin: true,
      checkUpdates: true,
      language: q.get("lang") || "he",
      onboarded: q.get("view") === "settings",
      fixCount: 42,
      hadPermission: false,
    },
    platform: q.get("platform") || "macos",
    version: "1.0.0",
    status,
    permission: q.get("platform") === "windows" ? "notNeeded" : status === "needsPermission" ? "missing" : "granted",
    update: q.get("update") ? { version: "1.1.0", url: "https://nadavm-ux.github.io/oops/#download" } : null,
    hebrewLayout: !q.get("nohe"),
    englishLayout: true,
    translocated: q.get("view") === "move",
    autostarted: false,
  };
  if (q.get("onb") === "perm") state.permission = "missing";
  const listeners = {};
  const clone = () => JSON.parse(JSON.stringify(state));
  const emit = (name, payload) => (listeners[name] || []).forEach((cb) => cb({ payload }));
  const MODS_OK = (h) => (state.platform === "macos" ? h.alt || h.ctrl || h.meta : h.alt || h.ctrl);

  window.__OOPS_MOCK__ = {
    core: {
      async invoke(cmd, args = {}) {
        switch (cmd) {
          case "get_state":
            return clone();
          case "update_settings":
            Object.assign(state.settings, args.patch);
            if ("enabled" in args.patch) state.status = args.patch.enabled ? "active" : "paused";
            setTimeout(() => emit("oops://state", clone()));
            return clone();
          case "validate_hotkey": {
            const h = args.hotkey;
            if (!/^(Key[A-Z]|Digit\d|F\d{1,2}|Backquote|Minus|Equal|Bracket(Left|Right)|Backslash|Semicolon|Quote|Comma|Period|Slash)$/.test(h.code)) throw "unsupportedKey";
            if (!/^F\d/.test(h.code) && !MODS_OK(h)) throw "needsModifier";
            if (state.platform === "macos" && h.meta && !h.alt && !h.ctrl && !h.shift) throw "reserved";
            return null;
          }
          case "request_permission":
            setTimeout(() => {
              state.permission = "granted";
              state.status = "active";
              emit("oops://state", clone());
            }, 2500);
            return null;
          case "finish_onboarding":
            state.settings.onboarded = true;
            state.settings.launchAtLogin = args.launchAtLogin;
            setTimeout(() => emit("oops://state", clone()));
            return clone();
          case "restart_onboarding":
            state.settings.onboarded = false;
            return clone();
          case "convert_text":
            return window.oopsConvert(args.text);
          default:
            console.log("[mock] invoke", cmd, args);
            return null;
        }
      },
    },
    event: {
      async listen(name, cb) {
        (listeners[name] ||= []).push(cb);
        return () => {};
      },
    },
  };
})();
