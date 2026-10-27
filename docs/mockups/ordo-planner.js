// Ordo mockups: the Preparation Mass planner on the shared kit (load after ordo-kit.js).
// Each planner owns a working copy of the sample Mass, so its flows can add, replace and customize
// items and Undo them without touching other mockups on the page. Mockup behaviour only.
(function () {
  "use strict";
  var K = window.OrdoKit, esc = K.esc, icon = K.icon, statusLabel = K.statusLabel, cueLabel = K.cueLabel, KIND = K.KIND_LABEL;
  KIND.blackout = "Blackout";

  // ── Themes (Decision 95): canonical colours are Theme content, not operator chrome ───────────
  var THEMES = [
    { id: "ordo-black", name: "Ordo Black", note: "White on black. The protected fallback Theme." },
    { id: "parish-light", name: "Parish Light", note: "Dark text on warm paper, for a bright daytime nave." },
    { id: "ordinary-green", name: "Ordinary Time", note: "Cream text on deep green." },
    { id: "high-contrast", name: "High Contrast", note: "Yellow on black, for a long projector throw." }
  ];
  function themeName(id) { for (var i = 0; i < THEMES.length; i++) if (THEMES[i].id === id) return THEMES[i].name; return id; }
  function tslide(c, th) { return K.slide(c).replace('class="o-slide"', 'class="o-slide" data-th="' + th + '"'); }

  // ── Song Library fixture ──────────────────────────────────────────────────────────────────
  function strip(c) { var o = {}; for (var k in c) if (k !== "item" && k !== "i" && k !== "ci") o[k] = c[k]; return o; }
  function fromMass(title) { var r = null; K.MASS.sections.forEach(function (s) { s.items.forEach(function (it) { if (it.song === title) r = it.cues; }); }); return r; }
  function L(h, b) { return { k: "lyrics", h: h, b: b }; }
  var CHOIR = K.MASS.choir;
  var LIB = [
    { title: "Daygon Ta", by: "R. Villanueva", arrs: [
      { name: CHOIR, cues: fromMass("Daygon Ta") },
      { name: "Youth Choir", cues: [{ k: "song-id", t: "Daygon Ta", s: "R. Villanueva · Youth Choir" }, L("Bersikulo 1", "Daygon ta ang Ginoo\nsa tanang kasingkasing"), L("Koro", "Daygon, daygon ta\nang ngalan sa Ginoo")] }] },
    { title: "Ihalad Namo", by: "J. Bacalso", arrs: [
      { name: CHOIR + " · scanned score", cues: fromMass("Ihalad Namo"), broken: "Arrangement image asset is missing from the snapshot" },
      { name: CHOIR, cues: [L("Bersikulo 1", "Ihalad namo kining pan ug bino\nsa imong halaran"), L("Koro", "Dawata, O Ginoo,\nang among halad"),
        L("Bersikulo 2", "Ihalad usab namo\nang among kinabuhi"), L("Koro", "Dawata, O Ginoo,\nang among halad")] },
      { name: "Youth Choir", cues: [L("Koro", "Dawata, O Ginoo,\nang among halad")] }] },
    { title: "Halad sa Kasingkasing", by: "M. Dela Cruz", arrs: [
      { name: CHOIR, cues: [L("Bersikulo 1", "Kining kasingkasing\namo nang ihalad"), L("Koro", "Dawata kami, Ginoo,\nsa imong gugma")] }] },
    { title: "Ginoo, Kaloy-i Kami", by: "Misa Bisaya", arrs: [{ name: CHOIR, cues: fromMass("Ginoo, Kaloy-i Kami") }] },
    { title: "Himaya sa Dios", by: "Misa Bisaya", arrs: [{ name: CHOIR, cues: fromMass("Himaya sa Dios") }] },
    { title: "Aleluya", by: "Misa Bisaya", arrs: [{ name: CHOIR, cues: fromMass("Aleluya") }] },
    { title: "Santos", by: "Misa Bisaya", arrs: [{ name: CHOIR, cues: fromMass("Santos") }] },
    { title: "Pan sa Kinabuhi", by: "Koro San Roque", arrs: [{ name: CHOIR, cues: fromMass("Pan sa Kinabuhi") }] },
    { title: "Padayon", by: "Koro San Roque", arrs: [{ name: CHOIR, cues: fromMass("Padayon") }] },
    { title: "Ang Ginoo Akong Magbalantay", by: "Salmo 23", arrs: [{ name: CHOIR, cues: [L("Koro", "Ang Ginoo akong magbalantay,\ndili ako kulangon")] }] }
  ];
  function libIndex(title) { for (var i = 0; i < LIB.length; i++) if (LIB[i].title === title) return i; return null; }

  // ── the working copy of the Mass ──────────────────────────────────────────────────────────
  function model() {
    return K.MASS.sections.map(function (sec) {
      return { title: sec.title, items: sec.items.map(function (it) {
        return { title: it.title, type: it.type, song: it.song || null, cues: it.cues.map(strip),
          arr: it.song ? (it.song === "Ihalad Namo" ? CHOIR + " · scanned score" : CHOIR) : null,
          local: false, base: null, changes: [], empty: false,
          theme: it.title === "Kyrie" ? "high-contrast" : null,
          warn: it.song === "Pan sa Kinabuhi" ? "Pagination not reviewed since the lyrics changed" : null };
      }) };
    });
  }

  var NEW_ITEM = {
    "Song": { title: "New Song", icon: "music", note: "Choose a Song and Arrangement" },
    "Reading": { title: "New Reading", icon: "book-open", note: "Scripture reference, prompt and response" },
    "Response or Prayer": { title: "New Response", icon: "messages-square", note: "Role-tagged prompts and responses" },
    "General Presentation": { title: "New General Presentation", icon: "presentation", note: "Text or image Slides" },
    "Hold": { title: "Hold", icon: "square", note: "A black operator checkpoint" },
    "Blackout": { title: "Blackout", icon: "moon", note: "Enters the Blackout state" },
    "Custom sequence": { title: "New Custom sequence", icon: "list-plus", note: "Mix Cue types for an unusual moment" }
  };

  // opts: w, h, item (number), cue, scale, afterRender(api)
  function planner(wrapper, opts) {
    opts = opts || {};
    var W = opts.w || 1366, H = opts.h || 768;
    var app = document.createElement("div");
    app.className = "o-app";
    app.tabIndex = -1;
    wrapper.classList.add("o-fit");
    wrapper.appendChild(app);

    var S = { sec: model(), cue: opts.cue || 0, scale: opts.scale || 1, theme: "ordo-black",
      overlay: null, child: null, pick: null, stagedTheme: null, keepOverride: {}, dest: null, toast: null, undo: null, focus: null, fresh: false };
    function items() { var out = []; S.sec.forEach(function (sec, si) { sec.items.forEach(function (it, ai) { it.section = si; it.at = ai; it.n = out.length + 1; out.push(it); }); }); return out; }
    S.sel = items()[(opts.item || 2) - 1];

    function bad(it) { return it.cues.filter(function (c) { return c.k === "unavailable"; }).length; }
    function findings() {
      var inc = [], warn = [];
      items().forEach(function (it) {
        if (it.type === "Song" && !it.song) inc.push({ it: it, title: "No Song chosen", text: "Nothing can be presented for this item until a Song and Arrangement are chosen.", action: "Choose Song" });
        else if (bad(it)) inc.push({ it: it, title: bad(it) + " Cues unavailable", text: it.cues[0].why + ", so these Cues can't produce a frame.", action: "Choose another Arrangement" });
        else if (it.empty) inc.push({ it: it, title: "No content yet", text: "This " + it.type + " has no Cues, so nothing is presented for it.", action: "Add content" });
        if (it.warn) warn.push({ it: it, title: "Pagination not reviewed", text: it.warn + ".", action: "Review pagination" });
      });
      return { inc: inc, warn: warn };
    }
    function readinessLabel(f) {
      var parts = [];
      if (f.inc.length) parts.push(f.inc.length + " incomplete");
      if (f.warn.length) parts.push(f.warn.length + (f.warn.length === 1 ? " warning" : " warnings"));
      if (!parts.length) return statusLabel("live", "Ready");
      return statusLabel(f.inc.length ? "warning" : "info", parts.join(" · "), f.inc.length ? "diamond" : null);
    }

    // ── undo: one level, enough to show the toast's promise ─────────────────────────────────
    function snapshot() { return { sec: JSON.parse(JSON.stringify(S.sec)), n: S.sel.n, cue: S.cue, theme: S.theme }; }
    function done(text, snap) { S.undo = snap; S.toast = text; }
    function undo() {
      if (!S.undo) return;
      S.sec = S.undo.sec; S.theme = S.undo.theme; S.sel = items()[Math.min(S.undo.n, items().length) - 1]; S.cue = S.undo.cue;
      S.undo = null; S.toast = null; S.overlay = null; S.child = null;
    }

    // ── commands ────────────────────────────────────────────────────────────────────────────
    function select(n) { S.sel = items()[n - 1]; S.cue = 0; }
    function openAdd() {
      S.overlay = "add"; S.child = null;
      S.dest = { sec: S.sel.section, at: S.sel.at };
    }
    function destLabel(d) {
      var sec = S.sec[d.sec];
      return d.at < 0 ? "Start of " + sec.title : "After " + sec.items[d.at].title + " · " + sec.title;
    }
    function addItem(type) {
      var snap = snapshot(), spec = NEW_ITEM[type], d = S.dest || { sec: S.sel.section, at: S.sel.at };
      var it = { title: spec.title, type: type === "Response or Prayer" ? "Response sequence" : type, song: null, arr: null, local: false, base: null, changes: [], theme: null, warn: null,
        cues: type === "Hold" ? [{ k: "hold", b: "Hold" }] : type === "Blackout" ? [{ k: "blackout", b: "Blackout" }] : [],
        empty: type !== "Song" && type !== "Hold" && type !== "Blackout" };
      S.sec[d.sec].items.splice(d.at + 1, 0, it);
      items(); S.sel = it; S.cue = 0;
      done("Added " + it.title + " · " + destLabel(d).replace(/^After /, "after ").replace(/^Start of /, "at the start of ") + ".", snap);
      S.overlay = null;
      if (type === "Song") openPicker();
    }
    function openPicker() {
      S.overlay = "picker"; S.child = null;
      var si = S.sel.song ? libIndex(S.sel.song) : null;
      S.pick = { q: "", song: si, arr: null };
    }
    function stage(song, arr) { S.pick.song = libIndex(song); var a = LIB[S.pick.song].arrs; for (var j = 0; j < a.length; j++) if (a[j].name === arr) S.pick.arr = j; }
    function staged() { return S.pick && S.pick.song != null && S.pick.arr != null ? { song: LIB[S.pick.song], arr: LIB[S.pick.song].arrs[S.pick.arr] } : null; }
    function isCurrent(song, arr) { return S.sel.song === song.title && S.sel.arr === arr.name; }
    function commitPick() {
      var st = staged(); if (!st || isCurrent(st.song, st.arr)) return;
      if (S.sel.local) { S.child = "review"; return; }
      applyPick(st);
    }
    function applyPick(st) {
      var snap = snapshot(), it = S.sel;
      it.song = st.song.title; it.arr = st.arr.name; it.cues = st.arr.cues.map(strip);
      it.local = false; it.base = null; it.changes = []; it.empty = false; it.warn = null;
      S.cue = 0; S.overlay = null; S.child = null;
      done(it.title + " now uses " + st.song.title + " · " + st.arr.name + ".", snap);
    }
    function customize() {
      var snap = snapshot(), it = S.sel;
      it.local = true; it.base = it.cues.map(strip); it.changes = [];
      done(it.title + " is now a Mass-local override.", snap);
    }
    function change(text) { if (S.sel.changes.indexOf(text) < 0) S.sel.changes.push(text); }
    function linked(it, i) { var c = it.cues[i]; return !c.forked && it.cues.filter(function (q) { return !q.forked && q.h === c.h; }).length > 1; }
    function editText(v) {
      var it = S.sel, c = it.cues[S.cue];
      if (linked(it, S.cue)) it.cues.forEach(function (q) { if (!q.forked && q.h === c.h) q.b = v; });
      else c.b = v;
      change("Edited " + c.h + (c.forked ? " (one occurrence)" : ""));
    }
    function returnToLibrary() {
      var snap = snapshot(), it = S.sel;
      it.cues = it.base.map(strip); it.local = false; it.base = null; it.changes = [];
      S.cue = Math.min(S.cue, it.cues.length - 1); S.overlay = null;
      done(it.title + " follows the library Arrangement again.", snap);
    }
    function openTheme() { S.overlay = "theme"; S.child = null; S.stagedTheme = S.theme; S.keepOverride = {}; }
    function overridden() { return items().filter(function (it) { return it.theme; }); }
    function applyTheme() {
      var snap = snapshot(), removed = 0;
      S.theme = S.stagedTheme;
      overridden().forEach(function (it) { if (S.keepOverride[it.n] === false) { it.theme = null; removed++; } });
      S.overlay = null; S.child = null;
      done("Mass Theme is now " + themeName(S.theme) + (removed ? "; " + removed + " override removed." : "."), snap);
    }

    // ── views ───────────────────────────────────────────────────────────────────────────────
    function th(it) { return it.theme || S.theme; }
    function header(wide, f) {
      return '<header class="pl-head"><div class="o-grow"><div class="o-meta">Masses / ' + esc(K.MASS.title) + '</div><h1 class="o-title pl-title">' + esc(K.MASS.title) + "</h1>" +
        '<div class="o-meta">' + esc(K.MASS.date) + " · " + esc(K.MASS.language) + " · " + esc(CHOIR) + (wide ? " · Parokya ni San Roque · Fr. Mateo Reyes" : "") + "</div></div>" +
        '<span class="o-meta pl-saved">' + icon("check") + (wide ? "Saved just now" : "Saved") + "</span>" +
        '<button class="o-btn pl-ready" data-act="readiness"' + (S.overlay === "readiness" ? ' aria-pressed="true"' : "") + ' aria-label="Readiness">' + readinessLabel(f) + "</button>" +
        '<button class="o-btn" data-act="theme">' + icon("palette") + "Theme: " + esc(themeName(S.theme)) + "</button>" +
        '<button class="o-btn o-btn--primary">' + icon("presentation") + "Present</button></header>";
    }
    function outline(wide) {
      var html = "";
      S.sec.forEach(function (sec) {
        html += '<div class="o-section-label pl-sec">' + icon("chevron-down") + esc(sec.title) + "</div>";
        sec.items.forEach(function (x) {
          var flag = (x.type === "Song" && !x.song) || bad(x) || x.empty;
          html += '<button class="o-row pl-item" data-item="' + x.n + '"' + (x === S.sel ? ' aria-selected="true"' : "") + '><span class="o-meta">' + x.n + '</span><span class="o-grow o-ellipsis">' + esc(x.title) +
            (x.song ? ' <span class="o-muted">· ' + esc(x.song) + "</span>" : "") + "</span>" +
            (x.local ? '<span class="o-meta" title="Mass-local override">' + icon("pencil") + "</span>" : "") +
            (flag ? '<span class="o-status" data-state="warning" title="Incomplete" aria-label="Incomplete">' + icon("triangle-alert") + "</span>" : "") + "</button>";
        });
      });
      return '<section class="o-pane pl-outline" aria-label="Mass outline"><div class="o-pane-head"><b>Mass outline</b><span class="o-grow"></span>' +
        '<button class="o-btn' + (wide ? "" : " o-btn--icon") + '" data-act="add" data-anchor="add" aria-haspopup="menu" aria-expanded="' + (S.overlay === "add") + '"' + (wide ? "" : ' aria-label="Add Mass item" title="Add Mass item"') + ">" +
        icon("plus") + (wide ? "Add Mass item" : "") + "</button></div>" +
        '<div class="o-pane-body o-rows--open">' + html + "</div></section>";
    }
    function thumbs(it) {
      return '<div class="pl-thumbs">' + it.cues.map(function (q, i) {
        var cur = i === S.cue ? ' aria-current="true"' : "";
        return '<button class="pr-tile pl-thumb" data-cue="' + i + '"' + cur + '><div class="o-slide-wrap"' + cur + ">" + tslide(q, th(it)) + "</div>" +
          '<div class="o-caption"><span class="o-n">' + (i + 1) + '</span><span class="o-t">' + esc(q.k === "unavailable" ? "Unavailable" : cueLabel(q)) + "</span></div></button>";
      }).join("") + "</div>";
    }
    function songEditor(it, wide) {
      if (!it.song) {
        return '<div class="pl-empty">' + icon("music", "o-icon-20") + "<b>No Song chosen yet</b>" +
          '<div class="o-meta">Pick a Song and one of its Arrangements. Nothing is presented for this item until you do.</div>' +
          '<button class="o-btn o-btn--primary" data-act="picker" data-focus-key="choose">' + icon("search") + "Choose Song</button></div>";
      }
      var local = it.local;
      var banner = local ? '<div class="o-notice" data-state="info">' + icon("pencil") + '<div class="o-grow"><b>Mass-local override.</b> Changes here belong only to this Mass plan; the library Arrangement is unchanged.' +
        '<div class="pl-banner-acts"><button class="o-btn" data-act="return">' + icon("undo-2") + "Return to library arrangement</button>" +
        '<button class="o-btn o-btn--quiet">' + icon("library") + "Open in Song Library</button></div></div></div>" : "";
      var arr = '<div class="pl-card"><div class="o-grow"><div class="o-label">' + (local ? "Copied from " : "Using ") + esc(it.arr) + " arrangement</div>" +
        '<div class="o-meta">' + esc(K.MASS.language) + (local ? " · Mass-local override" : " · linked to Song Library" + (wide ? " · last reviewed 17 Okt 2026" : "")) + "</div></div>" +
        '<button class="o-btn" data-act="picker">' + icon("replace") + (wide ? "Change Song or Arrangement" : "Change") + "</button>" +
        (local ? "" : '<button class="o-btn" data-act="customize">' + icon("pencil") + "Customize for this Mass</button>") + "</div>";
      var seq;
      if (local) {
        seq = it.cues.map(function (q, i) {
          var name = q.k === "song-id" ? "Song title" : q.h + (q.forked ? " (this occurrence)" : "");
          var link = linked(it, i) ? '<span class="pl-chip">' + icon("link") + "Linked</span>" : "";
          return '<div class="o-row pl-seq pl-occ"' + (i === S.cue ? ' aria-selected="true"' : "") + ">" + icon("grip-vertical") +
            '<button class="pl-occ-name" data-cue="' + i + '"><span class="o-meta">' + (i + 1) + "</span> " + esc(name) + "</button>" + link + '<span class="o-grow"></span>' +
            '<button class="o-btn o-btn--quiet o-btn--icon" data-move="' + i + ':-1" aria-label="Move up" title="Move up"' + (i === 0 ? " disabled" : "") + ">" + icon("arrow-up") + "</button>" +
            '<button class="o-btn o-btn--quiet o-btn--icon" data-move="' + i + ':1" aria-label="Move down" title="Move down"' + (i === it.cues.length - 1 ? " disabled" : "") + ">" + icon("arrow-down") + "</button>" +
            '<button class="o-btn o-btn--quiet o-btn--icon" data-remove="' + i + '" aria-label="Remove from sequence" title="Remove from sequence">' + icon("x") + "</button></div>";
        }).join("");
        var c = it.cues[S.cue];
        if (c && c.k === "lyrics") {
          seq += '<div class="pl-text"><label class="o-label" for="pl-text-' + it.n + '">' + esc(c.h) + (c.forked ? " · this occurrence" : "") + " text</label>" +
            '<textarea id="pl-text-' + it.n + '" class="pl-textarea" data-input="text" data-focus-key="text" rows="3">' + esc(c.b) + "</textarea>" +
            (linked(it, S.cue) ? '<div class="pl-text-foot"><span class="o-meta">' + icon("link") + " Linked: editing " + esc(c.h) + " changes every " + esc(c.h) + " occurrence in this Song item.</span>" +
              '<button class="o-btn o-btn--quiet" data-act="fork">Edit this occurrence only</button></div>' : "") + "</div>";
        }
      } else {
        seq = it.cues.map(function (q, i) {
          var name = q.k === "song-id" ? "Song title" : q.k === "unavailable" ? q.b : q.h;
          return '<div class="o-row pl-seq"><span class="o-meta">' + (i + 1) + '</span><span class="o-grow">' + esc(name) + "</span>" +
            (q.k === "unavailable" ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : '<span class="o-meta">1 Cue</span>') + "</div>";
        }).join("");
      }
      return '<div class="pl-ed-body">' + banner + arr +
        '<div class="pl-block"><div class="pl-block-head"><b>Sequence</b><span class="o-meta">' + it.cues.length + " sections · " + it.cues.length + (wide ? " generated Cues" : " Cues") + "</span></div>" + seq + "</div>" +
        '<div class="pl-block"><div class="pl-block-head"><b>Generated Cues</b><span class="o-meta">Selected ' + (S.cue + 1) + " of " + it.cues.length + (wide ? " · click a Cue to inspect" : "") + "</span></div>" + thumbs(it) + "</div>" +
        (wide && !local ? '<div class="pl-block"><div class="pl-block-head"><b>Mass-local state</b></div><div class="o-meta">Following the library Arrangement; no changes for this Mass.</div></div>' : "") +
        "</div>";
    }
    function cueEditor(it) {
      if (it.empty) {
        var spec = NEW_ITEM[it.type] || NEW_ITEM[it.type === "Response sequence" ? "Response or Prayer" : "Custom sequence"];
        return '<div class="pl-empty">' + icon(spec.icon, "o-icon-20") + "<b>This " + esc(it.type) + " is empty</b>" +
          '<div class="o-meta">' + esc(spec.note) + ". Its editor is a separate screen, not part of this mockup.</div>" +
          '<button class="o-btn" data-focus-key="fix">' + icon("plus") + "Add content</button></div>";
      }
      return '<div class="pl-ed-body"><div class="pl-block"><div class="pl-block-head"><b>' + esc(it.type) + '</b><span class="o-meta">' + it.cues.length + (it.cues.length === 1 ? " Cue" : " Cues") + "</span></div>" +
        it.cues.map(function (q, i) {
          var lead = q.k === "trigger" ? '<span class="pr-who">' + esc(q.who) + "</span>" : q.k === "unavailable" ? icon("circle-off") :
            q.k === "wait" ? icon("hourglass") : q.k === "hold" ? icon("square") : q.k === "blackout" ? icon("moon") : icon("monitor");
          var opOnly = q.k === "trigger" || q.k === "wait" || q.k === "hold" || q.k === "blackout";
          var tag = q.k === "trigger" ? '<span class="o-meta">Operator only</span>' : opOnly ? '<span class="o-meta">' + KIND[q.k] + " · operator only</span>" :
            q.k === "unavailable" ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : '<span class="o-meta">Output</span>';
          var sub = q.k === "trigger" ? (q.who === "P" ? "Priest" : "Lector") + " · private timing prompt" : q.k === "response" ? "Congregation response" :
            q.k === "blackout" ? "Program stays black until Space" : KIND[q.k];
          return '<button class="o-row pl-cue" data-cue="' + i + '"' + (i === S.cue ? ' aria-selected="true"' : "") + '><span class="o-meta">' + (i + 1) + "</span>" + lead +
            '<span class="o-grow"><span class="o-ellipsis pl-cue-t">' + esc(cueLabel(q)) + '</span><span class="o-meta">' + esc(sub) + "</span></span>" + tag + "</button>";
        }).join("") + "</div></div>";
    }
    function editor(it, wide) {
      var nBad = bad(it);
      return '<section class="o-pane pl-editor" aria-label="Selected Mass item"><div class="o-pane-head"><div class="o-grow"><div class="o-meta">' + esc(S.sec[it.section].title) + " · " + esc(it.type) + "</div><b>" + it.n + ". " + esc(it.title) + "</b>" +
        (it.song ? ' <span class="o-muted">' + esc(it.song) + "</span>" : "") + "</div>" + (it.theme ? '<span class="o-meta">' + icon("palette") + " " + esc(themeName(it.theme)) + "</span>" : "") + "</div>" +
        (nBad ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div><b>" + nBad + " Cues unavailable.</b> " + esc(it.cues[0].why) + '. <button class="pl-link" data-act="picker" data-focus-key="fix">Choose another Arrangement</button></div></div>' : "") +
        (it.warn ? '<div class="o-notice" data-state="info">' + icon("info") + "<div><b>Pagination not reviewed.</b> " + esc(it.warn) + '. <button class="pl-link" data-focus-key="fix">Review pagination</button></div></div>' : "") +
        '<div class="o-pane-body">' + (it.type === "Song" ? songEditor(it, wide) : cueEditor(it)) + "</div></section>";
    }
    function inspector(it, wide) {
      var n = it.cues.length, c = it.cues[S.cue];
      var head = '<div class="o-pane-head"><b>Inspector</b>';
      if (!c) return '<aside class="o-pane pl-inspector" aria-label="Inspector">' + head + '</div><div class="o-pane-body pl-insp-body">' + tslide(null, th(it)) + '<div class="o-meta">Nothing to inspect: this item has no Cues yet.</div></div></aside>';
      var first = S.cue === 0, last = S.cue === n - 1;
      var opOnly = c.k === "trigger" || c.k === "wait" || c.k === "hold" || c.k === "blackout";
      var frame = opOnly ? tslide(null, th(it)) : tslide(c, th(it));
      var ctx = "";
      if (c.k === "trigger") ctx = '<div class="pr-private"><span class="pr-who">' + esc(c.who) + "</span><div><b>" + (c.who === "P" ? "Priest" : "Lector") + ": " + esc(c.b) + '</b><div class="o-meta">Advance when this line is finished.</div></div></div>';
      else if (c.k === "wait" || c.k === "hold" || c.k === "blackout") ctx = '<div class="pr-private">' + icon(c.k === "wait" ? "hourglass" : c.k === "hold" ? "square" : "moon") + "<div><b>" + KIND[c.k] + " · " + esc(c.b) + '</b><div class="o-meta">' +
        (c.k === "wait" ? "Program stays as it is" : c.k === "hold" ? "Black checkpoint, not Blackout" : "Program stays black until Space takes Preview live") + "</div></div></div>";
      else if (c.k === "unavailable") ctx = '<div class="pr-private" data-state="unavailable">' + icon("circle-off") + "<div><b>Unavailable</b><div>" + esc(c.why) + "</div></div></div>";
      var items_ = items(), nextItem = items_[it.n], prevItem = items_[it.n - 2];
      var edge = last ? "End of " + it.title + "." + (nextItem ? " Select " + nextItem.title + " in the Mass outline to inspect it." : "") : first && prevItem ? "Start of " + it.title + "." : "";
      var meta = [["Congregation frame", opOnly ? "Black" : c.k === "unavailable" ? "Unavailable" : "Canonical 16:9"], ["Theme", themeName(th(it)) + (it.theme ? " · item override" : "")],
        ["Pagination", c.k === "unavailable" ? "—" : it.warn ? "Not reviewed" : "Fits safely"], ["Output", "1920 × 1080"]];
      if (wide || it.local) meta.push(["Source", it.local ? "Mass-local override" : it.song ? "Library Arrangement" : "Mass item"]);
      if (wide) meta.push(["Safe area", "Within 120 / 80 px"]);
      return '<aside class="o-pane pl-inspector" aria-label="Inspector">' + head + (opOnly ? '<span class="o-meta">Operator only</span>' : "") + '<span class="o-grow"></span>' +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-step="-1" aria-label="Previous Cue" title="Previous Cue"' + (first ? " disabled" : "") + ">" + icon("chevron-left") + "</button>" +
        '<span class="o-meta">Cue ' + (S.cue + 1) + " of " + n + "</span>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-step="1" aria-label="Next Cue" title="Next Cue"' + (last ? " disabled" : "") + ">" + icon("chevron-right") + "</button></div>" +
        '<div class="o-pane-body pl-insp-body">' + frame + ctx +
        '<dl class="pl-meta">' + meta.map(function (m) { return "<dt>" + m[0] + "</dt><dd>" + esc(m[1]) + "</dd>"; }).join("") + "</dl>" +
        '<div class="pl-scope"><span class="o-meta">Navigation scope</span><b>This item only</b>' + (edge ? '<div class="o-meta">' + esc(edge) + "</div>" : "") + "</div>" +
        "</div></aside>";
    }

    // overlays
    function closeTab(label) { return '<button class="pl-close-tab" data-act="close" aria-label="' + label + '" title="' + label + '">' + icon("chevron-right") + "</button>"; }
    function addMenu() {
      var opts_ = "";
      S.sec.forEach(function (sec, si) {
        opts_ += '<option value="' + si + ':-1"' + (S.dest.sec === si && S.dest.at === -1 ? " selected" : "") + ">Start of " + esc(sec.title) + "</option>";
        sec.items.forEach(function (x, ai) { opts_ += '<option value="' + si + ":" + ai + '"' + (S.dest.sec === si && S.dest.at === ai ? " selected" : "") + ">After " + esc(x.title) + " · " + esc(sec.title) + "</option>"; });
      });
      var types = Object.keys(NEW_ITEM).map(function (t) {
        return '<button class="o-row pl-type" role="menuitem" data-add="' + esc(t) + '">' + icon(NEW_ITEM[t].icon) + '<span class="o-grow"><b>' + esc(t) + '</b><span class="o-meta">' + esc(NEW_ITEM[t].note) + "</span></span></button>" +
          (t === "General Presentation" ? '<div class="pl-menu-sep"></div>' : "");
      }).join("");
      return '<div class="o-menu pl-menu" role="menu" aria-label="Add Mass item">' +
        '<div class="pl-dest"><span class="o-meta">Insert</span><b>' + esc(destLabel(S.dest)) + "</b>" +
        '<label class="o-meta pl-dest-change">' + icon("map-pin") + 'Change<select data-dest aria-label="Insertion destination">' + opts_ + "</select></label></div>" + types + "</div>";
    }
    function picker() {
      var st = staged(), it = S.sel, q = S.pick.q.toLowerCase();
      var replacing = !!it.song;
      var list = LIB.map(function (song, si) {
        if (q && (song.title + " " + song.by).toLowerCase().indexOf(q) < 0) return "";
        var open = S.pick.song === si;
        var row = '<button class="o-row pl-result" data-song="' + si + '"' + (open ? ' aria-expanded="true"' : ' aria-expanded="false"') + ">" + icon(open ? "chevron-down" : "chevron-right") +
          '<span class="o-grow"><b>' + esc(song.title) + '</b> <span class="o-meta">' + esc(song.by) + '</span></span><span class="o-meta">' + song.arrs.length + (song.arrs.length === 1 ? " Arrangement" : " Arrangements") + "</span></button>";
        if (!open) return row;
        return row + '<div class="pl-arrs">' + song.arrs.map(function (a, ai) {
          var cur = isCurrent(song, a), sel = S.pick.arr === ai;
          return '<button class="o-row pl-arr" data-arr="' + ai + '"' + (sel ? ' aria-selected="true"' : "") + (a.broken ? " disabled" : "") + ">" + icon(sel ? "circle-dot" : "circle") +
            '<span class="o-grow"><span class="o-label">' + esc(a.name) + '</span><span class="o-meta">' + (a.broken ? esc(a.broken) : a.cues.length + " Cues · " + esc(K.MASS.language)) + "</span></span>" +
            (cur ? '<span class="pl-chip">Current</span>' : "") + (a.broken ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : "") + "</button>";
        }).join("") + "</div>";
      }).join("") || '<div class="o-meta pl-none">No Songs match “' + esc(S.pick.q) + "”.</div>";
      var preview = "";
      if (st) {
        var firstOut = st.arr.cues.filter(K.projectable)[0];
        preview = '<div class="pl-staged"><div class="pl-block-head"><b>Staged</b><span class="o-meta">' + esc(st.song.title) + " · " + esc(st.arr.name) + "</span></div>" + tslide(firstOut, th(it)) +
          '<div class="pl-seqchips">' + st.arr.cues.map(function (c) { return '<span class="pl-chip">' + esc(c.k === "song-id" ? "Title" : c.h) + "</span>"; }).join("") + "</div>" +
          (it.local ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div>This item has Mass-local changes. Using another Arrangement asks you to confirm first.</div></div>" : "") + "</div>";
      }
      var use = st ? (isCurrent(st.song, st.arr) ? "Already in use" : "Use " + st.arr.name + " arrangement") : "Use arrangement";
      return '<div class="o-scrim pl-scrim--light"></div><div class="o-slideover pl-picker' + (S.child ? " is-receded" : "") + '" role="dialog" aria-modal="true" aria-labelledby="pl-pick-t">' + closeTab("Close Song picker") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="pl-pick-t">' + (replacing ? "Change Song or Arrangement" : "Choose Song") + "</h2>" +
        '<div class="o-meta">For ' + it.n + ". " + esc(it.title) + " · " + esc(S.sec[it.section].title) + "</div></div>" +
        '<button class="o-btn o-btn--primary" data-act="use"' + (!st || isCurrent(st.song, st.arr) ? " disabled" : "") + ">" + icon("check") + esc(use) + "</button></div>" +
        '<div class="o-overlay-body"><div class="o-field">' + icon("search") + '<input data-input="q" data-focus-key="q" placeholder="Search Songs and composers" value="' + esc(S.pick.q) + '" aria-label="Search Songs"></div>' +
        preview + '<div class="pl-results">' + list + "</div></div></div>";
    }
    function changesList(it) {
      var ch = it.changes.length ? it.changes : ["No edits yet beyond the copy"];
      return '<ul class="pl-changes">' + ch.map(function (c) { return "<li>" + esc(c) + "</li>"; }).join("") + "</ul>";
    }
    function review() {
      var st = staged(), it = S.sel;
      return '<div class="o-slideover pl-child" role="dialog" aria-modal="true" aria-labelledby="pl-rev-t">' +
        '<div class="o-overlay-head"><button class="o-btn o-btn--quiet o-btn--icon" data-act="back" aria-label="Back to Song picker" title="Back">' + icon("arrow-left") + "</button>" +
        '<div class="o-grow"><h2 class="o-h2" id="pl-rev-t">Replace a customized Song?</h2><div class="o-meta">' + it.n + ". " + esc(it.title) + "</div></div></div>" +
        '<div class="o-overlay-body"><div class="pl-compare"><div><div class="o-section-label">Current</div><b>' + esc(it.song) + "</b><div class=\"o-meta\">Mass-local override, copied from " + esc(it.arr) + "</div>" + changesList(it) + "</div>" +
        '<div><div class="o-section-label">Replacement</div><b>' + esc(st.song.title) + '</b><div class="o-meta">' + esc(st.arr.name) + " · " + st.arr.cues.length + " Cues, from the Song Library</div></div></div>" +
        '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div>The local lyric, sequence and pagination changes are detached from this item; Ordo never merges them into another Arrangement. Undo brings them back.</div></div></div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-act="back">Back</button><button class="o-btn o-btn--primary" data-act="confirm-replace">Replace</button></div></div>';
    }
    function returnConfirm() {
      var it = S.sel;
      return '<div class="o-scrim"></div><div class="o-slideover" role="dialog" aria-modal="true" aria-labelledby="pl-ret-t">' + closeTab("Cancel") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="pl-ret-t">Return to library arrangement?</h2><div class="o-meta">' + it.n + ". " + esc(it.title) + "</div></div></div>" +
        '<div class="o-overlay-body"><div class="pl-compare"><div><div class="o-section-label">Library Arrangement</div><b>' + esc(it.song) + '</b><div class="o-meta">' + esc(it.arr) + " · " + it.base.length + " Cues</div></div>" +
        '<div><div class="o-section-label">Local changes removed</div>' + changesList(it) + "</div></div>" +
        '<p class="o-meta">The item follows the library Arrangement again. Undo restores the local copy.</p></div>' +
        '<div class="o-overlay-foot"><button class="o-btn" data-act="close">Cancel</button><button class="o-btn o-btn--primary" data-act="confirm-return">Return to library arrangement</button></div></div>';
    }
    function readiness() {
      var f = findings();
      function group(title, list, state) {
        if (!list.length) return "";
        return '<div class="pl-fgroup"><div class="o-section-label">' + title + "</div>" + list.map(function (x) {
          return '<div class="pl-find">' + statusLabel(state, "", state === "warning" ? "diamond" : null) + '<div class="o-grow"><button class="pl-link pl-find-t" data-find="' + x.it.n + '">' + x.it.n + ". " + esc(x.it.title) + "</button>" +
            "<div><b>" + esc(x.title) + ".</b> " + esc(x.text) + '</div><button class="o-btn" data-fix="' + x.it.n + '">' + esc(x.action) + "</button></div></div>";
        }).join("") + "</div>";
      }
      var sum = f.inc.length ? statusLabel("warning", "Incomplete", "diamond") + '<span class="o-meta">' + f.inc.length + " incomplete · " + f.warn.length + " warning" + (f.warn.length === 1 ? "" : "s") + "</span>" :
        f.warn.length ? statusLabel("info", "Ready with warnings") : statusLabel("live", "Ready");
      return '<div class="o-slideover pl-readiness" role="dialog" aria-labelledby="pl-rd-t">' + closeTab("Close Readiness") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="pl-rd-t">Readiness</h2><div class="pl-rd-sum">' + sum + "</div></div></div>" +
        '<div class="o-overlay-body">' + group("Incomplete presentation", f.inc, "warning") + group("Review recommended", f.warn, "info") +
        (f.inc.length || f.warn.length ? '<p class="o-meta">Incomplete content doesn\'t stop Present: its Cues stay visible but can\'t be taken live.</p>' :
          '<div class="pl-empty">' + icon("circle-check", "o-icon-20") + "<b>Nothing needs attention</b><div class=\"o-meta\">Every Cue can produce its frame.</div></div>") + "</div></div>";
    }
    function themeChooser() {
      var c = S.sel.cues[S.cue], ov = overridden();
      var rows = THEMES.map(function (t) {
        var sel = S.stagedTheme === t.id;
        return '<button class="o-row pl-theme" data-theme="' + t.id + '"' + (sel ? ' aria-selected="true"' : "") + '><span class="pl-swatch">' + tslide({ k: "response", h: "", b: "Aa" }, t.id) + "</span>" +
          '<span class="o-grow"><span class="o-label">' + esc(t.name) + '</span><span class="o-meta">' + esc(t.note) + "</span></span>" + (S.theme === t.id ? '<span class="pl-chip">Current</span>' : "") + "</button>";
      }).join("");
      var keep = ov.filter(function (it) { return S.keepOverride[it.n] !== false; });
      var disclose = ov.length ? '<div class="o-notice" data-state="info">' + icon("info") + '<div class="o-grow">' +
        (keep.length ? "<b>" + keep.length + (keep.length === 1 ? " item keeps its own Theme" : " items keep their own Theme") + "</b> and won't change: " + keep.map(function (it) { return it.n + ". " + esc(it.title) + " uses " + esc(themeName(it.theme)); }).join("; ") + "." :
          "<b>All item overrides will be removed</b> when you apply.") +
        ' <button class="pl-link" data-act="overrides">Review</button></div></div>' : "";
      var changed = S.stagedTheme !== S.theme || ov.some(function (it) { return S.keepOverride[it.n] === false; });
      return '<div class="o-scrim"></div><div class="o-slideover pl-theme-panel' + (S.child ? " is-receded" : "") + '" role="dialog" aria-modal="true" aria-labelledby="pl-th-t">' + closeTab("Close Mass Theme") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="pl-th-t">Mass Theme</h2><div class="o-meta">Applies to every item that doesn\'t keep its own Theme</div></div>' +
        '<button class="o-btn o-btn--primary" data-act="apply-theme"' + (changed ? "" : " disabled") + ">" + icon("check") + (S.stagedTheme !== S.theme ? "Apply " + esc(themeName(S.stagedTheme)) : changed ? "Apply override changes" : "Apply") + "</button></div>" +
        '<div class="o-overlay-body"><div class="pl-block-head"><b>Preview</b><span class="o-meta">' + S.sel.n + ". " + esc(S.sel.title) + " · Cue " + (S.cue + 1) + (S.sel.theme ? " · keeps its override" : "") + "</span></div>" +
        tslide(c && K.projectable(c) ? c : S.sel.cues.filter(K.projectable)[0] || null, S.sel.theme && S.keepOverride[S.sel.n] !== false ? S.sel.theme : S.stagedTheme) +
        disclose + '<div class="pl-themes">' + rows + "</div>" +
        '<button class="o-btn o-btn--quiet">' + icon("pencil") + "Edit selected Theme</button></div></div>";
    }
    function overridesPanel() {
      var rows = overridden().map(function (it) {
        var keep = S.keepOverride[it.n] !== false, sample = it.cues.filter(K.projectable)[0] || null;
        return '<div class="pl-ovr"><div class="pl-block-head"><b>' + it.n + ". " + esc(it.title) + "</b>" +
          '<button class="o-btn" data-ovr="' + it.n + '">' + (keep ? "Use Mass Theme" : "Keep override") + "</button></div>" +
          '<div class="pl-ovr-cmp"><div>' + tslide(sample, it.theme) + '<span class="o-meta">Current override · ' + esc(themeName(it.theme)) + "</span></div>" + icon("arrow-right") +
          "<div>" + tslide(sample, S.stagedTheme) + '<span class="o-meta">Inherited · ' + esc(themeName(S.stagedTheme)) + "</span></div></div>" +
          '<div class="o-meta">' + (keep ? "Keeps " + esc(themeName(it.theme)) + "." : "Will use the Mass Theme, " + esc(themeName(S.stagedTheme)) + ".") + "</div></div>";
      }).join("");
      return '<div class="o-slideover pl-child" role="dialog" aria-modal="true" aria-labelledby="pl-ov-t">' +
        '<div class="o-overlay-head"><button class="o-btn o-btn--quiet o-btn--icon" data-act="back" aria-label="Back to Mass Theme" title="Back">' + icon("arrow-left") + "</button>" +
        '<div class="o-grow"><h2 class="o-h2" id="pl-ov-t">Theme Overrides</h2><div class="o-meta">Items that keep their own Theme, in Mass order</div></div>' +
        '<button class="o-btn" data-act="ovr-all">Use Mass Theme for all</button></div>' +
        '<div class="o-overlay-body">' + rows + "</div>" +
        '<div class="o-overlay-foot"><button class="o-btn o-btn--primary" data-act="back">Done</button></div></div>';
    }

    function render() {
      app.style.setProperty("--o-scale", S.scale);
      var wide = W / S.scale >= 1700;
      app.dataset.layout = wide ? "wide" : "compact";
      items();
      var it = S.sel, f = findings();
      // keep scroll and focus across redraws (outline always; editor and Inspector within one item)
      var keep = {}, ae = document.activeElement, fk = ae && app.contains(ae) && ae.dataset ? ae.dataset.focusKey : null, caret = fk && ae.selectionStart;
      [".pl-outline", ".pl-editor", ".pl-inspector", ".pl-picker", ".pl-readiness", ".pl-theme-panel"].forEach(function (q) {
        var el = app.querySelector(q + " .o-pane-body, " + q + " .o-overlay-body"); if (el) keep[q] = el.scrollTop; });
      var sameItem = S.drawn === it; S.drawn = it;
      var ov = "";
      if (S.overlay === "add") ov = addMenu();
      else if (S.overlay === "picker") ov = picker() + (S.child === "review" ? review() : "");
      else if (S.overlay === "return") ov = returnConfirm();
      else if (S.overlay === "readiness") ov = readiness();
      else if (S.overlay === "theme") ov = themeChooser() + (S.child === "overrides" ? overridesPanel() : "");
      var toast = S.toast ? '<div class="o-toast" role="status">' + esc(S.toast) + (S.undo ? '<button class="o-btn" data-act="undo">Undo</button>' : "") + "</div>" : "";
      app.innerHTML = '<div class="o-body">' + K.navrail("Masses") + '<div class="o-grow pl-ws">' + header(wide, f) +
        '<div class="pl-panes">' + outline(wide) + editor(it, wide) + inspector(it, wide) + "</div></div></div>" + ov + toast;
      K.icons();
      Object.keys(keep).forEach(function (q) {
        var el = app.querySelector(q + " .o-pane-body, " + q + " .o-overlay-body");
        if (el && (q === ".pl-outline" || q.indexOf("pl-picker") > 0 || q.indexOf("readiness") > 0 || q.indexOf("theme") > 0 || sameItem)) el.scrollTop = keep[q];
      });
      if (S.overlay === "add") {                          // anchor the menu under its button
        var b = app.querySelector('[data-anchor="add"]'), m = app.querySelector(".pl-menu"), a = app.getBoundingClientRect(), k = a.width / app.offsetWidth || 1, br = b.getBoundingClientRect();
        m.style.left = (br.left - a.left) / k + "px"; m.style.top = (br.bottom - a.top) / k + 4 + "px";
      }
      var want = S.focus || fk;
      if (want) {
        var t = app.querySelector('[data-focus-key="' + want + '"]');
        if (t) { t.focus({ preventScroll: !S.focus }); if (S.focus) t.classList.add("o-focus"); if (caret != null && t.setSelectionRange && !S.focus) t.setSelectionRange(caret, caret); }
      }
      S.focus = null;
      if (S.overlay === "picker" && !fk && !S.child && opts.live) { var qi = app.querySelector('[data-focus-key="q"]'); if (qi) qi.focus({ preventScroll: true }); }
      if (opts.afterRender) opts.afterRender(api);
    }

    app.addEventListener("click", function (e) {
      var t = e.target.closest("[data-act],[data-item],[data-cue],[data-step],[data-add],[data-song],[data-arr],[data-theme],[data-ovr],[data-find],[data-fix],[data-move],[data-remove]");
      if (!t || t.disabled) return;
      e.preventDefault();
      var d = t.dataset, a = d.act;
      if (a !== "undo") { S.toast = null; S.undo = null; }   // a toast and its Undo last until the next action
      if (a === "add") { if (S.overlay === "add") S.overlay = null; else openAdd(); }
      else if (d.add) addItem(d.add);
      else if (a === "picker") openPicker();
      else if (d.song != null) { var si = +d.song; S.pick.song = S.pick.song === si ? null : si; S.pick.arr = null; }
      else if (d.arr != null) S.pick.arr = +d.arr;
      else if (a === "use") commitPick();
      else if (a === "confirm-replace") applyPick(staged());
      else if (a === "back") S.child = null;
      else if (a === "close") { if (S.child) S.child = null; else S.overlay = null; }
      else if (a === "customize") customize();
      else if (a === "return") S.overlay = "return";
      else if (a === "confirm-return") returnToLibrary();
      else if (a === "fork") { var c = S.sel.cues[S.cue]; c.forked = true; change("Forked one " + c.h + " occurrence"); }
      else if (d.move) { var p = d.move.split(":"), i = +p[0], j = i + +p[1], cs = S.sel.cues, x = cs[i]; cs[i] = cs[j]; cs[j] = x; S.cue = j; change("Reordered the sequence"); }
      else if (d.remove != null) { var r = +d.remove, gone = S.sel.cues.splice(r, 1)[0]; S.cue = Math.max(0, Math.min(S.cue, S.sel.cues.length - 1)); change("Removed a " + (gone.h || "title") + " occurrence"); }
      else if (a === "readiness") S.overlay = S.overlay === "readiness" ? null : "readiness";
      else if (d.find) { select(+d.find); }
      else if (d.fix) { select(+d.fix); S.overlay = null; S.focus = "fix"; if (S.sel.type === "Song" && !S.sel.song) S.focus = "choose"; }
      else if (a === "theme") openTheme();
      else if (d.theme) S.stagedTheme = d.theme;
      else if (a === "overrides") S.child = "overrides";
      else if (d.ovr) S.keepOverride[d.ovr] = S.keepOverride[d.ovr] === false;
      else if (a === "ovr-all") overridden().forEach(function (it) { S.keepOverride[it.n] = false; });
      else if (a === "apply-theme") applyTheme();
      else if (a === "undo") undo();
      else if (d.item) { select(+d.item); if (S.overlay === "add") S.overlay = null; }
      else if (d.cue != null) S.cue = +d.cue;
      else if (d.step) { var n = S.sel.cues.length; S.cue = Math.max(0, Math.min(n - 1, S.cue + +d.step)); }   // stops at item boundaries (Decision 169)
      render();
    });
    app.addEventListener("input", function (e) {
      var k = e.target.dataset.input; if (!k) return;
      if (k === "q") { S.pick.q = e.target.value; S.pick.arr = S.pick.arr; }
      else if (k === "text") editText(e.target.value);
      render();
    });
    app.addEventListener("change", function (e) {
      if (e.target.hasAttribute("data-dest")) { var p = e.target.value.split(":"); S.dest = { sec: +p[0], at: +p[1] }; render(); }
    });
    app.addEventListener("keydown", function (e) {
      if (e.key !== "Escape" || !S.overlay) return;
      if (S.child) S.child = null; else S.overlay = null;
      e.preventDefault(); render();
    });

    // A small scripting surface so a page can open the planner in a given state.
    var api = {
      app: app, state: S, render: render, items: items,
      select: function (n) { select(n); return api; }, cue: function (i) { S.cue = i; return api; },
      openAdd: function () { openAdd(); return api; }, add: function (type) { openAdd(); addItem(type); return api; },
      openPicker: function () { openPicker(); return api; }, stage: function (song, arr) { stage(song, arr); return api; },
      commit: function () { commitPick(); return api; }, readiness: function () { S.overlay = "readiness"; return api; },
      theme: function (id) { openTheme(); if (id) S.stagedTheme = id; return api; }, overrides: function () { S.child = "overrides"; return api; },
      customize: function () { customize(); S.toast = null; return api; }, edit: function (v) { editText(v); return api; },
      removeAt: function (i) { var gone = S.sel.cues.splice(i, 1)[0]; change("Removed a " + gone.h + " occurrence"); return api; },
      openReturn: function () { S.overlay = "return"; return api; }, quiet: function () { S.toast = null; S.undo = null; return api; }
    };
    K.fit(wrapper, app, W, H);
    if (opts.setup) opts.setup(api);
    render();
    return api;
  }

  K.planner = planner;
  K.THEMES = THEMES;
  K.LIBRARY = LIB;
})();
