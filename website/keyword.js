// A word drawn as Israeli dual-legend keycaps, and the "fix" animation:
// the keys go down, the Hebrew legend takes over, and the row mirrors itself,
// because the same keys spell Hebrew from right to left.
(function () {
  "use strict";
  // US key -> the character the same key types on the Hebrew (SI-1452) layout.
  const PAIRS = {
    "`": ";", q: "/", w: "'", e: "ק", r: "ר", t: "א", y: "ט", u: "ו", i: "ן", o: "ם", p: "פ",
    a: "ש", s: "ד", d: "ג", f: "כ", g: "ע", h: "י", j: "ח", k: "ל", l: "ך", ";": "ף", "'": ",",
    z: "ז", x: "ס", c: "ב", v: "ה", b: "נ", n: "מ", m: "צ", ",": "ת", ".": "ץ", "/": ".",
  };
  const REVERSE = Object.fromEntries(Object.entries(PAIRS).map(([en, he]) => [he, en]));
  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const isHebrew = (c) => c >= "א" && c <= "ת";

  /** Same idea as the app's Rust converter, for demos and the website's try-it box. */
  function convert(text) {
    let latin = 0, hebrew = 0;
    for (const c of text) {
      if (/[a-z]/i.test(c)) latin++;
      else if (isHebrew(c)) hebrew++;
    }
    if (!latin && !hebrew) return text;
    const toHebrew = latin >= hebrew;
    const targetAnywhere = [...text].some((c) => (toHebrew ? isHebrew(c) : /[a-z]/.test(c)));
    return text.replace(/\S+/g, (word) => {
      const chars = [...word];
      const hasSrc = chars.some((c) => (toHebrew ? /[a-z]/i.test(c) : isHebrew(c)));
      const hasTgt = chars.some((c) => (toHebrew ? isHebrew(c) : /[a-z]/.test(c)));
      const all = (hasSrc && !hasTgt) || (!hasSrc && !hasTgt && !targetAnywhere);
      const lettersOnly = hasSrc && hasTgt;
      return chars
        .map((c) => {
          const isSrcLetter = toHebrew ? /[a-z]/i.test(c) : isHebrew(c);
          if (!all && !(lettersOnly && isSrcLetter)) return c;
          const mapped = toHebrew ? PAIRS[c.toLowerCase()] : REVERSE[c];
          return mapped === undefined ? c : mapped;
        })
        .join("");
    });
  }

  function makeKey(latin) {
    const hebrew = PAIRS[latin] ?? latin;
    const el = document.createElement("span");
    el.className = "key";
    el.dataset.active = "lat";
    const l = document.createElement("span");
    l.className = "lg l";
    l.textContent = latin;
    const h = document.createElement("span");
    h.className = "lg h" + (isHebrew(hebrew) ? "" : " punct");
    h.textContent = hebrew;
    el.append(l, h);
    return el;
  }

  class KeyWord {
    /** @param {HTMLElement} el container (gets direction:ltr from CSS) */
    constructor(el, latinWord) {
      this.el = el;
      this.busy = false;
      this.set(latinWord);
    }

    set(latinWord) {
      this.latin = latinWord;
      this.hebrew = [...latinWord].map((c) => PAIRS[c] ?? c).join("");
      this.state = "lat";
      this.el.replaceChildren(...[...latinWord].map(makeKey));
      this.el.setAttribute("role", "img");
      this.el.setAttribute("aria-label", latinWord);
    }

    /** Animates to "heb" or "lat" (default: the other one). */
    async play(target) {
      target = target || (this.state === "lat" ? "heb" : "lat");
      if (this.busy || target === this.state) return;
      this.busy = true;
      const keys = [...this.el.children];
      const n = keys.length;
      if (reduceMotion.matches) {
        keys.forEach((k) => (k.dataset.active = target));
        this.el.replaceChildren(...keys.reverse());
      } else {
        keys.forEach((k, i) =>
          setTimeout(() => {
            k.classList.add("is-down");
            setTimeout(() => k.classList.remove("is-down"), 120);
          }, i * 55)
        );
        await wait(n * 55 + 70);
        keys.forEach((k) => (k.dataset.active = target));
        await wait(160);
        const xs = keys.map((k) => k.offsetLeft);
        const lift = keys[0].offsetHeight * 0.62;
        const anims = keys.map((k, i) => {
          const dx = xs[n - 1 - i] - xs[i];
          // Keys travelling right hop over, keys travelling left duck under.
          const dy = dx > 0 ? -lift : dx < 0 ? lift * 0.55 : -lift * 0.25;
          k.style.zIndex = dx > 0 ? "2" : "1";
          return k.animate(
            [
              { transform: "translate(0, 0)" },
              { transform: `translate(${dx / 2}px, ${dy}px)`, offset: 0.5 },
              { transform: `translate(${dx}px, 0)` },
            ],
            { duration: 620, delay: i * 40, easing: "cubic-bezier(.4, .1, .2, 1)", fill: "forwards" }
          );
        });
        await Promise.all(anims.map((a) => a.finished.catch(() => {})));
        this.el.replaceChildren(...keys.reverse());
        anims.forEach((a) => a.cancel());
        keys.forEach((k) => (k.style.zIndex = ""));
      }
      this.state = target;
      this.el.setAttribute("aria-label", target === "heb" ? this.hebrew : this.latin);
      this.busy = false;
    }
  }

  window.KeyWord = KeyWord;
  window.oopsConvert = convert;
})();
