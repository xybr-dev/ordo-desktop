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
  // Library entries Quick Insert can find (Decisions 12, 258).
  var QI_SONGS = [
    { t: "Pan sa Kinabuhi", a: "Koro San Roque", lang: "Cebuano", rep: true, alias: ["Bread of Life"], first: "Ikaw ang pan sa kinabuhi", slides: ["Ikaw ang pan sa kinabuhi\nnga gikan sa langit", "Kinsa ang mokaon niini\ndili na gutomon"] },
    { t: "Panalangin", a: "Koro San Roque", lang: "Cebuano", rep: true, alias: [], first: "Panalangin sa Ginoo", slides: ["Panalangin sa Ginoo\nmaanaa kanatong tanan", "Ug ang iyang kalinaw\nmagpabilin kanato"] },
    { t: "Salamat, Ginoo", a: "Koro San Roque", lang: "Cebuano", rep: true, alias: ["Pasalamat"], first: "Salamat, Ginoo, sa tanan", slides: ["Salamat, Ginoo, sa tanan\nnimong gihatag kanamo", "Ang among kasingkasing\nnagadayeg kanimo"] },
    { t: "Kanunay Ka Uban Kanamo", a: "Koro Sta. Cecilia", lang: "Cebuano", rep: false, alias: [], first: "Bisan asa kami moadto", slides: ["Bisan asa kami moadto\nkanunay ka uban kanamo", "Ikaw ang among kahayag\nsa dalan sa kinabuhi"] },
    { t: "Ave Maria", a: "Gounod · Youth Choir", lang: "Latin", rep: false, alias: ["Hail Mary"], first: "Ave Maria, gratia plena", slides: ["Ave Maria, gratia plena\nDominus tecum", "Benedicta tu in mulieribus"] },
    { t: "Bayan Umawit", a: "Youth Choir", lang: "Tagalog", rep: false, alias: [], first: "Bayan, umawit ng papuri", slides: ["Bayan, umawit ng papuri\nsa Panginoon", "Purihin ang Kanyang ngalan"] },
    { t: "Halad", a: "Koro Sta. Cecilia", lang: "Cebuano", rep: false, alias: ["Offering"], first: "Kining pan ug bino", slides: ["Kining pan ug bino\namong ginahalad", "Dawata, O Ginoo,\nang among kinabuhi"] }
  ];
  var QI_RESP = [
    { t: "Pag-ampo alang sa mga Masakiton", lang: "Cebuano", cues: [{ k: "trigger", who: "P", b: "Mag-ampo kita alang sa atong mga masakiton." }, { k: "response", h: "TUBAG:", b: "Ginoo, ayoha sila." }] },
    { t: "Angelus", lang: "Cebuano", cues: [{ k: "trigger", who: "L", b: "Ang Anghel sa Ginoo nagpahibalo kang Maria." }, { k: "response", h: "TUBAG:", b: "Ug siya nanamkon pinaagi sa Espiritu Santo." }] },
    { t: "Pag-ampo alang sa mga Namatay", lang: "Cebuano", cues: [{ k: "trigger", who: "P", b: "Ang kahayag nga walay katapusan isidlak kanila, O Ginoo." }, { k: "response", h: "TUBAG:", b: "Ug mopahulay sila sa kalinaw. Amen." }] }
  ];
  function cloneMass() {
    function strip(o, drop) { var x = {}; Object.keys(o).forEach(function (k) { if (drop.indexOf(k) < 0) x[k] = o[k]; }); return x; }
    return { title: MASS.title, date: MASS.date, choir: MASS.choir, language: MASS.language, sections: MASS.sections.map(function (sec) {
      return { title: sec.title, items: sec.items.map(function (it) { var x = strip(it, ["cues", "section", "n", "first"]); x.cues = it.cues.map(function (c) { return strip(c, ["item", "i", "ci"]); }); return x; }) };
    }) };
  }
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
    if (c.img) return '<div class="o-canvas" data-t="image"><div class="o-img" title="' + esc(c.img) + '"></div></div>';
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

    // Each Presenter owns a copy of the Mass so live inserts never leak into other mockups.
    var M = cloneMass(), ITEMS = [], CUES = [], UID = presenter.n = (presenter.n || 0) + 1;   // radio groups stay per Presenter
    function reindex() {
      ITEMS.length = 0; CUES.length = 0;
      M.sections.forEach(function (sec, si) { sec.items.forEach(function (it) {
        it.section = si; it.n = ITEMS.length + 1; it.first = CUES.length; ITEMS.push(it);
        it.cues.forEach(function (c, ci) { c.item = it; c.i = CUES.length; c.ci = ci; CUES.push(c); });
      }); });
    }
    reindex();
    var S = {
      scale: opts.scale || 1, mode: opts.mode || "grid", railPref: opts.rail || "auto", railOpen: false,
      output: (opts.state && opts.state.output) || "disabled",
      preview: opts.preview != null ? opts.preview : 1,
      program: opts.program != null ? opts.program : null,
      live: !!opts.live, frozen: !!opts.frozen, blackout: false, frozenReason: "", blackReason: "",
      collapsed: {}, message: "", preflight: opts.preflight || null, back: null, strip: null, prior: null, cong: { n: "2", name: "Epson EB-X51" }
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
      if (S.output === "disconnected") {
        if (S.back !== "same") { say(S.back === "other" ? "Space refused: that display isn’t " + dn() + "." : "Space refused: " + dn() + " isn’t back yet."); return; }
        if (CUES[S.preview].k === "unavailable") { say("Can’t take an unavailable Cue live: " + CUES[S.preview].why + ". Output stays off."); return; }
        S.output = "enabled"; S.back = null;
      }
      if (S.output !== "enabled") { say("Congregation output is disabled. Enable Output first; Space takes Preview live once output is on, and also clears Freeze or Blackout."); return; }
      var c = CUES[S.preview];
      if (c.k === "unavailable") { say("Can't take an unavailable Cue live: " + c.why + "."); return; }
      S.program = projectable(c) ? S.preview : (c.k === "wait" ? S.program : null);
      S.live = true; S.frozen = false; S.blackout = false; S.frozenReason = ""; S.blackReason = ""; S.held = null; say("");
      if (S.strip && S.strip.kind !== "fail") S.strip = null;
      S.prior = null;
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
    // ── Faults (MVP-PLAN §6.28, §6.29, §8.1; Decisions 49, 228, 272, 274) ─────────────────────
    // Every fault fails closed: nothing stale or partial is claimed to be on screen, and only an
    // explicit Space (or the primary control) puts content back on the congregation display.
    function disconnect() {
      S.output = "disconnected"; S.back = null; S.live = false; S.frozen = false; S.blackout = false;
      S.program = null; S.held = null; S.frozenReason = ""; S.blackReason = ""; say("");
      S.strip = { kind: "out", state: "danger", icon: "monitor-off", b: dn() + " disconnected", t: "Congregation output stopped and won’t move to this screen. Preview and navigation continue; nothing resumes on its own." };
    }
    function reconnect(same) {
      S.back = same ? "same" : "other"; say("");
      S.strip = same ? { kind: "out", state: "warning", icon: "monitor-check", b: dn() + " is back", t: "Still black. Space (or Resume Output) takes Preview live." } :
        { kind: "out", state: "danger", icon: "monitor-x", b: "A different display was connected", t: "It isn’t " + dfull() + ", so Ordo won’t use it on its own. Reconnect " + dn() + ", or choose a display in Select display…" };
    }
    function failCue(i, why) {
      var c = CUES[i], onAir = i === S.program && S.output === "enabled" && S.live && !S.frozen && !S.blackout;
      c.b = cueLabel(c); c.k = "unavailable"; c.why = why;
      if (onAir) { S.program = null; S.blackReason = "Cue unavailable"; }
      S.strip = { kind: "fail", state: "danger", icon: "circle-off", b: "Cue " + c.item.n + "." + (c.ci + 1) + " (" + c.item.title + ") can’t be shown: " + why,
        t: onAir ? "Program is black. The next valid Cue goes live as usual." : "It stays in the timeline. If it’s reached while live, Program goes black instead of showing it.", close: true };
    }
    function recover(o) {
      S.preview = o.preview; S.prior = { cue: o.program, state: o.state || "LIVE" };
      S.output = o.display === "lost" ? "disconnected" : "enabled"; S.back = null;
      S.live = false; S.frozen = false; S.blackout = false; S.program = null; S.held = null; S.frozenReason = ""; S.blackReason = ""; S.ins = null; say("");
      S.strip = { kind: "recovered", state: "info", icon: "history", b: "Presentation recovered · Congregation output is black",
        t: "Ordo closed unexpectedly at 9:31. Your place and emergency changes are back. " + (o.display === "lost" ? dn() + " wasn’t recognized, so output is disconnected." : "Space takes Preview live.") };
    }
    function dn() { return "Display " + S.cong.n; }
    function dfull() { return dn() + " · " + S.cong.name; }

    // ── Select display (Decisions 49, 50, 202; proposed Decision 292) ─────────────────────────
    // The Settings → Displays picker, in the Insert panel's place, so the operator never leaves
    // Presenter. Choosing a display opens its window black; only Space puts Preview on it.
    var DISPS = { "1": { name: "Built-in display", res: "1366 × 768", pos: "Left", op: true },
      "2": { name: "Epson EB-X51", res: "1920 × 1080", pos: "Right", known: true },
      "3": { name: "Samsung TV", res: "1920 × 1080", pos: "Right" } };
    function present() {
      var out = ["1"];
      if (S.output !== "disconnected" || S.back === "same") out.push(S.cong.n);
      if (S.back === "other") out.push(S.cong.n === "3" ? "2" : "3");
      return out;
    }
    function openDisp() { S.ins = { kind: "disp", pick: S.back ? present().filter(function (n) { return !DISPS[n].op; })[0] : null, identify: false, fit: 0 }; say(""); }
    function dispPanel() {
      var d = S.ins, here = present(), lost = S.output === "disconnected" && S.back !== "same";
      var tiles = ["1", "2", "3"].filter(function (n) { return here.indexOf(n) >= 0 || (n === S.cong.n && lost); }).map(function (n) {
        var x = DISPS[n], un = here.indexOf(n) < 0, on = d.pick === n;
        var chip = x.op ? '<span class="pl-chip">' + icon("monitor") + "Operator display</span>" : un ? statusLabel("unavailable", "Unavailable · congregation display", "ring") :
          on ? '<span class="pl-chip sh-chip-def">' + icon("presentation") + "Use for the congregation</span>" : '<span class="o-meta">' + (x.known ? "Recognised" : "New to Ordo") + "</span>";
        return '<button class="sh-disp' + (x.op ? " is-op" : "") + (un ? " is-un" : "") + (on ? " is-cong" : "") + '" data-act="ins-disp-pick" data-v="' + n + '"' + (x.op || un ? " disabled" : ' aria-pressed="' + on + '"') + ">" +
          (d.identify ? '<span class="sh-ident">' + n + "</span>" : "") + "<b>" + n + " · " + esc(x.name) + '</b><span class="o-meta">' + x.res + " · 16:9 · " + x.pos + "</span>" + chip + "</button>";
      }).join("");
      var p = d.pick && DISPS[d.pick];
      var note = !p ? '<div class="o-notice" data-state="warning">' + icon("unplug") + "<div><b>" + dn() + " isn’t connected.</b> Check its cable and power. Ordo won’t move output onto this screen or pick another display by itself.</div></div>" :
        '<div class="o-notice" data-state="info">' + icon("info") + "<div><b>" + (p.known ? "Display " + d.pick + " is recognised." : "Display " + d.pick + " is new to Ordo.") + "</b> Using it opens its congregation window black and saves it for this computer, as in Settings → Displays. Space then takes Preview live.</div></div>" +
          (p.known ? "" : '<label class="sh-field"><span class="o-label">Output Fit</span><span class="sh-unit"><input type="range" min="0" max="10" step="1" data-qin="fit" value="' + d.fit + '"> ' + d.fit + '%</span><span class="o-meta">Shrinks the whole frame evenly if the edges are cut off. Check the calibration pattern in Settings → Displays outside Mass.</span></label>');
      var state = S.output === "disconnected" ? (S.back === "same" ? dn() + " is back · Program black" : dn() + " disconnected · Program black") : S.output === "enabled" ? "Output enabled on " + dn() : "Congregation output off";
      return '<section class="pr-ins-panel pr-disp-panel" role="dialog" aria-modal="true" aria-labelledby="pr-ins-t"><div class="pr-ins-head"><div class="o-grow"><h2 class="o-title" id="pr-ins-t">Select congregation display</h2>' +
        '<div class="o-meta">' + icon("monitor-cog") + esc(state) + " · the Presentation keeps running</div></div>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-act="ins-disp-close" aria-label="Close" title="Close (Esc)">' + icon("x") + "</button></div>" +
        '<div class="pr-ins-main pr-disp-main sh-form"><div class="o-label">This computer’s displays</div><div class="sh-topo">' + tiles + "</div>" +
        '<div class="sh-inline"><button class="o-btn" data-act="ins-disp-identify"' + (d.identify ? ' aria-pressed="true"' : "") + ">" + icon("scan-eye") + "Identify displays</button>" +
        '<span class="o-meta">Shows each display’s number on it for a few seconds. Mass content is never shown from here.</span></div>' + note + "</div>" +
        '<div class="pr-ins-foot"><span class="o-meta"><span class="o-kbd">Esc</span> close</span><span class="o-grow"></span>' +
        '<button class="o-btn o-btn--quiet" data-act="ins-disp-off">' + icon("monitor-x") + "Turn congregation output off</button>" +
        '<button class="o-btn" data-act="ins-disp-close">Cancel</button>' +
        '<button class="o-btn o-btn--primary" data-act="ins-disp-use"' + (p ? "" : " disabled") + ">" + icon("monitor-check") + (p ? "Use Display " + d.pick : "Use display") + "</button></div></section>";
    }
    function dispAct(a, t) {
      var d = S.ins;
      if (a === "ins-disp") openDisp();
      else if (a === "ins-disp-pick") d.pick = t.dataset.v;
      else if (a === "ins-disp-identify") d.identify = !d.identify;
      else if (a === "ins-disp-close") { S.ins = null; }
      else if (a === "ins-disp-off") { S.ins = null; S.output = "disabled"; S.back = null; S.live = false; S.program = null; S.strip = null; say("Congregation output is off. Enable Output when a display is ready."); }
      else if (a === "ins-disp-use") {
        S.cong = { n: d.pick, name: DISPS[d.pick].name }; DISPS[d.pick].known = true;
        S.ins = null; S.output = "enabled"; S.back = null; S.live = false; S.frozen = false; S.blackout = false; S.program = null; say("");
        S.strip = { kind: "out", state: "info", icon: "monitor-check", b: dn() + " is the congregation display", t: "Its window is open and black. Space takes Preview live." };
      }
      else return false;
      return true;
    }
    function strip() {
      if (!S.strip) return "";
      var x = S.strip;
      return '<div class="pr-strip" role="' + (x.state === "info" ? "status" : "alert") + '" data-state="' + x.state + '">' + icon(x.icon) + "<div><b>" + esc(x.b) + "</b> <span>" + esc(x.t) + "</span></div>" +
        (x.kind === "recovered" && S.prior ? '<span class="o-grow"></span>' + priorFrame() : "") +
        (x.close ? '<span class="o-grow"></span><button class="o-btn o-btn--quiet o-btn--icon" data-act="strip-close" aria-label="Dismiss" title="Dismiss">' + icon("x") + "</button>" : "") + "</div>";
    }
    function enable() { S.output = "enabled"; S.program = null; S.live = false; say("Output enabled. Program is black until you take Preview live."); }

    function primary() {
      if (S.output === "disconnected") return S.back === "same" ? '<button class="o-btn o-btn--primary o-btn--lg" data-act="take">' + icon("monitor-play") + 'Resume Output <span class="o-kbd" style="color:inherit;border-color:currentColor">Space</span></button>' :
        '<button class="o-btn o-btn--lg' + (S.back === "other" ? " o-btn--primary" : "") + '" data-act="ins-disp">' + icon("monitor-cog") + "Select display…</button>";
      if (S.output !== "enabled") return '<button class="o-btn o-btn--primary o-btn--lg" data-act="enable">' + icon("monitor-up") + "Enable Output</button>";
      if (!S.live || S.frozen || S.blackout) return '<button class="o-btn o-btn--primary o-btn--lg" data-act="take">' + icon("play") + 'Take Preview Live <span class="o-kbd" style="color:inherit;border-color:currentColor">Space</span></button>';
      return '<div class="pr-linked">' + statusLabel("live", "LIVE — navigation updates Program") + "</div>";
    }
    function programState() {
      if (S.output === "disconnected") return statusLabel("danger", S.back === "same" ? "DISCONNECTED · display back" : "DISCONNECTED", "square");
      if (S.output !== "enabled") return statusLabel("disabled", "Output disabled", "square");
      if (S.blackout) return statusLabel("blackout", "BLACKOUT", "square");
      if (S.frozen) return statusLabel("freeze", S.frozenReason ? "FROZEN · " + S.frozenReason : "FROZEN", "diamond");
      if (S.live && S.blackReason) return statusLabel("blackout", "LIVE · black · " + S.blackReason, "square");
      if (S.live) return statusLabel("live", "LIVE");
      return statusLabel("blackout", "BLACK · not live", "square");
    }
    function programSlide() {
      if (S.output === "disconnected") return '<div class="o-slide"><div class="o-slide-state" data-state="danger">' + icon("monitor-off") + "<div>" + dn() + (S.back === "same" ? " is back · black" : " disconnected") + '<br><span style="font-weight:400">' + (S.back === "same" ? "Nothing shows until you resume" : "Ordo can’t tell what the congregation sees") + "</span></div></div></div>";
      if (S.output !== "enabled") return '<div class="o-slide"><div class="o-slide-state"><div>No congregation window<br><span style="font-weight:400">Output is disabled</span></div></div></div>';
      return '<div class="o-slide"><div class="o-canvas-host">' + canvas(S.program == null ? null : S.frozen && S.held ? S.held : CUES[S.program]).replace('class="o-canvas"', 'class="o-canvas"' + (S.blackout ? ' data-dim="true"' : "")) + "</div></div>";
    }
    // The pre-interruption Program frame, as private context only (Decision 228)
    function priorFrame() {
      if (!S.prior) return "";
      var c = CUES[S.prior.cue];
      return '<div class="pr-prior"><span class="pr-mini">' + slide(c) + '</span><div><b>Before the interruption</b><div class="o-meta">' + esc(S.prior.state) + " on " + esc(c.item.title) + " · " + c.item.n + "." + (c.ci + 1) + " · not on screen now</div></div></div>";
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
      if (S.output === "disconnected") return '<div class="pr-outstat">' + (S.back === "same" ? statusLabel("warning", dn() + " back · output not resumed", "diamond") : statusLabel("danger", S.back === "other" ? "Unrecognized display connected" : "Congregation output disconnected", "square")) + '<span class="o-meta">' + dfull() + ' · ' + (S.back === "same" ? "recognized" : "last seen 9:14") + "</span></div>";
      if (S.output !== "enabled") return '<div class="pr-outstat">' + statusLabel("disabled", "Congregation output disabled", "square") + '<span class="o-meta">' + dfull() + ' · 1920×1080 ready</span></div>';
      return '<div class="pr-outstat">' + statusLabel("live", "Output enabled") + '<span class="o-meta">' + dfull() + ' · 1920×1080</span></div>';
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
          '<div class="o-caption"><span class="o-n">' + c.item.n + "." + (c.ci + 1) + '</span><span class="o-t">' + esc(KIND_LABEL[c.k] === "Unavailable" ? "Unavailable · " + c.b : cueLabel(c)) + "</span>" + (c.edited ? '<span class="pl-chip">' + icon("pencil") + "Edited</span>" : "") + onAir + "</div></button>";
      }
      var lead = c.k === "trigger" ? '<span class="pr-who">' + esc(c.who) + "</span>" :
        c.k === "wait" ? icon("hourglass") : c.k === "hold" ? icon("square") :
        c.k === "unavailable" ? icon("circle-off") : '<span class="pr-mini">' + slide(c) + "</span>";
      var kind = c.k === "unavailable" ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : '<span class="o-meta">' + KIND_LABEL[c.k] + "</span>";
      return '<button class="o-row pr-row" data-cue="' + c.i + '"' + (cur ? ' aria-selected="true"' : "") + airAttr + '><span class="o-n o-meta">' + c.item.n + "." + (c.ci + 1) + "</span>" + lead +
        '<span class="o-grow o-ellipsis">' + esc(cueLabel(c)) + "</span>" + (c.edited ? '<span class="pl-chip">' + icon("pencil") + "Edited</span>" : "") + kind + onAir + "</button>";
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
      var html = preflight() + '<div class="pr-tl-tools"><button class="o-btn" data-act="ins-open">' + icon("plus") + "Insert after " + esc(CUES[S.preview].item.title) + ' <span class="o-kbd">Insert</span></button>' +
        '<button class="o-btn" data-act="ins-edit"' + (projectable(CUES[S.preview]) && !CUES[S.preview].img ? "" : ' disabled title="Ctrl+E edits projectable slides"') + ">" + icon("pencil") + 'Edit slide <span class="o-kbd">Ctrl+E</span></button></div>';
      ITEMS.forEach(function (it) {
        var open = !S.collapsed[it.n];
        var bad = it.cues.filter(function (c) { return c.k === "unavailable"; }).length;
        html += '<section class="pr-group" data-item="' + it.n + '"><button class="pr-group-head" data-toggle="' + it.n + '" aria-expanded="' + open + '">' + icon(open ? "chevron-down" : "chevron-right") +
          '<span class="o-n o-meta">' + it.n + "</span><b>" + esc(it.title) + "</b>" + (it.song ? '<span class="o-meta">' + esc(it.song) + "</span>" : "") +
          (it.inserted ? '<span class="pl-chip">' + icon("zap") + (it.local ? "Inserted live · Mass-local Song" : "Inserted live") + "</span>" : "") +
          (it.cues.some(function (c) { return c.edited; }) ? '<span class="pl-chip">' + icon("pencil") + "Edited live · Mass-local</span>" : "") + '<span class="o-grow"></span>' + (bad ? '<span class="o-status" data-state="unavailable">' + icon("circle-off") + bad + " unavailable</span>" : "") +
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
      M.sections.forEach(function (sec) {
        html += '<div class="o-section-label">' + esc(sec.title) + "</div>";
        sec.items.forEach(function (it) {
          html += '<button class="o-row pr-rail-row" data-jump="' + it.n + '"' + (it.n === cur ? ' aria-current="true"' : "") + '><span class="o-n o-meta">' + it.n + '</span><span class="o-ellipsis">' + esc(it.title) + "</span>" + (it.inserted ? '<span class="pl-chip">Inserted</span>' : "") + (it.cues.some(function (c) { return c.edited; }) ? '<span class="pl-chip">Edited</span>' : "") + "</button>";
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

    // ── Emergency insert (MVP-PLAN §8.5, Decisions 12, 258; Decision 290) ─────────────
    // Insert Freezes Program and opens one Insert panel over the Cue timeline, leaving Program,
    // Preview, Freeze and Blackout visible. Song, Response sequence and Text or image are tabs of
    // the same panel; Song and Response share the keyboard-first search, list and preview. Saving
    // adds a Mass-local item right after the current Mass item and selects its first projectable
    // Cue in Preview; Program stays frozen until Space.
    var KINDS = [["song", "music", "Song", "S"], ["resp", "messages-square", "Response sequence", "R"], ["general", "presentation", "Text or image", "T"]];
    function insAnchor() { return CUES[S.preview].item; }
    function openInsert(kind) {
      if (S.output === "enabled" && S.live && !S.frozen && !S.blackout) { S.frozen = true; S.frozenReason = "inserting"; }
      else if (S.frozen && S.frozenReason === "Insert ready") S.frozenReason = "inserting";
      S.ins = { kind: kind || "song", q: "", active: 0, create: null, mode: "Text", h: "", text: "" }; say("");
    }
    function closeInsert() {
      S.ins = null;
      if (S.frozenReason === "inserting") S.frozenReason = "";
      say(S.frozen ? "Insert cancelled. Program is still frozen; Space or F takes Preview live." : "Insert cancelled.");
    }
    function norm(s) { return String(s || "").toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, ""); }
    // Songs in two groups: the plan's choir's repertoire first, then the rest of the Library.
    function songResults(q) {
      q = norm(q).trim();
      var out = [];
      QI_SONGS.forEach(function (s) {
        if (!q) { out.push({ x: s, rank: 0, why: "" }); return; }
        var t = norm(s.t), al = s.alias.filter(function (a) { return norm(a).indexOf(q) >= 0; })[0], fl = norm(s.first).indexOf(q) >= 0;
        var rank = t.indexOf(q) === 0 ? 0 : t.indexOf(q) > 0 ? 1 : al ? 2 : fl ? 3 : -1;
        if (rank >= 0) out.push({ x: s, rank: rank, why: rank === 2 ? "Also called " + al : rank === 3 ? "First line: " + s.first : "" });
      });
      out.sort(function (a, b) { return (b.x.rep - a.x.rep) || (a.rank - b.rank); });
      return out;
    }
    function respResults(q) {
      q = norm(q).trim();
      return QI_RESP.filter(function (r) { return !q || norm(r.t).indexOf(q) >= 0 || r.cues.some(function (c) { return norm(c.b).indexOf(q) >= 0; }); }).map(function (r) { return { x: r, why: "" }; });
    }
    function results() { return S.ins.kind === "song" ? songResults(S.ins.q) : S.ins.kind === "resp" ? respResults(S.ins.q) : []; }
    // Insert a new Mass-local item after the anchor item; keep Program on its held Cue.
    function insertItem(it) {
      var anchor = insAnchor(), sec = M.sections[anchor.section], at = sec.items.indexOf(anchor) + 1, p = anchor.first + anchor.cues.length;
      it.inserted = true; sec.items.splice(at, 0, it); reindex();
      if (S.program != null && S.program >= p) S.program += it.cues.length;
      var first = it.cues.filter(projectable)[0] || it.cues[0];
      S.preview = first.i; S.follow = true; S.collapsed = {};
      S.ins = null; S.frozenReason = S.frozen ? "Insert ready" : "";
      say("Inserted " + it.title + " after " + anchor.title + ". Saved to this Mass plan only. " + (S.frozen ? "Program is still frozen; Space takes it live." : S.output === "enabled" ? "Space takes it live." : ""));
    }
    function songItem(s) {
      return { title: s.t, song: s.local ? "Typed for this Mass" : s.a, type: "Song", local: !!s.local, cues: [{ k: "song-id", t: s.t, s: s.local ? "" : s.a }].concat(s.slides.map(function (b, i) { return { k: "lyrics", h: s.local ? "" : "Bersikulo " + (i + 1), b: b }; })) };
    }
    function slidesOf(text) { return String(text).split(/\n\s*\n/).map(function (x) { return x.trim(); }).filter(Boolean); }
    function chosenCues() {
      var d = S.ins;
      if (d.create) return [{ k: "song-id", t: d.create.title || "Untitled", s: "" }].concat(slidesOf(d.create.text).map(function (b) { return { k: "lyrics", h: "", b: b }; }));
      if (d.kind === "general") return [d.mode === "Image" ? { k: "general", img: "fiesta-poster.jpg", b: "" } : { k: "general", h: d.h, b: d.text || " ", f: "Parokya ni San Roque" }];
      var r = results()[Math.min(d.active, results().length - 1)];
      return r ? (d.kind === "song" ? songItem(r.x).cues : r.x.cues) : [];
    }
    function searchList() {
      var d = S.ins, r = results(), act = Math.min(d.active, Math.max(0, r.length - 1)), html = "", group = null;
      r.forEach(function (o, i) {
        if (d.kind === "song") {
          var g = o.x.rep ? M.choir + "’s repertoire" : "Other Songs in the Library";
          if (g !== group) { group = g; html += '<div class="o-section-label pr-qi-group">' + esc(g) + "</div>"; }
        }
        var meta = d.kind === "song" ? esc(o.x.a) + " · " + esc(o.x.lang) + " · " + o.x.slides.length + " slides" : o.x.cues.length + " Cues · " + esc(o.x.lang);
        html += '<div class="o-row pr-qi-row" role="option" id="qi-' + i + '" data-qi="' + i + '" aria-selected="' + (i === act) + '"><div class="o-grow"><b>' + esc(o.x.t) + '</b><span class="o-meta">' + meta + (o.why ? " · " + esc(o.why) : "") + "</span></div>" +
          (d.kind === "song" && !o.x.rep && i === act ? '<span class="o-meta">Not in ' + esc(M.choir) + "’s repertoire</span>" : "") + "</div>";
      });
      if (!r.length) html = d.kind === "song" ? '<div class="pr-qi-none"><b>No Song matches “' + esc(d.q) + '”.</b><span class="o-meta">Type it for this Mass; it stays in this plan until someone adds it to the Library after Mass.</span>' +
        '<button class="o-btn" data-act="ins-create">' + icon("plus") + "Type “" + esc(d.q) + '” for this Mass <span class="o-kbd">Enter</span></button></div>' :
        '<div class="pr-qi-none"><b>Nothing in Responses &amp; Prayers matches “' + esc(d.q) + '”.</b><span class="o-meta">Use <b>Text or image</b> to type it.</span></div>';
      var ph = d.kind === "song" ? "Title, other name or first line" : "Title or words of a response";
      return '<label class="o-field pr-qi-search">' + icon("search") + '<input class="pl-input" data-qin="q" value="' + esc(d.q) + '" placeholder="' + ph + '" role="combobox" aria-expanded="true" aria-controls="qi-list" aria-activedescendant="' + (r.length ? "qi-" + act : "") + '" autocomplete="off"></label>' +
        '<div class="pr-qi-list" id="qi-list" role="listbox" aria-label="' + (d.kind === "song" ? "Songs" : "Responses and Prayers") + '">' + html + "</div>";
    }
    function form() {
      var d = S.ins;
      if (d.create) return '<button class="o-btn o-btn--quiet pr-ins-back" data-act="ins-back">' + icon("arrow-left") + "Back to search</button>" +
        '<label class="sh-field"><span class="o-label">Title</span><input class="pl-input" data-qin="create.title" value="' + esc(d.create.title) + '"></label>' +
        '<label class="sh-field"><span class="o-label">Lyrics</span><textarea class="pl-input" rows="8" data-qin="create.text" placeholder="Type the lyrics. A blank line starts a new slide.">' + esc(d.create.text) + "</textarea>" +
        '<span class="o-meta">Saved in this Mass plan only; after Mass it can be added to the Library with a review.</span></label>';
      return '<div class="o-segmented" role="group" aria-label="Kind">' + ["Text", "Image"].map(function (m) { return '<button data-act="ins-mode" data-v="' + m + '" aria-pressed="' + (d.mode === m) + '">' + m + "</button>"; }).join("") + "</div>" +
        (d.mode === "Image" ? '<div class="pr-ins-img">' + icon("image") + '<div><b>fiesta-poster.jpg</b><br><span class="o-meta">1920×1080 · fills the slide · from Pictures</span></div><button class="o-btn">Choose another…</button></div>' :
          '<label class="sh-field"><span class="o-label">Heading</span><input class="pl-input" data-qin="h" value="' + esc(d.h) + '" placeholder="Optional"></label>' +
          '<label class="sh-field"><span class="o-label">Text</span><textarea class="pl-input" rows="6" data-qin="text" placeholder="What the congregation should read">' + esc(d.text) + "</textarea></label>");
    }
    function insertPanel() {
      if (!S.ins) return "";
      if (S.ins.kind === "edit") return editPanel();
      if (S.ins.kind === "disp") return dispPanel();
      var d = S.ins, a = insAnchor(), cues = chosenCues(), searching = (d.kind === "song" || d.kind === "resp") && !d.create;
      var ok = d.create ? d.create.title.trim() && slidesOf(d.create.text).length : d.kind === "general" ? d.mode === "Image" || d.text.trim() : results().length > 0;
      var label = d.create ? "Insert typed Song" : d.kind === "song" ? "Insert Song" : d.kind === "resp" ? "Insert Response sequence" : "Insert slide";
      var r = searching ? results()[Math.min(d.active, results().length - 1)] : null;
      return '<section class="pr-ins-panel" role="dialog" aria-modal="true" aria-labelledby="pr-ins-t"><div class="pr-ins-head"><div class="o-grow"><h2 class="o-title" id="pr-ins-t">Insert</h2>' +
        '<div class="o-meta">' + icon("corner-down-right") + "After <b>" + esc(a.title) + "</b> · " + esc(M.sections[a.section].title) + " · " + (S.frozen ? "Program frozen" : S.output === "enabled" ? "Program not live" : "Rehearsal: output disabled") + "</div></div>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-act="ins-close" aria-label="Close" title="Close (Esc)">' + icon("x") + "</button></div>" +
        '<div class="o-tabs pr-ins-tabs" role="tablist">' + KINDS.map(function (k) { return '<button role="tab" data-act="ins-kind" data-v="' + k[0] + '" aria-selected="' + (d.kind === k[0]) + '">' + icon(k[1]) + k[2] + ' <span class="o-kbd">Alt+' + k[3] + "</span></button>"; }).join("") + "</div>" +
        '<div class="pr-ins-split"><div class="pr-ins-main sh-form">' + (searching ? searchList() : form()) + "</div>" +
        '<div class="pr-ins-prev"><div class="o-label">' + (r ? esc(r.x.t) : "Preview") + '</div><div class="pr-ins-thumbs">' + cues.slice(0, 4).map(function (c) { return projectable(c) ? slide(c) : '<div class="pr-private"><span class="pr-who">' + esc(c.who || "") + "</span><div><b>" + esc(c.b) + '</b><div class="o-meta">Operator-only · not projected</div></div></div>'; }).join("") + "</div></div></div>" +
        '<div class="pr-ins-foot"><span class="o-meta">' + (searching ? '<span class="o-kbd">↑</span><span class="o-kbd">↓</span> move · <span class="o-kbd">Enter</span> insert · ' : "") + '<span class="o-kbd">Esc</span> ' + (d.create ? "back" : "close") + " · saves to this Mass plan</span>" +
        '<span class="o-grow"></span><button class="o-btn" data-act="ins-close">Cancel</button><button class="o-btn o-btn--primary" data-act="ins-save"' + (ok ? "" : " disabled") + ">" + label + "</button></div></section>";
    }
    function insSave() {
      var d = S.ins;
      if (d.create) { var sl = slidesOf(d.create.text); insertItem(songItem({ t: d.create.title.trim(), slides: sl, local: true })); return; }
      if (d.kind === "general") { insertItem({ title: d.mode === "Image" ? "Image · fiesta-poster.jpg" : d.h.trim() || "Announcement", type: "General Presentation", cues: [d.mode === "Image" ? { k: "general", img: "fiesta-poster.jpg", b: "" } : { k: "general", h: d.h.trim(), b: d.text.trim(), f: "Parokya ni San Roque" }] }); return; }
      var r = results()[Math.min(d.active, results().length - 1)]; if (!r) return;
      if (d.kind === "song") insertItem(songItem(r.x));
      else insertItem({ title: r.x.t, type: "Response sequence", cues: r.x.cues.map(function (c) { return Object.assign({}, c); }) });
    }
    function insAct(a, t) {
      if (a.indexOf("ins-ed") === 0) return edAct(a, t);
      if (a.indexOf("ins-disp") === 0) return dispAct(a, t);
      if (a === "ins-open") openInsert();
      else if (a === "ins-close") closeInsert();
      else if (a === "ins-kind") { S.ins.kind = t.dataset.v; S.ins.q = ""; S.ins.active = 0; S.ins.create = null; }
      else if (a === "ins-mode") S.ins.mode = t.dataset.v;
      else if (a === "ins-create") S.ins.create = { title: (S.ins.q || "").trim().replace(/^\w/, function (c) { return c.toUpperCase(); }), text: "" };
      else if (a === "ins-back") S.ins.create = null;
      else if (a === "ins-save") insSave();
      else return false;
      return true;
    }
    function insKey(e) {
      if (S.ins.kind === "edit") return edKey(e);
      if (S.ins.kind === "disp") { if (e.key === "Escape") { e.preventDefault(); S.ins = null; render(); app.focus({ preventScroll: true }); } return; }
      var k = e.key, d = S.ins;
      if (k === "Escape") { e.preventDefault(); if (d.create) d.create = null; else closeInsert(); render(); return; }
      if (e.altKey) { var m = { s: "song", r: "resp", t: "general" }[k.toLowerCase()]; if (m) { e.preventDefault(); d.kind = m; d.q = ""; d.active = 0; d.create = null; render(); } return; }
      if ((d.kind === "song" || d.kind === "resp") && !d.create) {
        var n = results().length;
        if (k === "ArrowDown" || k === "ArrowUp") { e.preventDefault(); d.active = Math.max(0, Math.min(n - 1, d.active + (k === "ArrowDown" ? 1 : -1))); render(); }
        else if (k === "Enter") { e.preventDefault(); if (n) insSave(); else if (d.kind === "song" && d.q.trim()) insAct("ins-create"); render(); }
      }
      // Other keys belong to the focused field; no single-key Presenter command runs (MVP-PLAN §8.2).
    }

    // ── Emergency edit (MVP-PLAN §8.5, Decision 12; Decision 291) ────────────────────
    // Ctrl+E Freezes Program and opens the selected projectable Cue in the same panel as Insert.
    // Program keeps its frame; each keystroke autosaves a Mass-local override and updates
    // Preview only. A reused Song section updates every linked occurrence in this Mass unless
    // the operator limits it to this one. Space takes the corrected Preview live.
    var ED_FIELDS = {
      "song-id": [["t", "Title"], ["s", "Credit line"]], celebration: [["t", "Title"], ["s", "Subtitle"]],
      lyrics: [["h", "Section label"], ["b", "Lyrics"]], psalm: [["h", "Heading"], ["b", "Text"], ["f", "Reference"]],
      response: [["h", "Heading"], ["b", "Text"]], "reading-id": [["b", "Title"], ["f", "Reference"]],
      reading: [["b", "Text"], ["f", "Reference"]], general: [["h", "Heading"], ["b", "Text"], ["f", "Footer"]]
    };
    function edFields(c) { return c.img ? [] : ED_FIELDS[c.k] || [["b", "Text"]]; }
    function pick(c, fs) { var o = {}; fs.forEach(function (f) { o[f[0]] = c[f[0]] == null ? "" : c[f[0]]; }); return o; }
    function edSource(c) {
      var it = c.item;
      if (it.local) return ["file-pen", "Song typed for this Mass · already Mass-local"];
      if (it.inserted && it.type !== "Song") return ["file-pen", "Inserted live · already Mass-local"];
      if (it.type === "Song") return ["library", "From the Library Arrangement " + it.song + " · the Library isn’t changed"];
      if (it.type === "Reading" || it.type === "Psalm") return ["book-open", "From the imported liturgy · the imported text isn’t changed"];
      if (it.type === "Response sequence") return ["messages-square", "From Responses & Prayers · the Library entry isn’t changed"];
      return ["presentation", "General Presentation in this Mass"];
    }
    function openEdit() {
      var c = CUES[S.preview];
      if (c.k === "unavailable") { say("Can’t edit an unavailable Cue: " + c.why + ". Use Insert to put something in its place."); return; }
      if (!projectable(c)) { say("Ctrl+E edits projectable slides; this " + KIND_LABEL[c.k].toLowerCase() + " is never shown."); return; }
      if (c.img) { say("A picture can’t be edited live. Use Insert to add another one."); return; }
      if (S.output === "enabled" && S.live && !S.frozen && !S.blackout) { S.frozen = true; S.frozenReason = "editing"; }
      else if (S.frozen && /ready$/.test(S.frozenReason)) S.frozenReason = "editing";
      if (S.frozen && S.program != null && !S.held) S.held = Object.assign({}, CUES[S.program]);   // Program keeps its frame
      var fs = edFields(c);
      var links = c.item.type === "Song" ? CUES.filter(function (x) { return x.k === c.k && x.item.song === c.item.song && x.h === c.h && x.b === c.b; }).map(function (x) { return x.i; }) : [c.i];
      var before = {}; links.forEach(function (i) { before[i] = pick(CUES[i], fs); if (!CUES[i].base) CUES[i].base = pick(CUES[i], fs); });
      S.ins = { kind: "edit", cue: c.i, fs: fs, links: links, before: before, draft: pick(c, fs), scope: "all", saved: false }; say("");
    }
    function edTargets(d) { return d.scope === "one" ? [d.cue] : d.links; }
    function applyEdit() {
      var d = S.ins, on = edTargets(d);
      d.links.forEach(function (i) {
        var c = CUES[i], v = on.indexOf(i) >= 0 ? d.draft : d.before[i];
        d.fs.forEach(function (f) { c[f[0]] = v[f[0]]; });
        c.edited = d.fs.some(function (f) { return c[f[0]] !== c.base[f[0]]; });
      });
      d.saved = true; d.dirty = true;
    }
    function closeEdit() {
      var d = S.ins, c = CUES[d.cue], n = edTargets(d).length;
      S.ins = null;
      if (S.frozenReason === "editing") S.frozenReason = d.dirty ? "Edit ready" : "";
      if (d.dirty && !c.edited) { say("Back to the original words. Saved to this Mass plan. " + (S.frozen ? "Program is still frozen; Space takes it live." : "")); return; }
      if (!c.edited) { say(S.frozen ? "No change. Program is still frozen; Space or F takes Preview live." : ""); return; }
      say("Corrected " + cueLabel(c).split(" · ")[0] + (n > 1 ? " in " + n + " places" : "") + ". Saved to this Mass plan only. " +
        (S.frozen ? "Program is still frozen; Space takes the correction live." : S.output === "enabled" ? "Space takes it live." : ""));
    }
    function editPanel() {
      var d = S.ins, c = CUES[d.cue], src = edSource(c), base = Object.assign({}, c, c.base), n = d.links.length;
      var fields = d.fs.map(function (f) {
        var v = esc(d.draft[f[0]]), big = f[0] === "b" && c.k !== "reading-id";
        return '<label class="sh-field"><span class="o-label">' + f[1] + "</span>" + (big ? '<textarea class="pl-input" rows="' + (c.k === "reading" ? 7 : 4) + '" data-qin="ed.' + f[0] + '">' + v + "</textarea>" : '<input class="pl-input" data-qin="ed.' + f[0] + '" value="' + v + '">') + "</label>";
      }).join("");
      var scope = n > 1 ? '<fieldset class="pr-ed-scope"><legend class="o-label">Apply to</legend>' +
        '<label class="o-radio"><input type="radio" name="ed-scope-' + UID + '" data-act="ins-ed-scope" data-v="all"' + (d.scope === "all" ? " checked" : "") + "><span><b>All " + n + " “" + esc(c.h || cueLabel(c)) + "” slides in this Mass</b><span class=\"o-meta\">Cues " + d.links.map(function (i) { return CUES[i].item.n + "." + (CUES[i].ci + 1); }).join(", ") + " · the same section sung again</span></span></label>" +
        '<label class="o-radio"><input type="radio" name="ed-scope-' + UID + '" data-act="ins-ed-scope" data-v="one"' + (d.scope === "one" ? " checked" : "") + "><span><b>Only this slide</b><span class=\"o-meta\">Cue " + c.item.n + "." + (c.ci + 1) + " · the others keep the Library words</span></span></label></fieldset>" : "";
      var state = S.frozen ? "Program frozen" : S.output === "enabled" ? "Program not live" : "Rehearsal: output disabled";
      return '<section class="pr-ins-panel pr-ed-panel" role="dialog" aria-modal="true" aria-labelledby="pr-ins-t"><div class="pr-ins-head"><div class="o-grow"><h2 class="o-title" id="pr-ins-t">Edit slide</h2>' +
        '<div class="o-meta">' + icon("pencil") + "<b>" + esc(c.item.title) + "</b> · Cue " + c.item.n + "." + (c.ci + 1) + " · " + KIND_LABEL[c.k] + " · " + state + "</div>" +
        '<div class="o-meta">' + icon(src[0]) + esc(src[1]) + "</div></div>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-act="ins-ed-done" aria-label="Done" title="Done (Esc)">' + icon("x") + "</button></div>" +
        '<div class="pr-ins-split"><div class="pr-ins-main sh-form">' + fields + scope + "</div>" +
        '<div class="pr-ins-prev"><div class="o-label">Now in Preview</div>' + slide(c) +
        '<div class="o-label">' + (c.item.type === "Song" && !c.item.local ? "Library" : "Before") + (c.edited ? "" : " · unchanged") + "</div>" + slide(base) +
        (S.frozen ? '<span class="o-meta">Program keeps showing the old slide until you take Preview live.</span>' : "") + "</div></div>" +
        '<div class="pr-ins-foot"><span class="o-meta">' + (d.saved ? icon("check") + "Saved to this Mass plan · Mass-local" : "Changes save as you type, to this Mass plan only") + ' · <span class="o-kbd">Esc</span> done</span>' +
        '<span class="o-grow"></span><button class="o-btn" data-act="ins-ed-revert"' + (c.edited || d.links.some(function (i) { return CUES[i].edited; }) ? "" : " disabled") + ">" + icon("undo-2") + (c.item.local || c.item.inserted ? "Undo edits" : c.item.type === "Song" ? "Back to Library words" : c.item.type === "Reading" || c.item.type === "Psalm" ? "Back to imported text" : "Back to original") + "</button>" +
        '<button class="o-btn o-btn--primary" data-act="ins-ed-done">Done <span class="o-kbd" style="color:inherit;border-color:currentColor">Ctrl+Enter</span></button></div></section>';
    }
    function edAct(a, t) {
      var d = S.ins;
      if (a === "ins-edit") openEdit();
      else if (a === "ins-ed-scope") { d.scope = t.dataset.v; applyEdit(); }
      else if (a === "ins-ed-revert") { d.draft = pick(Object.assign({}, CUES[d.cue], CUES[d.cue].base), d.fs); d.links.forEach(function (i) { d.before[i] = pick(Object.assign({}, CUES[i], CUES[i].base), d.fs); }); applyEdit(); }
      else if (a === "ins-ed-done") closeEdit();
      else return false;
      return true;
    }
    function edKey(e) {
      if (e.key === "Escape" || (e.key === "Enter" && (e.ctrlKey || e.metaKey))) { e.preventDefault(); closeEdit(); render(); app.focus({ preventScroll: true }); }
      // Everything else belongs to the focused field; no single-key Presenter command runs.
    }

    function render() {
      var l = layout(), pinned = railPinned();
      app.style.setProperty("--o-scale", S.scale);
      app.style.setProperty("--pr-band-mon", Math.floor(H * 0.3 * 16 / 9) + "px");
      app.dataset.layout = l;
      var header = '<header class="pr-header"><div class="pr-session"><b>' + esc(MASS.title) + '</b><span class="o-meta">' + esc(MASS.date) + "</span></div>" +
        '<span class="o-grow"></span><span class="o-meta pr-clock">9:14</span>' +
        (S.output === "enabled" ? statusLabel("live", dn() + " connected") : S.output === "disconnected" ? (S.back === "same" ? statusLabel("warning", dn() + " back · not resumed", "diamond") : statusLabel("danger", dn() + " disconnected", "square")) : statusLabel("disabled", dn() + " ready · not enabled", "square")) +
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
      var fk = document.activeElement && app.contains(document.activeElement) && document.activeElement.dataset ? document.activeElement.dataset.qin : null, caret = fk && document.activeElement.selectionStart;
      app.innerHTML = header + strip() + '<div class="o-body pr-body">' + body + insertPanel() + "</div>" + overlay;
      icons();
      if (S.ins) {
        var dk = app.querySelector(".pr-dock"), pn = app.querySelector(".pr-ins-panel");
        if (pn) pn.style.right = (dk ? dk.offsetWidth : 0) + "px";
        var f = app.querySelector('[data-qin="' + (fk || "") + '"]') || app.querySelector('[data-qin="ed.b"], [data-qin="q"]') || app.querySelector("[data-qin]") || pn;
        if (f) { f.focus({ preventScroll: true }); if (caret != null && f.setSelectionRange) try { f.setSelectionRange(caret, caret); } catch (e) { /* not text */ } }
        var on = app.querySelector('.pr-qi-row[aria-selected="true"]'); if (on) on.scrollIntoView({ block: "nearest" });
      }
      // Keep the active Cue in view by scrolling the timeline only, never the page.
      var tl = app.querySelector(".pr-timeline"), cur = tl && tl.querySelector('[data-cue="' + S.preview + '"]');
      if (tl) tl.scrollTop = kept;                     // F, B, Space and clicks never move the timeline
      if (cur && S.follow) {
        // Keep Preview in view, and the Program Cue too when both fit (Freeze or Blackout lets them differ).
        var k = tl.getBoundingClientRect().height / tl.clientHeight || 1, pad = 104;   // clears the pinned Insert row and group head
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
        var t = e.target.closest("[data-cue],[data-act],[data-view],[data-toggle],[data-jump],[data-qi]");
        if (!t) return;
        if (t.dataset.qi) { S.ins.active = +t.dataset.qi; render(); return; }
        if (t.dataset.act && t.dataset.act.indexOf("ins-") === 0) { if (!t.disabled) insAct(t.dataset.act, t); render(); if (!S.ins) app.focus({ preventScroll: true }); return; }
        if (S.ins && !t.closest(".pr-dock, .pr-band-controls")) return;   // the Output Dock stays usable by mouse
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
          else if (a === "strip-close") S.strip = null;
          else if (a === "pin") { S.railPref = railPinned() ? "collapsed" : "pinned"; S.railOpen = false; }
        }
        render(); app.focus({ preventScroll: true });
      });
      app.addEventListener("input", function (e) {
        var k = e.target.dataset && e.target.dataset.qin; if (!k || !S.ins) return;
        if (k.indexOf("ed.") === 0) { S.ins.draft[k.slice(3)] = e.target.value; applyEdit(); render(); return; }
        if (k.indexOf("create.") === 0) S.ins.create[k.slice(7)] = e.target.value; else S.ins[k] = e.target.value;
        if (k === "q") S.ins.active = 0; render();
      });
      app.addEventListener("keydown", function (e) {
        if (S.ins) { insKey(e); return; }
        var k = e.key, used = true;
        if (k === "Insert") { openInsert(); render(); e.preventDefault(); return; }
        if ((e.ctrlKey || e.metaKey) && (k === "e" || k === "E")) { openEdit(); render(); e.preventDefault(); return; }
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
      disconnect: function () { disconnect(); render(); }, reconnect: function (same) { reconnect(same); render(); },
      failCue: function (i, why) { failCue(i, why); render(); }, recover: function (o) { recover(o); S.follow = true; render(); },
      go: function (i) { S.follow = true; select(i); render(); }, take: function () { take(); render(); },
      selectDisplay: function (o) { openDisp(); Object.keys(o || {}).forEach(function (k) { S.ins[k] = o[k]; }); render(); },
      setMode: function (m) { S.mode = m; render(); },
      insert: function (kind, o) { openInsert(kind); Object.keys(o || {}).forEach(function (k) { S.ins[k] = o[k]; }); render(); },
      edit: function (cue, o) { S.preview = cue; S.follow = true; openEdit(); o = o || {}; if (S.ins) { if (o.scope) S.ins.scope = o.scope; Object.keys(o.draft || {}).forEach(function (k) { S.ins.draft[k] = o.draft[k]; }); if (o.draft || o.scope) applyEdit(); if (o.done) closeEdit(); } render(); },
      insertSong: function (title) { openInsert(); insertItem(songItem(QI_SONGS.filter(function (x) { return x.t === title; })[0])); render(); },
      state: S, render: render, app: app
    };
  }

  // ── Preparation shell and Mass Planning (Decisions 170-178, 250, 257) ───────────────────────
  var NAV = [["Masses", "calendar-days"], ["Library", "library"], ["Themes", "palette"], ["Transfer & Backup", "hard-drive-download"]];
  var NAV_FOOT = [["Diagnostics", "activity"], ["Help", "circle-help"], ["Settings", "settings"]];
  // Each rail destination links to the mockup page that shows it.
  var PAGE_OF = { Masses: "masses-index.html", Library: "song-library-master-detail-v2.html", Themes: "themes.html", "Transfer & Backup": "transfer-backup.html",
    Diagnostics: "diagnostics.html", Help: "help.html", Settings: "settings.html" };
  function navrail(active) {
    function a(n) { return '<a href="' + (PAGE_OF[n[0]] || "#") + '" data-dest="' + esc(n[0]) + '" aria-current="' + (n[0] === active ? "page" : "false") + '">' + icon(n[1], "o-icon-20") + "<span>" + esc(n[0]) + "</span></a>"; }
    return '<nav class="o-navrail" aria-label="Global navigation">' + NAV.map(a).join("") + '<span class="o-spacer"></span>' + NAV_FOOT.map(a).join("") + "</nav>";
  }

  // The Mass planner itself lives in ordo-planner.js, which adds OrdoKit.planner.

  window.OrdoKit = {
    MASS: MASS, ITEMS: ITEMS, CUES: CUES, QI_SONGS: QI_SONGS, projectable: projectable, cueLabel: cueLabel, KIND_LABEL: KIND_LABEL,
    esc: esc, icon: icon, canvas: canvas, slide: slide, statusLabel: statusLabel, icons: icons, fit: fit, presenter: presenter,
    navrail: navrail
  };
  document.addEventListener("DOMContentLoaded", icons);
})();
