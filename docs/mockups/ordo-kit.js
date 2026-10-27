// Ordo mockup kit: a sample Mass, a canonical-slide renderer, fit-to-width windows, and an
// interactive Presenter that follows MVP-PLAN §8. Mockup behaviour only; the app is native GPUI.
(function () {
  "use strict";

  // ── fixture: one Cebuano Sunday Mass in the six-section scaffold (Decision 171) ──────────
  // Cue kinds: projectable (lyrics, response, psalm, reading-id, reading, song-id, celebration,
  // general), operator-only (trigger, wait, hold), and unavailable.
  var MASS = {
    title: "Ika-30 nga Domingo sa Ordinaryong Panahon",
    date: "Dom 25 Okt 2026 · 9:00", choir: "Koro San Roque", language: "Cebuano",
    sections: [
      { title: "Before Mass", items: [
        { title: "Pahibalo", type: "General Presentation", cues: [
          { k: "general", h: "Pahibalo", b: "Maayong buntag! Palihug isira o i-silent ang inyong mga cellphone.", f: "Parokya ni San Roque" },
          { k: "general", h: "Pahibalo", b: "Ang Bible study matag Huwebes, 6:00 sa gabii, sa parish hall.", f: "Parokya ni San Roque" } ] } ] },
      { title: "Introductory Rites", items: [
        { title: "Entrance Song", song: "Daygon Ta", type: "Song", cues: [
          { k: "song-id", t: "Daygon Ta", s: "R. Villanueva · Koro San Roque" },
          { k: "lyrics", h: "Bersikulo 1", b: "Daygon ta ang Ginoo\nsa tanang kasingkasing" },
          { k: "lyrics", h: "Koro", b: "Daygon, daygon ta\nang ngalan sa Ginoo" },
          { k: "lyrics", h: "Bersikulo 2", b: "Siya ang atong bato\nug atong kalig-on" },
          { k: "lyrics", h: "Koro", b: "Daygon, daygon ta\nang ngalan sa Ginoo" } ] },
        { title: "Greeting", type: "Response sequence", cues: [
          { k: "trigger", who: "P", b: "Sa ngalan sa Amahan, ug sa Anak, ug sa Espiritu Santo." },
          { k: "response", h: "TUBAG:", b: "Amen." },
          { k: "trigger", who: "P", b: "Ang Ginoo maanaa kaninyo." },
          { k: "response", h: "TUBAG:", b: "Ug maanaa usab sa imong espiritu." } ] },
        { title: "Kyrie", song: "Ginoo, Kaloy-i Kami", type: "Song", cues: [
          { k: "lyrics", h: "Kyrie", b: "Ginoo, kaloy-i kami" },
          { k: "lyrics", h: "Kyrie", b: "Kristo, kaloy-i kami" } ] },
        { title: "Gloria", song: "Himaya sa Dios", type: "Song", cues: [
          { k: "lyrics", h: "Gloria", b: "Himaya sa Dios sa kahitas-an\nug sa yuta kalinaw" },
          { k: "lyrics", h: "Gloria", b: "Kanimo kami nagadayeg\nnagadalayeg kanimo" } ] } ] },
      { title: "Liturgy of the Word", items: [
        { title: "First Reading", type: "Reading", cues: [
          { k: "reading-id", b: "Pagbasa gikan sa Libro ni Propeta Jeremias", f: "Jeremias 31:7-9" },
          { k: "reading", b: "Mao kini ang giingon sa Ginoo: Pag-awit sa kalipay alang kang Jacob, pagsinggit alang sa nag-una sa kanasoran.", f: "Jeremias 31:7-9" },
          { k: "trigger", who: "L", b: "Ang Pulong sa Dios." },
          { k: "response", h: "TUBAG:", b: "Salamat sa Dios." } ] },
        { title: "Responsorial Psalm", type: "Psalm", cues: [
          { k: "psalm", h: "SALMO RESPONSORYO:", b: "Dagkong mga butang ang gibuhat sa Ginoo alang kanato.", f: "Salmo 126" },
          { k: "wait", b: "Cantor sings verse 1 (not projected)" },
          { k: "psalm", h: "SALMO RESPONSORYO:", b: "Dagkong mga butang ang gibuhat sa Ginoo alang kanato.", f: "Salmo 126" } ] },
        { title: "Second Reading", type: "Reading", cues: [
          { k: "reading-id", b: "Pagbasa gikan sa Sulat ngadto sa mga Hebreohanon", f: "Hebreohanon 5:1-6" },
          { k: "reading", b: "Ang matag labawng pari gipili gikan sa mga tawo ug gitudlo sa pag-alagad sa Dios alang kanila.", f: "Hebreohanon 5:1-6" } ] },
        { title: "Gospel Acclamation", song: "Aleluya", type: "Song", cues: [
          { k: "lyrics", h: "Aleluya", b: "Aleluya, aleluya" } ] },
        { title: "Gospel", type: "Reading", cues: [
          { k: "trigger", who: "P", b: "Pagbasa sa balaang Ebanghelyo sumala kang San Marcos." },
          { k: "response", h: "TUBAG:", b: "Himaya kanimo, O Ginoo." },
          { k: "reading-id", b: "Ebanghelyo sumala kang San Marcos", f: "Marcos 10:46-52" },
          { k: "hold", b: "Homily" } ] } ] },
      { title: "Liturgy of the Eucharist", items: [
        { title: "Offertory Song", song: "Ihalad Namo", type: "Song", cues: [
          { k: "unavailable", b: "Ihalad Namo", why: "Arrangement image asset is missing from the snapshot" },
          { k: "unavailable", b: "Ihalad Namo · Koro", why: "Arrangement image asset is missing from the snapshot" } ] },
        { title: "Sanctus", song: "Santos", type: "Song", cues: [
          { k: "lyrics", h: "Sanctus", b: "Santos, santos, santos\nang Ginoong Dios" } ] },
        { title: "Memorial Acclamation", type: "Response sequence", cues: [
          { k: "trigger", who: "P", b: "Ang misteryo sa pagtoo." },
          { k: "response", h: "TUBAG:", b: "Ang imong kamatayon among ginasangyaw, O Ginoo." } ] },
        { title: "Our Father", type: "Response sequence", cues: [
          { k: "response", h: "", b: "Amahan namo, nga anaa sa mga langit, pagdaygon ang imong ngalan." },
          { k: "response", h: "", b: "Ihatag kanamo karong adlawa ang among kalan-on." } ] },
        { title: "Communion Song", song: "Pan sa Kinabuhi", type: "Song", cues: [
          { k: "song-id", t: "Pan sa Kinabuhi", s: "Koro San Roque" },
          { k: "lyrics", h: "Bersikulo 1", b: "Ikaw ang pan sa kinabuhi\nnga gikan sa langit" } ] } ] },
      { title: "Concluding Rites", items: [
        { title: "Dismissal", type: "Response sequence", cues: [
          { k: "trigger", who: "P", b: "Panglakaw kamo sa kalinaw." },
          { k: "response", h: "TUBAG:", b: "Salamat sa Dios." } ] },
        { title: "Recessional", song: "Padayon", type: "Song", cues: [
          { k: "lyrics", h: "Koro", b: "Padayon kita\nsa dalan sa Ginoo" } ] } ] },
      { title: "After Mass", items: [
        { title: "Pahibalo", type: "General Presentation", cues: [
          { k: "general", h: "Pahibalo", b: "Salamat sa inyong pagtambong. Kitakits sa sunod Domingo!", f: "Parokya ni San Roque" } ] } ] }
    ]
  };

  // Flatten to cues with stable indices; each cue knows its item and section.
  var ITEMS = [], CUES = [];
  MASS.sections.forEach(function (sec, si) {
    sec.items.forEach(function (it) {
      it.section = si; it.n = ITEMS.length + 1; it.first = CUES.length; ITEMS.push(it);
      it.cues.forEach(function (c, ci) { c.item = it; c.i = CUES.length; c.ci = ci; CUES.push(c); });
    });
  });
  var PROJECTABLE = { lyrics: 1, response: 1, psalm: 1, "reading-id": 1, reading: 1, "song-id": 1, celebration: 1, general: 1 };
  function projectable(c) { return !!PROJECTABLE[c.k]; }
  var KIND_LABEL = { lyrics: "Lyrics", response: "Response", psalm: "Psalm", "reading-id": "Reading title", reading: "Passage",
    "song-id": "Song title", celebration: "Celebration", general: "Text", trigger: "Operator-only trigger",
    wait: "Wait", hold: "Hold", unavailable: "Unavailable" };
  function cueLabel(c) {
    if (c.k === "song-id") return c.t;
    if (c.k === "lyrics" || c.k === "psalm") return c.h + " · " + c.b.split("\n")[0];
    return (c.b || "").split("\n")[0];
  }

  // ── rendering helpers ─────────────────────────────────────────────────────────────────────
  function esc(s) { return String(s == null ? "" : s).replace(/[&<>"]/g, function (ch) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]; }); }
  function lines(s) { return esc(s).replace(/\n/g, "<br>"); }
  function icon(name, extra) { return '<i data-lucide="' + name + '"' + (extra ? ' class="' + extra + '"' : "") + "></i>"; }

  // The canonical frame for one cue (Ordo Black fallback Theme). `null` renders black.
  function canvas(c) {
    if (!c || !projectable(c)) return '<div class="o-canvas" data-t="black"></div>';
    var h = c.h ? '<div class="h">' + esc(c.h) + "</div>" : "";
    var f = c.f ? '<div class="f">' + esc(c.f) + "</div>" : "";
    if (c.k === "song-id") return '<div class="o-canvas" data-t="song-id"><div class="b"><div class="t">' + esc(c.t) + '</div><div class="s">' + esc(c.s) + "</div></div></div>";
    if (c.k === "celebration") return '<div class="o-canvas" data-t="celebration"><div class="b"><div class="t">' + esc(c.t) + '</div><div class="s">' + esc(c.s || "") + "</div></div>" + f + "</div>";
    var t = c.k === "reading-id" ? "reading-id" : c.k;
    return '<div class="o-canvas" data-t="' + t + '">' + h + '<div class="b"><div>' + lines(c.b) + "</div></div>" + f + "</div>";
  }
  // An operator viewport. kind: "frame" (canonical), "private" (operator-only, black + note
  // outside), "unavailable" (operator-only placeholder that must not look like black output).
  function slide(c, opts) {
    opts = opts || {};
    if (c && c.k === "unavailable") {
      return '<div class="o-slide" data-kind="unavailable"><div class="o-slide-state">' + icon("circle-off") +
        "<div><b>Output unavailable</b><br>" + esc(c.why) + "</div></div></div>";
    }
    return '<div class="o-slide">' + canvas(c) + (opts.dim ? "" : "") + "</div>";
  }
  function statusLabel(state, text, marker) {
    var m = marker === "square" ? "o-marker o-marker--square" : marker === "diamond" ? "o-marker o-marker--diamond" : marker === "ring" ? "o-marker o-marker--ring" : "o-marker";
    return '<span class="o-status" data-state="' + state + '"><span class="' + m + '"></span>' + esc(text) + "</span>";
  }
  function icons() { if (window.lucide) window.lucide.createIcons({ attrs: { "stroke-width": 2 } }); }

  // Shrink a fixed logical window (w x h) to the width of its wrapper, as a screenshot would.
  function fit(wrapper, app, w, h) {
    app.style.width = w + "px"; app.style.height = h + "px";
    function apply() {
      var k = Math.min(1, wrapper.clientWidth / w);
      app.style.transform = "scale(" + k + ")";
      wrapper.style.height = (h * k) + "px";
    }
    apply();
    window.addEventListener("resize", apply);
  }

  // ── Presenter ─────────────────────────────────────────────────────────────────────────────
  // opts: w, h (logical window), scale (1..2), mode ("grid"|"list"), rail ("pinned"|"collapsed"|"auto"),
  //       state: {output: "disabled"|"enabled"}, preview: cue index, program: cue index, live: bool,
  //       interactive: bool, preflight: [{state, text, action}] (shown in the Output Dock).
  function presenter(wrapper, opts) {
    opts = opts || {};
    var W = opts.w || 1920, H = opts.h || 1080;
    var app = document.createElement("div");
    app.className = "o-app o-presenter";
    app.tabIndex = 0;
    app.setAttribute("aria-label", "Ordo Presenter mockup; use the Presenter keys while this has focus");
    wrapper.classList.add("o-fit");
    wrapper.appendChild(app);

    var S = {
      scale: opts.scale || 1, mode: opts.mode || "grid", railPref: opts.rail || "auto", railOpen: false,
      output: (opts.state && opts.state.output) || "disabled",
      preview: opts.preview != null ? opts.preview : 1,
      program: opts.program != null ? opts.program : null,
      live: !!opts.live, frozen: !!opts.frozen, blackout: false, frozenReason: "", blackReason: "",
      collapsed: {}, message: "", preflight: opts.preflight || null
    };

    function layout() {
      var eff = W / S.scale;
      if (eff >= 1500) return "wide";
      if (eff >= 1000) return "compact";
      return "constrained";
    }
    function railPinned() {
      var l = layout();
      if (l === "constrained") return false;            // overlay-only
      if (S.railPref === "pinned") return true;
      if (S.railPref === "collapsed") return false;
      return l === "wide";                               // 1920 pinned, 1366 collapsed
    }
    // Frames keep a 300px floor (scaled with the chrome); drop a column before shrinking below it.
    function maxCols() {
      var s = S.scale, l = layout(), w = W;
      if (l !== "constrained") w -= (railPinned() ? 200 : 28) * s + (l === "wide" ? 400 : 360) * s;
      else w -= 28 * s;
      w -= (32 + 16) * s;                                  // timeline padding and group indent
      var n = Math.floor((w + 12 * s) / ((300 + 12) * s));
      return Math.max(1, Math.min(l === "wide" ? 4 : 3, n));
    }

    // presentation commands (MVP-PLAN §8.1, Decision 227)
    function say(m) { S.message = m; }
    function select(i) {
      i = Math.max(0, Math.min(CUES.length - 1, i));
      S.preview = i; say("");
      var c = CUES[i];
      if (S.output !== "enabled" || !S.live || S.frozen || S.blackout) return;
      S.blackReason = "";
      if (c.k === "unavailable") { S.program = null; S.blackReason = "Cue unavailable"; say("Program is black: " + c.why + ". The next valid Cue goes live as usual."); return; }   // Decision 274
      if (projectable(c)) S.program = i;
      else if (c.k === "trigger" || c.k === "hold") S.program = null;   // black
      // wait: Program unchanged
    }
    function take() {
      if (S.output !== "enabled") { say("Congregation output is disabled. Enable Output first; Space takes Preview live once output is on, and also clears Freeze or Blackout."); return; }
      var c = CUES[S.preview];
      if (c.k === "unavailable") { say("Can't take an unavailable Cue live: " + c.why + "."); return; }
      S.program = projectable(c) ? S.preview : (c.k === "wait" ? S.program : null);
      S.live = true; S.frozen = false; S.blackout = false; S.frozenReason = ""; S.blackReason = ""; say("");
    }
    // F and B toggle; releasing either does exactly what Space does (Decision 273)
    function freeze() {
      if (S.output !== "enabled" || S.blackout) return;
      if (S.frozen) take();
      else { S.frozen = true; S.frozenReason = ""; say(""); }
    }
    function blackout() {
      if (S.output !== "enabled") return;
      if (S.blackout) take();
      else { S.blackout = true; S.frozen = false; say(""); }
    }
    function enable() { S.output = "enabled"; S.program = null; S.live = false; say("Output enabled. Program is black until you take Preview live."); }

    function primary() {
      if (S.output !== "enabled") return '<button class="o-btn o-btn--primary o-btn--lg" data-act="enable">' + icon("monitor-up") + "Enable Output</button>";
      if (!S.live || S.frozen || S.blackout) return '<button class="o-btn o-btn--primary o-btn--lg" data-act="take">' + icon("play") + 'Take Preview Live <span class="o-kbd" style="color:inherit;border-color:currentColor">Space</span></button>';
      return '<div class="pr-linked">' + statusLabel("live", "LIVE — navigation updates Program") + "</div>";
    }
    function programState() {
      if (S.output !== "enabled") return statusLabel("disabled", "Output disabled", "square");
      if (S.blackout) return statusLabel("blackout", "BLACKOUT", "square");
      if (S.frozen) return statusLabel("freeze", S.frozenReason ? "FROZEN · " + S.frozenReason : "FROZEN", "diamond");
      if (S.live && S.blackReason) return statusLabel("blackout", "LIVE · black · " + S.blackReason, "square");
      if (S.live) return statusLabel("live", "LIVE");
      return statusLabel("blackout", "BLACK · not live", "square");
    }
    function programSlide() {
      if (S.output !== "enabled") return '<div class="o-slide"><div class="o-slide-state"><div>No congregation window<br><span style="font-weight:400">Output is disabled</span></div></div></div>';
      return '<div class="o-slide"><div class="o-canvas-host">' + canvas(S.program == null ? null : CUES[S.program]).replace('class="o-canvas"', 'class="o-canvas"' + (S.blackout ? ' data-dim="true"' : "")) + "</div></div>";
    }
    function previewSlide() {
      var c = CUES[S.preview];
      return slide(c);
    }
    function privateContext() {
      var c = CUES[S.preview];
      if (c.k === "trigger") return '<div class="pr-private"><span class="pr-who">' + esc(c.who) + "</span><div><b>" + esc(c.b) + '</b><div class="o-meta">Operator-only trigger · not projected. Next: ' + esc(cueLabel(CUES[c.i + 1] || c)) + "</div></div></div>";
      if (c.k === "wait") return '<div class="pr-private">' + icon("hourglass") + "<div><b>" + esc(c.b) + '</b><div class="o-meta">Wait · Program stays as it is</div></div></div>';
      if (c.k === "hold") return '<div class="pr-private">' + icon("square") + "<div><b>Hold · " + esc(c.b) + '</b><div class="o-meta">Black checkpoint, not Blackout</div></div></div>';
      if (c.k === "unavailable") return '<div class="pr-private" data-state="unavailable">' + icon("circle-off") + "<div><b>Unavailable</b><div>" + esc(c.why) + '</div><div class="o-meta">Inspectable, never taken live</div></div></div>';
      return '<div class="pr-private o-meta">' + esc(c.item.title) + " · Cue " + (c.ci + 1) + " of " + c.item.cues.length + " · " + KIND_LABEL[c.k] + "</div>";
    }
    function outputStatus() {
      if (S.output !== "enabled") return '<div class="pr-outstat">' + statusLabel("disabled", "Congregation output disabled", "square") + '<span class="o-meta">Display 2 · Epson EB-X51 · 1920×1080 ready</span></div>';
      return '<div class="pr-outstat">' + statusLabel("live", "Output enabled") + '<span class="o-meta">Display 2 · Epson EB-X51 · 1920×1080</span></div>';
    }

    function cueTile(c) {
      var cur = c.i === S.preview ? ' aria-current="true"' : "";
      // Program (live or held) and Preview each get a ring and a word; linked Live shows both on one Cue
      var air = c.i === S.program && S.output === "enabled" && !S.blackout ? (S.frozen ? "held" : "live") : "";
      var onAir = (air || cur) ? '<span class="pr-onair">' + (air ? statusLabel(air === "held" ? "freeze" : "live", air === "held" ? "Held" : "Program", air === "held" ? "diamond" : null) : "") +
        (cur ? statusLabel("preview", "Preview", "ring") : "") + "</span>" : "";
      var airAttr = air ? ' data-onair="' + air + '"' : "";
      if (S.mode === "grid" && (projectable(c) || c.k === "unavailable")) {
        return '<button class="pr-tile" data-cue="' + c.i + '"' + cur + '><div class="o-slide-wrap"' + cur + airAttr + ">" + slide(c) + "</div>" +
          '<div class="o-caption"><span class="o-n">' + c.item.n + "." + (c.ci + 1) + '</span><span class="o-t">' + esc(KIND_LABEL[c.k] === "Unavailable" ? "Unavailable · " + c.b : cueLabel(c)) + "</span>" + onAir + "</div></button>";
      }
      var lead = c.k === "trigger" ? '<span class="pr-who">' + esc(c.who) + "</span>" :
        c.k === "wait" ? icon("hourglass") : c.k === "hold" ? icon("square") :
        c.k === "unavailable" ? icon("circle-off") : '<span class="pr-mini">' + slide(c) + "</span>";
      var kind = c.k === "unavailable" ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : '<span class="o-meta">' + KIND_LABEL[c.k] + "</span>";
      return '<button class="o-row pr-row" data-cue="' + c.i + '"' + (cur ? ' aria-selected="true"' : "") + airAttr + '><span class="o-n o-meta">' + c.item.n + "." + (c.ci + 1) + "</span>" + lead +
        '<span class="o-grow o-ellipsis">' + esc(cueLabel(c)) + "</span>" + kind + onAir + "</button>";
    }
    function preflight() {
      if (!S.preflight) return "";
      return '<section class="pr-preflight" aria-label="Preflight"><div class="pr-pf-head"><b>Preflight</b><span class="o-grow"></span>' +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-act="close-pf" aria-label="Dismiss preflight" title="Dismiss preflight">' + icon("x") + "</button></div>" +
        S.preflight.map(function (p) {
          return '<div class="pr-pf-row">' + statusLabel(p.state, p.text, p.state === "live" ? null : p.state === "warning" ? "diamond" : "square") +
            (p.action ? '<button class="o-btn">' + esc(p.action) + "</button>" : "") + "</div>";
        }).join("") + "</section>";
    }
    function timeline() {
      var html = preflight();
      ITEMS.forEach(function (it) {
        var open = !S.collapsed[it.n];
        var bad = it.cues.some(function (c) { return c.k === "unavailable"; });
        html += '<section class="pr-group" data-item="' + it.n + '"><button class="pr-group-head" data-toggle="' + it.n + '" aria-expanded="' + open + '">' + icon(open ? "chevron-down" : "chevron-right") +
          '<span class="o-n o-meta">' + it.n + "</span><b>" + esc(it.title) + "</b>" + (it.song ? '<span class="o-meta">' + esc(it.song) + "</span>" : "") +
          '<span class="o-grow"></span>' + (bad ? '<span class="o-status" data-state="unavailable">' + icon("circle-off") + "2 unavailable</span>" : "") +
          '<span class="o-meta">' + it.cues.length + (it.cues.length === 1 ? " Cue" : " Cues") + "</span></button>";
        if (open) {
          html += '<div class="pr-cues" data-mode="' + S.mode + '">';
          var run = [];
          function flush() { if (run.length) { html += '<div class="pr-grid" style="grid-template-columns:repeat(' + maxCols() + ',minmax(0,1fr))">' + run.join("") + "</div>"; run = []; } }
          it.cues.forEach(function (c) {
            var t = cueTile(c);
            if (S.mode === "grid" && (projectable(c) || c.k === "unavailable")) run.push(t); else { flush(); html += t; }
          });
          flush();
          html += "</div>";
        }
        html += "</section>";
      });
      return html;
    }
    function rail(overlay) {
      var cur = CUES[S.preview].item.n, html = "";
      MASS.sections.forEach(function (sec) {
        html += '<div class="o-section-label">' + esc(sec.title) + "</div>";
        sec.items.forEach(function (it) {
          html += '<button class="o-row pr-rail-row" data-jump="' + it.n + '"' + (it.n === cur ? ' aria-current="true"' : "") + '><span class="o-n o-meta">' + it.n + '</span><span class="o-ellipsis">' + esc(it.title) + "</span></button>";
        });
      });
      var head = '<div class="o-pane-head"><b>Ordo</b><span class="o-grow"></span>' +
        (layout() === "constrained" ? '<span class="o-meta">Overlay only at this size</span>' :
          '<button class="o-btn o-btn--quiet o-btn--icon" data-act="pin" title="' + (railPinned() ? "Unpin Ordo" : "Pin Ordo") + '" aria-label="' + (railPinned() ? "Unpin Ordo" : "Pin Ordo") + '">' + icon(railPinned() ? "pin-off" : "pin") + "</button>") +
        (overlay ? '<button class="o-btn o-btn--quiet o-btn--icon" data-act="close-rail" title="Close" aria-label="Close">' + icon("x") + "</button>" : "") + "</div>";
      return '<aside class="o-pane pr-rail' + (overlay ? " pr-rail--overlay" : "") + '" aria-label="Ordo">' + head + '<div class="o-pane-body">' + html + '</div><div class="pr-rail-foot o-meta"><span class="o-kbd">O</span> open Ordo</div></aside>';
    }
    function dock() {
      var l = layout();
      var monitors = '<div class="pr-monitor"><div class="pr-mon-head"><b>Program</b>' + programState() + "</div>" + programSlide() + "</div>" +
        '<div class="pr-monitor"><div class="pr-mon-head"><b>Preview</b><span class="o-meta">' + esc(CUES[S.preview].item.title) + "</span></div>" + previewSlide() + privateContext() + "</div>";
      var controls = outputStatus() +
        '<div class="pr-hs"><button class="o-btn o-btn--lg" data-act="freeze"' + (S.frozen ? ' aria-pressed="true"' : "") + (S.output !== "enabled" ? " disabled" : "") + ">" + icon("snowflake") + (S.frozen ? "Unfreeze" : "Freeze") + ' <span class="o-kbd">F</span></button>' +
        '<button class="o-btn o-btn--lg" data-act="blackout"' + (S.blackout ? ' aria-pressed="true"' : "") + (S.output !== "enabled" ? " disabled" : "") + ">" + icon("square") + (S.blackout ? "End Blackout" : "Blackout") + ' <span class="o-kbd">B</span></button></div>' +
        '<div class="pr-primary">' + primary() + "</div>" +
        (S.message ? '<div class="pr-msg" role="status">' + esc(S.message) + "</div>" : "");
      if (l === "constrained") return '<div class="pr-band">' + monitors + '</div><div class="pr-band-controls">' + controls + "</div>";
      return '<aside class="pr-dock" aria-label="Output controls">' + monitors + controls + "</aside>";
    }

    function render() {
      var l = layout(), pinned = railPinned();
      app.style.setProperty("--o-scale", S.scale);
      app.style.setProperty("--pr-band-mon", Math.floor(H * 0.3 * 16 / 9) + "px");
      app.dataset.layout = l;
      var header = '<header class="pr-header"><div class="pr-session"><b>' + esc(MASS.title) + '</b><span class="o-meta">' + esc(MASS.date) + "</span></div>" +
        '<span class="o-grow"></span><span class="o-meta pr-clock">9:14</span>' +
        (S.output === "enabled" ? statusLabel("live", "Display 2 connected") : statusLabel("disabled", "Display 2 ready · not enabled", "square")) +
        '<div class="o-segmented" role="group" aria-label="Timeline view"><button data-view="grid" aria-pressed="' + (S.mode === "grid") + '">' + icon("layout-grid") + (l === "constrained" ? "" : "Grid") + '</button><button data-view="list" aria-pressed="' + (S.mode === "list") + '">' + icon("list") + (l === "constrained" ? "" : "List") + "</button></div>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" title="Keyboard shortcuts" aria-label="Keyboard shortcuts">' + icon("keyboard") + "</button>" +
        '<button class="o-btn">' + icon("log-out") + "Exit Presenter</button></header>";
      var tab = pinned ? "" : '<button class="pr-ordo-tab" data-act="open-rail" aria-label="Open Ordo">ORDO</button>';
      var main = '<main class="pr-timeline" aria-label="Cue timeline">' + timeline() + "</main>";
      var body;
      if (l === "constrained") body = tab + '<div class="pr-col">' + dock() + main + "</div>";
      else body = (pinned ? rail(false) : tab) + main + dock();
      var overlay = S.railOpen && !pinned ? '<div class="o-scrim" data-act="close-rail"></div>' + rail(true) : "";
      var old = app.querySelector(".pr-timeline"), kept = old ? old.scrollTop : 0;
      app.innerHTML = header + '<div class="o-body pr-body">' + body + "</div>" + overlay;
      icons();
      // Keep the active Cue in view by scrolling the timeline only, never the page.
      var tl = app.querySelector(".pr-timeline"), cur = tl && tl.querySelector('[data-cue="' + S.preview + '"]');
      if (tl) tl.scrollTop = kept;                     // F, B, Space and clicks never move the timeline
      if (cur && S.follow) {
        // Keep Preview in view, and the Program Cue too when both fit (Freeze or Blackout lets them differ).
        var k = tl.getBoundingClientRect().height / tl.clientHeight || 1, pad = 56;
        function span(el) { var t = (el.getBoundingClientRect().top - tl.getBoundingClientRect().top) / k + tl.scrollTop; return [t, t + el.offsetHeight]; }
        var want = span(cur), prog = S.program != null && S.program !== S.preview && tl.querySelector('[data-cue="' + S.program + '"]');
        if (prog) { var ps = span(prog); if (Math.max(want[1], ps[1]) - Math.min(want[0], ps[0]) + pad <= tl.clientHeight) want = [Math.min(want[0], ps[0]), Math.max(want[1], ps[1])]; }
        if (want[0] < tl.scrollTop + pad || want[1] > tl.scrollTop + tl.clientHeight) tl.scrollTop = Math.max(0, want[0] - pad);
      }
      S.follow = false;
    }

    function step(d) { S.follow = true; select(S.preview + d); render(); }
    function jumpItem(d) {
      var n = CUES[S.preview].item.n + d; var it = ITEMS[n - 1];
      if (it) { S.follow = true; select(it.first); render(); }
    }

    if (opts.interactive !== false) {
      app.addEventListener("click", function (e) {
        var t = e.target.closest("[data-cue],[data-act],[data-view],[data-toggle],[data-jump]");
        if (!t) return;
        if (t.dataset.cue) { select(+t.dataset.cue); }
        else if (t.dataset.view) S.mode = t.dataset.view;
        else if (t.dataset.toggle) S.collapsed[t.dataset.toggle] = !S.collapsed[t.dataset.toggle];
        else if (t.dataset.jump) {
          S.railOpen = false; render();
          var g = app.querySelector('.pr-group[data-item="' + t.dataset.jump + '"]');
          if (g) g.scrollIntoView({ block: "start" });   // scrolls, never filters, never selects
          app.focus(); return;
        } else {
          var a = t.dataset.act;
          if (a === "enable") enable(); else if (a === "take") take(); else if (a === "freeze") freeze();
          else if (a === "blackout") blackout(); else if (a === "open-rail") S.railOpen = true;
          else if (a === "close-rail") S.railOpen = false;
          else if (a === "close-pf") S.preflight = null;
          else if (a === "pin") { S.railPref = railPinned() ? "collapsed" : "pinned"; S.railOpen = false; }
        }
        render(); app.focus({ preventScroll: true });
      });
      app.addEventListener("keydown", function (e) {
        var k = e.key, used = true;
        if (k === "ArrowRight" || k === "ArrowDown") step(1);
        else if (k === "ArrowLeft" || k === "ArrowUp") step(-1);
        else if (k === "PageDown") jumpItem(1);
        else if (k === "PageUp") jumpItem(-1);
        else if (k === "Home") { S.follow = true; select(0); render(); }
        else if (k === "End") { S.follow = true; select(CUES.length - 1); render(); }
        else if (k === " ") { take(); render(); }
        else if (k === "f" || k === "F") { freeze(); render(); }
        else if (k === "b" || k === "B") { blackout(); render(); }
        else if (k === "o" || k === "O") { if (!railPinned()) S.railOpen = true; render(); }
        else if (k === "Escape") { S.railOpen = false; render(); }
        else used = false;
        if (used) e.preventDefault();
      });
    }

    fit(wrapper, app, W, H);
    S.follow = true;
    render();
    return {
      setScale: function (s) { S.scale = s; render(); },
      setMode: function (m) { S.mode = m; render(); },
      state: S, render: render, app: app
    };
  }

  // ── Preparation shell and Mass Planning (Decisions 170-178, 250, 257) ───────────────────────
  var NAV = [["Masses", "calendar-days"], ["Library", "library"], ["Themes", "palette"], ["Transfer & Backup", "hard-drive-download"]];
  var NAV_FOOT = [["Diagnostics", "activity"], ["Help", "circle-help"], ["Settings", "settings"]];
  function navrail(active) {
    function a(n) { return '<a href="#" aria-current="' + (n[0] === active ? "page" : "false") + '">' + icon(n[1], "o-icon-20") + "<span>" + esc(n[0]) + "</span></a>"; }
    return '<nav class="o-navrail" aria-label="Global navigation">' + NAV.map(a).join("") + '<span class="o-spacer"></span>' + NAV_FOOT.map(a).join("") + "</nav>";
  }
  function unavailableCount(it) { return it.cues.filter(function (c) { return c.k === "unavailable"; }).length; }

  // opts: w, h, item (selected item number), cue (selected cue index within item), scale
  // Wide (effective width >= 1700) spends space on rows, metadata and thumbnails; pane ownership never changes (Decision 240).
  function planner(wrapper, opts) {
    opts = opts || {};
    var W = opts.w || 1366, H = opts.h || 768;
    var app = document.createElement("div");
    app.className = "o-app";
    wrapper.classList.add("o-fit");
    wrapper.appendChild(app);
    var S = { item: opts.item || 2, cue: opts.cue || 0, scale: opts.scale || 1 };

    function songEditor(it, wide) {
      var arr = '<div class="pl-card"><div class="o-grow"><div class="o-label">Using ' + esc(MASS.choir) + " arrangement</div>" +
        '<div class="o-meta">' + esc(MASS.language) + " · linked to Song Library" + (wide ? " · last reviewed 17 Okt 2026" : "") + "</div></div>" +
        '<button class="o-btn">' + icon("replace") + (wide ? "Change Song or Arrangement" : "Change") + "</button>" +
        '<button class="o-btn">' + icon("pencil") + "Customize for this Mass</button></div>";
      var seq = it.cues.map(function (q, i) {
        var name = q.k === "song-id" ? "Song title" : q.k === "unavailable" ? q.b : q.h;
        return '<div class="o-row pl-seq"><span class="o-meta">' + (i + 1) + '</span><span class="o-grow">' + esc(name) + "</span>" +
          (q.k === "unavailable" ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : '<span class="o-meta">1 Cue</span>') + "</div>";
      }).join("");
      var thumbs = it.cues.map(function (q, i) {
        var cur = i === S.cue ? ' aria-current="true"' : "";
        return '<button class="pr-tile pl-thumb" data-cue="' + i + '"' + cur + '><div class="o-slide-wrap"' + cur + ">" + slide(q) + "</div>" +
          '<div class="o-caption"><span class="o-n">' + (i + 1) + '</span><span class="o-t">' + esc(q.k === "unavailable" ? "Unavailable" : cueLabel(q)) + "</span></div></button>";
      }).join("");
      return '<div class="pl-ed-body">' + arr +
        '<div class="pl-block"><div class="pl-block-head"><b>Sequence</b><span class="o-meta">' + it.cues.length + " sections · " + it.cues.length + (wide ? " generated Cues" : " Cues") + "</span></div>" + seq + "</div>" +
        '<div class="pl-block"><div class="pl-block-head"><b>Generated Cues</b><span class="o-meta">Selected ' + (S.cue + 1) + " of " + it.cues.length + (wide ? " · click a Cue to inspect" : "") + "</span></div>" +
        '<div class="pl-thumbs">' + thumbs + "</div></div>" +
        (wide ? '<div class="pl-block"><div class="pl-block-head"><b>Mass-local state</b></div><div class="o-meta">Following the library Arrangement; no changes for this Mass.</div></div>' : "") +
        "</div>";
    }
    function cueEditor(it) {
      return '<div class="pl-ed-body"><div class="pl-block"><div class="pl-block-head"><b>' + esc(it.type) + '</b><span class="o-meta">' + it.cues.length + " Cues</span></div>" +
        it.cues.map(function (q, i) {
          var lead = q.k === "trigger" ? '<span class="pr-who">' + esc(q.who) + "</span>" : q.k === "unavailable" ? icon("circle-off") :
            q.k === "wait" ? icon("hourglass") : q.k === "hold" ? icon("square") : icon("monitor");
          var tag = q.k === "trigger" ? '<span class="o-meta">Operator only</span>' : q.k === "wait" || q.k === "hold" ? '<span class="o-meta">' + KIND_LABEL[q.k] + " · operator only</span>" :
            q.k === "unavailable" ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : '<span class="o-meta">Output</span>';
          var sub = q.k === "trigger" ? (q.who === "P" ? "Priest" : "Lector") + " · private timing prompt" : q.k === "response" ? "Congregation response" : KIND_LABEL[q.k];
          return '<button class="o-row pl-cue" data-cue="' + i + '"' + (i === S.cue ? ' aria-selected="true"' : "") + '><span class="o-meta">' + (i + 1) + "</span>" + lead +
            '<span class="o-grow"><span class="o-ellipsis pl-cue-t">' + esc(cueLabel(q)) + '</span><span class="o-meta">' + esc(sub) + "</span></span>" + tag + "</button>";
        }).join("") + "</div></div>";
    }
    function inspector(it, c, wide) {
      var n = it.cues.length, first = S.cue === 0, last = S.cue === n - 1;
      var frame = c.k === "trigger" || c.k === "hold" || c.k === "wait" ? slide(null) : slide(c);
      var ctx = "";
      if (c.k === "trigger") ctx = '<div class="pr-private"><span class="pr-who">' + esc(c.who) + "</span><div><b>" + (c.who === "P" ? "Priest" : "Lector") + ": " + esc(c.b) + '</b><div class="o-meta">Advance when this line is finished.</div></div></div>';
      else if (c.k === "wait" || c.k === "hold") ctx = '<div class="pr-private">' + icon(c.k === "wait" ? "hourglass" : "square") + "<div><b>" + KIND_LABEL[c.k] + " · " + esc(c.b) + '</b><div class="o-meta">' + (c.k === "wait" ? "Program stays as it is" : "Black checkpoint, not Blackout") + "</div></div></div>";
      else if (c.k === "unavailable") ctx = '<div class="pr-private" data-state="unavailable">' + icon("circle-off") + "<div><b>Unavailable</b><div>" + esc(c.why) + "</div></div></div>";
      var nextItem = ITEMS[it.n], prevItem = ITEMS[it.n - 2];
      var edge = last ? "End of " + it.title + "." + (nextItem ? " Select " + nextItem.title + " in the Mass outline to inspect it." : "") :
        first && prevItem ? "Start of " + it.title + "." : "";
      var opOnly = c.k === "trigger" || c.k === "wait" || c.k === "hold";
      var meta = [["Congregation frame", opOnly ? "Black" : c.k === "unavailable" ? "Unavailable" : "Canonical 16:9"], ["Theme", "Ordo Black"],
        ["Pagination", c.k === "unavailable" ? "—" : "Fits safely"], ["Output", "1920 × 1080"]];
      if (wide) meta.push(["Safe area", "Within 120 / 80 px"], ["Source", it.song ? "Library Arrangement" : "Mass item"]);
      return '<aside class="o-pane pl-inspector" aria-label="Inspector"><div class="o-pane-head"><b>Inspector</b>' + (opOnly ? '<span class="o-meta">Operator only</span>' : "") + '<span class="o-grow"></span>' +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-step="-1" aria-label="Previous Cue" title="Previous Cue"' + (first ? " disabled" : "") + ">" + icon("chevron-left") + "</button>" +
        '<span class="o-meta">Cue ' + (S.cue + 1) + " of " + n + "</span>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-step="1" aria-label="Next Cue" title="Next Cue"' + (last ? " disabled" : "") + ">" + icon("chevron-right") + "</button></div>" +
        '<div class="o-pane-body pl-insp-body">' + frame + ctx +
        '<dl class="pl-meta">' + meta.map(function (m) { return "<dt>" + m[0] + "</dt><dd>" + m[1] + "</dd>"; }).join("") + "</dl>" +
        '<div class="pl-scope"><span class="o-meta">Navigation scope</span><b>This item only</b>' + (edge ? '<div class="o-meta">' + esc(edge) + "</div>" : "") + "</div>" +
        "</div></aside>";
    }

    function render() {
      app.style.setProperty("--o-scale", S.scale);
      var wide = W / S.scale >= 1700;
      app.dataset.layout = wide ? "wide" : "compact";
      var it = ITEMS[S.item - 1], c = it.cues[S.cue];
      var incomplete = ITEMS.filter(unavailableCount).length;
      var outline = "";
      MASS.sections.forEach(function (sec) {
        outline += '<div class="o-section-label pl-sec">' + icon("chevron-down") + esc(sec.title) + "</div>";
        sec.items.forEach(function (x) {
          var bad = unavailableCount(x);
          outline += '<button class="o-row pl-item" data-item="' + x.n + '"' + (x.n === S.item ? ' aria-selected="true"' : "") + '><span class="o-meta">' + x.n + '</span><span class="o-grow o-ellipsis">' + esc(x.title) +
            (x.song ? ' <span class="o-muted">· ' + esc(x.song) + "</span>" : "") + "</span>" +
            (bad ? '<span class="o-status" data-state="warning" title="Incomplete" aria-label="Incomplete">' + icon("triangle-alert") + "</span>" : "") + "</button>";
        });
      });
      var bad = unavailableCount(it);
      // Redrawing never moves the outline; the editor and Inspector keep their place until the item changes.
      var keep = {};
      [".pl-outline", ".pl-editor", ".pl-inspector"].forEach(function (q) { var el = app.querySelector(q + " .o-pane-body"); if (el) keep[q] = el.scrollTop; });
      var sameItem = S.drawnItem === S.item; S.drawnItem = S.item;
      app.innerHTML =
        '<div class="o-body">' + navrail("Masses") +
        '<div class="o-grow pl-ws">' +
          '<header class="pl-head"><div class="o-grow"><div class="o-meta">Masses / ' + esc(MASS.title) + '</div><h1 class="o-title pl-title">' + esc(MASS.title) + "</h1>" +
          '<div class="o-meta">' + esc(MASS.date) + " · " + esc(MASS.language) + " · " + esc(MASS.choir) + (wide ? " · Parokya ni San Roque · Fr. Mateo Reyes" : "") + "</div></div>" +
          '<span class="o-meta pl-saved">' + icon("check") + (wide ? "Saved just now" : "Saved") + "</span>" +
          '<button class="o-btn pl-ready">' + (incomplete ? statusLabel("warning", incomplete + " incomplete", "diamond") : statusLabel("live", "Ready")) + "</button>" +
          '<button class="o-btn">' + icon("palette") + "Theme: Ordo Black</button>" +
          '<button class="o-btn o-btn--primary">' + icon("presentation") + "Present</button></header>" +
          '<div class="pl-panes">' +
            '<section class="o-pane pl-outline" aria-label="Mass outline"><div class="o-pane-head"><b>Mass outline</b><span class="o-grow"></span>' +
              (wide ? '<button class="o-btn">' + icon("plus") + "Add Mass item</button>" : '<button class="o-btn o-btn--icon" aria-label="Add Mass item" title="Add Mass item">' + icon("plus") + "</button>") + "</div>" +
              '<div class="o-pane-body o-rows--open">' + outline + "</div></section>" +
            '<section class="o-pane pl-editor" aria-label="Selected Mass item"><div class="o-pane-head"><div class="o-grow"><div class="o-meta">' + esc(MASS.sections[it.section].title) + " · " + esc(it.type) + "</div><b>" + it.n + ". " + esc(it.title) + "</b>" +
              (it.song ? ' <span class="o-muted">' + esc(it.song) + "</span>" : "") + "</div></div>" +
              (bad ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div><b>" + bad + " Cues unavailable.</b> " + esc(it.cues[0].why) + '. <a href="#">Choose another Arrangement</a></div></div>' : "") +
              '<div class="o-pane-body">' + (it.song ? songEditor(it, wide) : cueEditor(it)) + "</div></section>" +
            inspector(it, c, wide) +
          "</div>" +
        "</div></div>";
      icons();
      Object.keys(keep).forEach(function (q) {
        var el = app.querySelector(q + " .o-pane-body");
        if (el && (q === ".pl-outline" || sameItem)) el.scrollTop = keep[q];
      });
    }
    app.addEventListener("click", function (e) {
      var t = e.target.closest("[data-item],[data-cue],[data-step]"); if (!t || t.disabled) return;
      e.preventDefault();
      if (t.dataset.item) { S.item = +t.dataset.item; S.cue = 0; }
      else if (t.dataset.cue) S.cue = +t.dataset.cue;
      else {                                              // stops at the item's first and last Cue (Decision 169)
        var n = ITEMS[S.item - 1].cues.length;
        S.cue = Math.max(0, Math.min(n - 1, S.cue + +t.dataset.step));
      }
      render();
    });
    fit(wrapper, app, W, H);
    render();
    return { app: app, state: S, render: render };
  }

  window.OrdoKit = {
    MASS: MASS, ITEMS: ITEMS, CUES: CUES, projectable: projectable, cueLabel: cueLabel, KIND_LABEL: KIND_LABEL,
    esc: esc, icon: icon, canvas: canvas, slide: slide, statusLabel: statusLabel, icons: icons, fit: fit, presenter: presenter,
    planner: planner, navrail: navrail
  };
  document.addEventListener("DOMContentLoaded", icons);
})();
