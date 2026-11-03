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
        { title: "Entrance Song", song: "Daygon Ta", arr: "Koro San Roque", type: "Song", cues: [
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
        { title: "Offertory Song", song: "Ihalad Namo", arr: "Koro San Roque", type: "Song", cues: [
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
  // Other Arrangements of the Songs planned in this Mass (Decision 293).
  var QI_VERSIONS = {
    "Daygon Ta": [
      { t: "Daygon Ta", a: "Youth Choir", lang: "Cebuano", rep: false, note: "Starts on the Koro · 3 verses", secs: [["Koro", "Daygon, daygon ta\nang Ginoo sa kahitas-an"], ["Bersikulo 1", "Daygon ta ang Ginoo\nsa tanang kasingkasing"], ["Koro", "Daygon, daygon ta\nang Ginoo sa kahitas-an"], ["Bersikulo 2", "Siya ang atong bato\nug atong kalig-on"], ["Koro", "Daygon, daygon ta\nang Ginoo sa kahitas-an"], ["Bersikulo 3", "Ang iyang gugma\nwalay katapusan"]] },
      { t: "Daygon Ta", a: "Parish Choir · short", lang: "Cebuano", rep: false, note: "1 verse, no repeat", secs: [["Bersikulo 1", "Daygon ta ang Ginoo\nsa tanang kasingkasing"], ["Koro", "Daygon, daygon ta\nang ngalan sa Ginoo"]] }
    ],
    "Ihalad Namo": [
      { t: "Ihalad Namo", a: "Koro Sta. Cecilia", lang: "Cebuano", rep: false, note: "Lyrics only, no image", secs: [["Bersikulo 1", "Ihalad namo kanimo\nang among kinabuhi"], ["Koro", "Dawata, O Ginoo,\nkining among halad"]] }
    ]
  };
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
  function icons() {
    if (!window.lucide) return;
    window.lucide.createIcons({ attrs: { "stroke-width": 2 } });
    // Palette draws its paint dots before the palette; move them last so the filled (current) nav icon can cut them out.
    document.querySelectorAll("svg.lucide-palette").forEach(function (s) { s.querySelectorAll("circle").forEach(function (c) { s.appendChild(c); }); });
  }

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

  // ── Constrained Preparation layouts (Decision 236) ─────────────────────────
  // Every Preparation workspace is a list, a main pane and sometimes an Inspector. When they stop fitting,
  // the main pane stays fixed and the others become opposing slideovers, one at a time. `width` is the
  // logical window width after Interface scale, so a resized window and a high scale behave alike.
  var RAIL_W = 77, MIN_LIST = 240, MIN_MAIN = 520, MIN_INSPECTOR = 300;
  var SIDE_PICK = "button.pl-item, button.sg-result, button.sg-doc, button.sh-navrow, button.sg-sec-name";   // choosing one of these closes the list
  var SIDE_ABOVE = ".o-menu, .o-dialog, .o-slideover";                                                       // surfaces that sit over a slideover
  var FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex="0"]';
  function restore(e, attr, v) { if (v) e.setAttribute(attr, v); else e.removeAttribute(attr); }
  function paneTier(width, hasInspector, listW) {
    var room = width - RAIL_W;
    if (hasInspector) return room >= MIN_LIST + MIN_MAIN + MIN_INSPECTOR ? "full" : room >= MIN_LIST + MIN_MAIN ? "inspector" : "editor";
    return room >= Math.max(MIN_LIST, listW || 0) + MIN_MAIN ? "full" : "editor";
  }
  function panes(app, width, opts) {
    app.removeAttribute("data-tier"); app.removeAttribute("data-side");
    app.querySelectorAll(".o-sidebar, .o-side-scrim, .o-side-close, .o-side-head").forEach(function (e) { e.remove(); });
    app.querySelectorAll(".o-side-l.is-open, .o-side-r.is-open").forEach(function (e) {
      e.classList.remove("is-open"); e.removeAttribute("aria-modal");
      restore(e, "role", e.dataset.sideRole); restore(e, "aria-label", e.dataset.sideLabel);
    });
    app.querySelectorAll(".o-side-main[inert]").forEach(function (e) { e.removeAttribute("inert"); });
    if (opts) app.__sideClose = opts.onClose;
    var host = app.querySelector(".o-body .pl-panes, .o-body .sg-panes");
    var list = host ? [].filter.call(host.children, function (c) { return c.classList.contains("o-pane"); }) : [];
    if (list.length < 2) { app.__side = null; return "full"; }
    var left = list[0], right = list.length > 2 ? list[list.length - 1] : null, scale = parseFloat(app.style.getPropertyValue("--o-scale")) || 1;
    host.classList.add("o-side-host"); list[1].classList.add("o-side-main");
    left.classList.add("o-side-l"); if (right) right.classList.add("o-side-r");
    var tier = paneTier(width, !!right, left.offsetWidth / scale);
    app.dataset.tier = tier; app.__paneW = width;
    if (opts && opts.open && tier === "editor") app.__side = opts.open;
    if (tier === "full" || (tier === "inspector" && app.__side === "left") || (app.__side === "right" && !right)) app.__side = null;
    if (tier === "full") return tier;
    function name(p) { var h = p.querySelector("h2, .o-pane-head b"); return p.getAttribute("aria-label") || (h && h.textContent) || "List"; }
    function toggle(id, p, ic) {
      return '<button class="o-btn o-btn--quiet" data-side-toggle="' + id + '" aria-expanded="' + (app.__side === id) + '">' + icon(ic) + esc(name(p)) + "</button>";
    }
    host.insertAdjacentHTML("beforebegin", '<div class="o-sidebar">' + (tier === "editor" ? toggle("left", left, "list-tree") : "") + '<span class="o-grow"></span>' + (right ? toggle("right", right, "panel-right") : "") + "</div>");
    var open = app.__side === "left" ? left : app.__side === "right" ? right : null;
    if (open) {
      app.dataset.side = app.__side; open.classList.add("is-open");
      var title = name(open), label = "Close " + title;
      open.dataset.sideRole = open.getAttribute("role") || ""; open.dataset.sideLabel = open.getAttribute("aria-label") || "";
      open.setAttribute("role", "dialog"); open.setAttribute("aria-modal", "true"); open.setAttribute("aria-label", title);
      list[1].setAttribute("inert", "");
      // Close leads the pane's own header (Notion-style); a pane without one gets a minimal header.
      var btn = '<button class="o-btn o-btn--quiet o-btn--icon o-close o-side-close" data-side-close aria-label="' + esc(label) + '" title="' + esc(label) + ' (Esc)">' + icon("x") + "</button>";
      var head = open.querySelector(":scope > .o-pane-head, :scope > .sg-browser-head > .sg-row");
      if (head) head.insertAdjacentHTML("afterbegin", btn);
      else open.insertAdjacentHTML("afterbegin", '<div class="o-pane-head o-side-head">' + btn + "<b>" + esc(title) + "</b></div>");
      host.insertAdjacentHTML("afterbegin", '<div class="o-scrim o-side-scrim o-scrim--clear" data-side-close></div>');
    }
    icons();
    if (!app.__sideBound) {
      app.__sideBound = true;
      var setSide = function (id, focusOn) {
        app.__side = id;
        if (!id && app.__sideClose) app.__sideClose();
        panes(app, app.__paneW);
        var f = app.querySelector(focusOn); if (f) f.focus({ preventScroll: true });
      };
      app.addEventListener("click", function (e) {
        var t = e.target.closest && e.target.closest("[data-side-toggle], [data-side-close]");
        if (t && t.dataset.sideToggle) { var id = t.dataset.sideToggle; if (app.__side === id) setSide(null, '[data-side-toggle="' + id + '"]'); else setSide(id, ".o-side-close"); }
        else if (t) { var was = app.__side; setSide(null, '[data-side-toggle="' + was + '"]'); }
        else if (app.__side === "left" && e.target.closest && e.target.closest(".o-side-l :is(" + SIDE_PICK + ")")) setSide(null, '[data-side-toggle="left"]');
      });
      // Capture phase: the slideover must see that a menu is open before the menu's own handler closes it.
      app.addEventListener("keydown", function (e) {
        if (!app.__side || e.defaultPrevented || app.querySelector(SIDE_ABOVE)) return;
        if (e.key === "Escape") {
          if (e.target.dataset && e.target.dataset.in === "draft") return;   // Escape cancels the rename first
          e.preventDefault(); var was = app.__side; setSide(null, '[data-side-toggle="' + was + '"]');
        } else if (e.key === "Tab") {
          var pane = app.querySelector(".o-side-l.is-open, .o-side-r.is-open");
          var all = [].filter.call(pane.querySelectorAll(FOCUSABLE), function (x) { return x.getClientRects().length && x.getAttribute("tabindex") !== "-1"; });
          if (!all.length) return;
          var at = all.indexOf(document.activeElement), next = at < 0 ? all[0] : e.shiftKey && at === 0 ? all[all.length - 1] : !e.shiftKey && at === all.length - 1 ? all[0] : null;
          if (next) { e.preventDefault(); next.focus(); }
        }
      }, true);
    }
    return tier;
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
    // Frames keep a 270px floor (scaled with the chrome); drop a column before shrinking below it.
    // A cell is the frame plus the 2px Cue padding on each side; the timeline's scrollbar takes its own width.
    function maxCols() {
      var s = S.scale, l = layout(), w = W;
      if (l !== "constrained") w -= (railPinned() ? 200 : 28) * s + (l === "wide" ? 520 : 360) * s;
      else w -= 28 * s;
      w -= (32 + 16) * s + 15;                             // timeline padding, group indent, scrollbar
      var n = Math.floor((w + 12 * s) / ((270 + 4 + 12) * s));
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
      S.live = true; S.frozen = false; S.blackout = false; S.frozenReason = ""; S.blackReason = ""; S.blackDoing = ""; S.held = null; say("");
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
      else { S.blackout = true; S.frozen = false; S.blackDoing = ""; say(""); }
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

    // ── Select display (Decisions 49, 50, 202; Decision 292) ─────────────────────────
    // The Settings → Displays picker, in the Insert panel's place, so the operator never leaves
    // Presenter. Choosing a display opens its window black; only Space puts Preview on it.
    var DISPS = { "1": { name: "Built-in display", res: "1366 × 768", pos: "Left", op: true },
      "2": { name: "Epson EB-X51", res: "1920 × 1080", pos: "Right", known: true },
      "3": { name: "Samsung TV", res: "1920 × 1080", pos: "Right" },
      "4": { name: "BenQ MW560", res: "1920 × 1080", pos: "Above" } };
    function present() {
      var out = ["1"];
      if (S.output !== "disconnected" || S.back === "same") out.push(S.cong.n);
      if (S.back === "other") out.push(S.cong.n === "3" ? "2" : "3");
      (S.extra || []).forEach(function (n) { if (out.indexOf(n) < 0) out.push(n); });
      return out;
    }
    function candidates() { return present().filter(function (n) { return !DISPS[n].op && !(n === S.cong.n && S.output === "disconnected" && S.back !== "same"); }); }
    function openDisp() { var c = candidates(); S.ins = { kind: "disp", pick: c.length === 1 ? c[0] : null, identify: false, fit: 0 }; say(""); }
    function dispPanel() {
      var d = S.ins, here = present(), lost = S.output === "disconnected" && S.back !== "same";
      var tiles = ["1", "2", "3", "4"].filter(function (n) { return here.indexOf(n) >= 0 || (n === S.cong.n && lost); }).map(function (n) {
        var x = DISPS[n], un = here.indexOf(n) < 0, on = d.pick === n;
        var chip = x.op ? '<span class="pl-chip">' + icon("monitor") + "Operator display</span>" : un ? statusLabel("unavailable", "Unavailable · congregation display", "ring") :
          on ? '<span class="pl-chip sh-chip-def">' + icon("presentation") + "Use for the congregation</span>" : '<span class="o-meta">' + (x.known ? "Recognised" : "New to Ordo") + "</span>";
        return '<button class="sh-disp' + (x.op ? " is-op" : "") + (un ? " is-un" : "") + (on ? " is-cong" : "") + '" data-act="ins-disp-pick" data-v="' + n + '"' + (x.op || un ? " disabled" : ' aria-pressed="' + on + '"') + ">" +
          (d.identify ? '<span class="sh-ident">' + n + "</span>" : "") + "<b>" + n + " · " + esc(x.name) + '</b><span class="o-meta">' + x.res + " · 16:9 · " + x.pos + "</span>" + chip + "</button>";
      }).join("");
      var p = d.pick && DISPS[d.pick];
      var many = candidates().length > 1;
      var note = !p && many ? '<div class="o-notice" data-state="info">' + icon("scan-eye") + "<div><b>" + candidates().length + " displays could show the congregation output.</b> Ordo won’t guess. Press Identify displays, check which number is on the projector screen, then choose that tile.</div></div>" :
        !p ? '<div class="o-notice" data-state="warning">' + icon("unplug") + "<div><b>" + dn() + " isn’t connected.</b> Check its cable and power. Ordo won’t move output onto this screen or pick another display by itself.</div></div>" :
        '<div class="o-notice" data-state="info">' + icon("info") + "<div><b>" + (p.known ? "Display " + d.pick + " is recognised." : "Display " + d.pick + " is new to Ordo.") + "</b> Using it opens its congregation window black and saves it for this computer, as in Settings → Displays. Space then takes Preview live.</div></div>" +
          (p.known ? "" : '<label class="sh-field"><span class="o-label">Output Fit</span><span class="sh-unit"><input type="range" min="0" max="10" step="1" data-qin="fit" value="' + d.fit + '"> ' + d.fit + '%</span><span class="o-meta">Shrinks the whole frame evenly if the edges are cut off. Check the calibration pattern in Settings → Displays outside Mass.</span></label>');
      var state = S.output === "disconnected" ? (S.back === "same" ? dn() + " is back · Program black" : dn() + " disconnected · Program black") : S.output === "enabled" ? "Output enabled on " + dn() : "Congregation output off";
      return '<section class="pr-ins-panel pr-disp-panel" role="dialog" aria-modal="true" aria-labelledby="pr-ins-t"><div class="pr-ins-head"><div class="o-grow"><h2 class="o-title" id="pr-ins-t">Select congregation display</h2>' +
        '<div class="o-meta">' + icon("monitor-cog") + esc(state) + " · the Presentation keeps running</div></div>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-act="ins-disp-close" aria-label="Close" title="Close (Esc)">' + icon("x") + "</button></div>" +
        '<div class="pr-ins-main pr-disp-main sh-form"><div class="o-label">This computer’s displays</div><div class="sh-topo">' + tiles + "</div>" +
        '<div class="sh-inline"><button class="o-btn' + (many && !d.pick ? " o-btn--primary" : "") + '" data-act="ins-disp-identify"' + (d.identify ? ' aria-pressed="true"' : "") + ">" + icon("scan-eye") + "Identify displays</button>" +
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
      if (S.blackout) return statusLabel("blackout", S.blackDoing ? "BLACKOUT · " + S.blackDoing : "BLACKOUT", "square");
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
        (cur ? statusLabel("preview", "Preview", "square") : "") + "</span>" : "";
      var airAttr = air ? ' data-onair="' + air + '"' : "";
      if (S.mode === "grid" && (projectable(c) || c.k === "unavailable")) {
        return '<button class="pr-tile" data-cue="' + c.i + '"' + cur + '><div class="o-slide-wrap"' + cur + airAttr + ">" + slide(c) + "</div>" +
          '<div class="o-caption"><span class="o-n">' + c.item.n + "." + (c.ci + 1) + '</span><span class="o-t">' + esc(KIND_LABEL[c.k] === "Unavailable" ? "Unavailable · " + c.b : cueLabel(c)) + "</span>" + (c.edited ? '<span class="pl-chip">' + icon("pencil") + "Edited</span>" : "") + onAir + "</div></button>";
      }
      var lead = c.k === "trigger" ? '<span class="pr-who">' + esc(c.who) + "</span>" :
        c.k === "wait" ? icon("hourglass") : c.k === "hold" ? icon("square") :
        c.k === "unavailable" ? icon("circle-off") : '<span class="pr-mini">' + slide(c) + "</span>";
      var kind = c.k === "unavailable" ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : '<span class="o-meta">' + KIND_LABEL[c.k] + "</span>";
      return '<button class="o-row pr-row" data-cue="' + c.i + '"' + (cur ? ' aria-current="true"' : "") + airAttr + '><span class="o-n o-meta">' + c.item.n + "." + (c.ci + 1) + "</span>" + lead +
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
        '<button class="o-btn" data-act="ins-rep" title="Replace ' + esc(CUES[S.preview].item.title) + ' for this Mass">' + icon("replace") + "Replace " + (CUES[S.preview].item.type === "Song" ? "Song" : "item") + ' <span class="o-kbd">Ctrl+R</span></button>' +
        '<button class="o-btn" data-act="ins-edit"' + (projectable(CUES[S.preview]) && !CUES[S.preview].img ? "" : ' disabled title="Ctrl+E edits projectable slides"') + ">" + icon("pencil") + 'Edit slide <span class="o-kbd">Ctrl+E</span></button></div>';
      ITEMS.forEach(function (it) {
        var open = !S.collapsed[it.n];
        var bad = it.cues.filter(function (c) { return c.k === "unavailable"; }).length;
        html += '<section class="pr-group" data-item="' + it.n + '"><button class="pr-group-head" data-toggle="' + it.n + '" aria-expanded="' + open + '">' + icon(open ? "chevron-down" : "chevron-right") +
          '<span class="o-n o-meta">' + it.n + "</span><b>" + esc(it.title) + "</b>" + (it.song ? '<span class="o-meta">' + esc(it.song) + "</span>" : "") +
          (it.replaced ? '<span class="pl-chip">' + icon("replace") + "Replaced live</span>" : "") + (it.inserted ? '<span class="pl-chip">' + icon("zap") + (it.local ? "Inserted live · Mass-local Song" : "Inserted live") + "</span>" : "") +
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
          html += '<button class="o-row pr-rail-row" data-jump="' + it.n + '"' + (it.n === cur ? ' aria-current="true"' : "") + '><span class="o-n o-meta">' + it.n + '</span><span class="o-ellipsis">' + esc(it.title) + "</span>" + (it.inserted ? '<span class="pl-chip">Inserted</span>' : "") + (it.replaced ? '<span class="pl-chip">Replaced</span>' : "") + (it.cues.some(function (c) { return c.edited; }) ? '<span class="pl-chip">Edited</span>' : "") + "</button>";
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
    function openInsert(kind, target) {
      // Replace blacks out rather than Freezes: the wrong version must leave the screen at once.
      if (target) { if (S.output === "enabled" && (S.live || S.blackout)) { S.blackout = true; S.frozen = false; S.held = null; S.frozenReason = ""; S.blackDoing = "replacing"; } }
      else if (S.output === "enabled" && S.live && !S.frozen && !S.blackout) { S.frozen = true; S.frozenReason = "inserting"; }
      else if (S.frozen && /ready$/.test(S.frozenReason)) S.frozenReason = "inserting";
      S.ins = { kind: kind || "song", q: "", active: 0, create: null, mode: "Text", h: "", text: "", rep: target || null }; say("");
    }
    function openReplace() { var it = CUES[S.preview].item; openInsert(it.type === "Song" ? "song" : it.type === "Response sequence" ? "resp" : "general", it); }
    function closeInsert() {
      var what = S.ins.rep ? "Replace" : "Insert";
      S.ins = null;
      if (S.frozenReason === "inserting") S.frozenReason = "";
      if (S.blackDoing === "replacing") S.blackDoing = "";
      say(S.frozen ? what + " cancelled. Program is still frozen; Space or F takes Preview live." : S.blackout ? what + " cancelled. Blackout stays on; Space or B takes Preview live." : what + " cancelled.");
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
      var it = S.ins.rep;
      if (!it) return out;
      // Replacing: other versions of this Song come first, so a wrong version is two keys away.
      var name = it.st || it.song;
      return (QI_VERSIONS[name] || []).filter(function (v) { return v.a !== it.arr && (!q || norm(v.t + " " + v.a).indexOf(q) >= 0); }).map(function (v) { return { x: v, ver: true, why: v.note }; })
        .concat(out.filter(function (o) { return o.x.t !== name; }));
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
      var secs = s.secs || s.slides.map(function (b, i) { return [s.local ? "" : "Bersikulo " + (i + 1), b]; });
      return { title: s.t, st: s.t, arr: s.a, song: s.local ? "Typed for this Mass" : s.a, type: "Song", local: !!s.local, cues: [{ k: "song-id", t: s.t, s: s.local ? "" : s.a }].concat(secs.map(function (x) { return { k: "lyrics", h: x[0], b: x[1] }; })) };
    }
    // Replace swaps one Mass item in place: its title and position stay, only its contents change.
    // Preview lands on the same section when the new version has it, otherwise on the first slide.
    function samePlace(c, cues) {
      if (c.k === "song-id") return cues.filter(function (x) { return x.k === "song-id"; })[0];
      if (c.k !== "lyrics" || !c.h) return null;
      var nth = c.item.cues.slice(0, c.ci).filter(function (x) { return x.k === "lyrics" && x.h === c.h; }).length;
      var same = cues.filter(function (x) { return x.k === "lyrics" && x.h === c.h; });
      return same[Math.min(nth, same.length - 1)] || null;
    }
    function swapItem(old, it) {
      var sec = M.sections[old.section], p = old.first, end = p + old.cues.length, from = CUES[S.preview];
      var place = samePlace(from, it.cues) || it.cues.filter(projectable)[0] || it.cues[0];
      if (S.program != null && S.program >= p && S.program < end) S.program = null;
      else if (S.program != null && S.program >= end) S.program += it.cues.length - old.cues.length;
      if (S.strip && S.strip.kind === "fail" && old.cues.some(function (c) { return c.k === "unavailable"; })) S.strip = null;
      sec.items[sec.items.indexOf(old)] = it; reindex();
      S.preview = place.i; S.follow = true; S.collapsed = {};
      S.ins = null; S.blackDoing = S.blackout ? "Replace ready" : "";
      return from.k === "lyrics" && from.h && place.h !== from.h ? from.h + " isn’t in it, so Preview is on its first slide. " : "";
    }
    function replaceItem(it) {
      var old = S.ins.rep, name = it.type === "Song" && !it.local ? it.st + " · " + it.arr : it.title;
      it.song = it.type === "Song" && !it.local ? name : it.local ? it.title + " · typed for this Mass" : it.title;
      it.title = old.title; it.replaced = true; it.was = old.was || old; it.inserted = old.inserted;
      var note = swapItem(old, it);
      say(old.title + " is now " + name + ". " + (note || "This Mass only. " + (S.output === "enabled" ? "Space takes it live." : "")));
    }
    function putBack() {
      var old = S.ins.rep, it = old.was; it.was = null; it.replaced = false;
      var note = swapItem(old, it);
      say(it.title + " is back as prepared. " + note + (it.cues.some(function (c) { return c.edited; }) ? "Its edits are kept. " : "") + (S.output === "enabled" ? "Space takes it live." : ""));
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
          var g = o.ver ? "Other versions of " + o.x.t : o.x.rep ? M.choir + "’s repertoire" : "Other Songs in the Library";
          if (g !== group) { group = g; html += '<div class="o-section-label pr-qi-group">' + esc(g) + "</div>"; }
        }
        var meta = d.kind === "song" ? (o.ver ? "" : esc(o.x.a) + " · ") + esc(o.x.lang) + " · " + (o.x.secs || o.x.slides).length + " slides" : o.x.cues.length + " Cues · " + esc(o.x.lang);
        html += '<div class="o-row pr-qi-row" role="option" id="qi-' + i + '" data-qi="' + i + '" aria-selected="' + (i === act) + '"><div class="o-grow"><b>' + esc(o.ver ? o.x.a : o.x.t) + '</b><span class="o-meta">' + meta + (o.why ? " · " + esc(o.why) : "") + "</span></div>" +
          (d.kind === "song" && !o.x.rep && !o.ver && i === act ? '<span class="o-meta">Not in ' + esc(M.choir) + "’s repertoire</span>" : "") + "</div>";
      });
      if (!r.length) html = d.kind === "song" ? '<div class="pr-qi-none"><b>No Song matches “' + esc(d.q) + '”.</b><span class="o-meta">Type it for this Mass; it stays in this plan until someone adds it to the Library after Mass.</span>' +
        '<button class="o-btn" data-act="ins-create">' + icon("plus") + "Type “" + esc(d.q) + '” for this Mass <span class="o-kbd">Enter</span></button></div>' :
        '<div class="pr-qi-none"><b>Nothing in Responses &amp; Prayers matches “' + esc(d.q) + '”.</b><span class="o-meta">Use <b>Text or image</b> to type it.</span></div>';
      var ph = d.kind === "song" ? "Title, other name or first line" : "Title or words of a response";
      return '<label class="o-field pr-qi-search">' + icon("search") + '<input class="pl-input" data-qin="q" value="' + esc(d.q) + '" placeholder="' + ph + '" role="combobox" aria-expanded="true" aria-controls="qi-list" aria-activedescendant="' + (r.length ? "qi-" + act : "") + '" autocomplete="off"></label>' +
        '<div class="pr-qi-list" id="qi-list" tabindex="0"' + (r.length ? ' role="listbox"' : "") + ' aria-label="' + (d.kind === "song" ? "Songs" : "Responses and Prayers") + '">' + html + "</div>";
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
      var verb = d.rep ? "Replace with " : "Insert ";
      var label = d.create ? verb + "typed Song" : d.kind === "song" ? verb + "Song" : d.kind === "resp" ? verb + "Response sequence" : verb + (d.rep ? "this slide" : "slide");
      var r = searching ? results()[Math.min(d.active, results().length - 1)] : null;
      if (r && r.ver) label = "Replace with this version";
      var inUse = d.rep ? (d.rep.st || d.rep.song || "") + (d.rep.arr ? " · " + d.rep.arr : "") : "";
      var edits = d.rep ? d.rep.cues.filter(function (c) { return c.edited; }).length : 0;
      var warn = edits ? '<div class="o-notice" data-state="warning">' + icon("pencil") + "<div><b>" + edits + (edits === 1 ? " slide was" : " slides were") + " edited live.</b> Replacing takes those edits out of this Mass; Put back restores them.</div></div>" : "";
      return '<section class="pr-ins-panel" role="dialog" aria-modal="true" aria-labelledby="pr-ins-t"><div class="pr-ins-head"><div class="o-grow"><h2 class="o-title" id="pr-ins-t">' + (d.rep ? "Replace" : "Insert") + "</h2>" +
        '<div class="o-meta">' + icon(d.rep ? "replace" : "corner-down-right") + (d.rep ? "" : "After ") + "<b>" + esc(a.title) + "</b>" + (inUse ? " · " + esc(inUse) : "") + " · " + esc(M.sections[a.section].title) + " · " + (S.blackout ? "Blackout on" : S.frozen ? "Program frozen" : S.output === "enabled" ? "Program not live" : "Rehearsal: output disabled") + "</div></div>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-act="ins-close" aria-label="Close" title="Close (Esc)">' + icon("x") + "</button></div>" +
        '<div class="o-tabs pr-ins-tabs" role="tablist">' + KINDS.map(function (k) { return '<button role="tab" data-act="ins-kind" data-v="' + k[0] + '" aria-selected="' + (d.kind === k[0]) + '">' + icon(k[1]) + k[2] + ' <span class="o-kbd">Alt+' + k[3] + "</span></button>"; }).join("") + "</div>" +
        '<div class="pr-ins-split"><div class="pr-ins-main sh-form">' + warn + (searching ? searchList() : form()) + "</div>" +
        '<div class="pr-ins-prev"><div class="o-label">' + (r ? esc(r.x.t) + (r.ver ? " · " + esc(r.x.a) : "") : "Preview") + '</div><div class="pr-ins-thumbs">' + cues.slice(0, 4).map(function (c) { return projectable(c) ? slide(c) : '<div class="pr-private"><span class="pr-who">' + esc(c.who || "") + "</span><div><b>" + esc(c.b) + '</b><div class="o-meta">Operator-only · not projected</div></div></div>'; }).join("") + "</div></div></div>" +
        '<div class="pr-ins-foot"><span class="o-meta">' + (searching ? '<span class="o-kbd">↑</span><span class="o-kbd">↓</span> move · <span class="o-kbd">Enter</span> ' + (d.rep ? "replace" : "insert") + " · " : "") + '<span class="o-kbd">Esc</span> ' + (d.create ? "back" : "close") + " · saves to this Mass plan</span>" +
        '<span class="o-grow"></span>' + (d.rep && d.rep.was ? '<button class="o-btn" data-act="ins-rep-back">' + icon("undo-2") + 'Put back the prepared version <span class="o-kbd">Alt+B</span></button>' : "") + '<button class="o-btn" data-act="ins-close">Cancel</button><button class="o-btn o-btn--primary" data-act="ins-save"' + (ok ? "" : " disabled") + ">" + label + "</button></div></section>";
    }
    function insSave() {
      var d = S.ins;
      var put = d.rep ? replaceItem : insertItem;
      if (d.create) { var sl = slidesOf(d.create.text); put(songItem({ t: d.create.title.trim(), slides: sl, local: true })); return; }
      if (d.kind === "general") { put({ title: d.mode === "Image" ? "Image · fiesta-poster.jpg" : d.h.trim() || "Announcement", type: "General Presentation", cues: [d.mode === "Image" ? { k: "general", img: "fiesta-poster.jpg", b: "" } : { k: "general", h: d.h.trim(), b: d.text.trim(), f: "Parokya ni San Roque" }] }); return; }
      var r = results()[Math.min(d.active, results().length - 1)]; if (!r) return;
      if (d.kind === "song") put(songItem(r.x));
      else put({ title: r.x.t, type: "Response sequence", cues: r.x.cues.map(function (c) { return Object.assign({}, c); }) });
    }
    function insAct(a, t) {
      if (a.indexOf("ins-ed") === 0) return edAct(a, t);
      if (a.indexOf("ins-disp") === 0) return dispAct(a, t);
      if (a === "ins-open") openInsert();
      else if (a === "ins-rep") openReplace();
      else if (a === "ins-rep-back") putBack();
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
      if (e.altKey && k.toLowerCase() === "b" && d.rep && d.rep.was) { e.preventDefault(); putBack(); render(); app.focus({ preventScroll: true }); return; }
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
      if (c.k === "unavailable") { say("Can’t edit an unavailable Cue: " + c.why + ". Replace puts something in its place."); return; }
      if (!projectable(c)) { say("Ctrl+E edits projectable slides; this " + KIND_LABEL[c.k].toLowerCase() + " is never shown."); return; }
      if (c.img) { say("A picture can’t be edited live. Replace swaps it for another."); return; }
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
      var had = app.contains(document.activeElement), onCue = had && document.activeElement.dataset.cue != null;
      var l = layout(), pinned = railPinned();
      app.style.setProperty("--o-scale", S.scale);
      app.style.setProperty("--pr-band-mon", Math.floor(H * 0.3 * 16 / 9) + "px");
      app.dataset.layout = l;
      var header = '<header class="pr-header"><div class="pr-session"><b>' + esc(MASS.title) + '</b><span class="o-meta">' + esc(MASS.date) + "</span></div>" +
        '<span class="o-grow"></span><span class="o-meta pr-clock">9:14</span>' +
        (S.output === "enabled" ? statusLabel("live", dn() + " connected") : S.output === "disconnected" ? (S.back === "same" ? statusLabel("warning", dn() + " back · not resumed", "diamond") : statusLabel("danger", dn() + " disconnected", "square")) : statusLabel("disabled", dn() + " ready · not enabled", "square")) +
        '<div class="o-segmented" role="group" aria-label="Timeline view"><button data-view="grid" aria-label="Grid" aria-pressed="' + (S.mode === "grid") + '">' + icon("layout-grid") + (l === "constrained" ? "" : "Grid") + '</button><button data-view="list" aria-label="List" aria-pressed="' + (S.mode === "list") + '">' + icon("list") + (l === "constrained" ? "" : "List") + "</button></div>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" title="Keyboard shortcuts" aria-label="Keyboard shortcuts">' + icon("keyboard") + "</button>" +
        '<button class="o-btn">' + icon("log-out") + "Exit Presenter</button></header>";
      var tab = pinned ? "" : '<button class="pr-ordo-tab" data-act="open-rail" aria-label="Open Ordo">ORDO</button>';
      var main = '<section class="pr-timeline" aria-label="Cue timeline">' + timeline() + "</section>";
      var body;
      if (l === "constrained") body = tab + '<div class="pr-col">' + dock() + main + "</div>";
      else body = (pinned ? rail(false) : tab) + main + dock();
      var overlay = S.railOpen && !pinned ? '<div class="o-scrim" data-act="close-rail"></div>' + rail(true) : "";
      var old = app.querySelector(".pr-timeline"), kept = old ? old.scrollTop : 0;
      var fk = document.activeElement && app.contains(document.activeElement) && document.activeElement.dataset ? document.activeElement.dataset.qin : null, caret = fk && document.activeElement.selectionStart;
      app.innerHTML = header + strip() + '<div class="o-body pr-body">' + body + insertPanel() + "</div>" + overlay;
      icons();
      // On a short window the monitors give up height, so Freeze, Blackout and the primary action stay on screen.
      var dockEl = app.querySelector(".pr-dock");
      if (dockEl) {
        dockEl.style.removeProperty("--pr-mon-w");
        var monitors = dockEl.querySelectorAll(".o-slide"), over = dockEl.scrollHeight - dockEl.clientHeight;
        if (over > 0 && monitors.length) dockEl.style.setProperty("--pr-mon-w", Math.floor((monitors[0].offsetHeight - over / monitors.length) * 16 / 9) + "px");
      }
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
      // innerHTML drops the focused element; without this Space, F and B would stop reaching the operator's keys.
      if (had && !S.ins && !app.contains(document.activeElement)) {
        ((onCue && tl && tl.querySelector('[data-cue="' + S.preview + '"]')) || app).focus({ preventScroll: true });
      }
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
        if ((e.ctrlKey || e.metaKey) && (k === "r" || k === "R")) { openReplace(); render(); e.preventDefault(); return; }
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
      connect: function (list) { S.extra = list; if (S.output === "disconnected" && !S.back) S.back = "other"; S.strip = { kind: "out", state: "danger", icon: "monitor-x", b: "Other displays were connected", t: "None of them is " + dfull() + ", so Ordo won’t use them on its own. Choose one in Select display…" }; render(); },
      selectDisplay: function (o) { openDisp(); Object.keys(o || {}).forEach(function (k) { S.ins[k] = o[k]; }); render(); },
      setMode: function (m) { S.mode = m; render(); },
      insert: function (kind, o) { openInsert(kind); Object.keys(o || {}).forEach(function (k) { S.ins[k] = o[k]; }); render(); },
      edit: function (cue, o) { S.preview = cue; S.follow = true; openEdit(); o = o || {}; if (S.ins) { if (o.scope) S.ins.scope = o.scope; Object.keys(o.draft || {}).forEach(function (k) { S.ins.draft[k] = o.draft[k]; }); if (o.draft || o.scope) applyEdit(); if (o.done) closeEdit(); } render(); },
      replace: function (cue, o) { S.preview = cue; S.follow = true; openReplace(); Object.keys(o || {}).forEach(function (k) { S.ins[k] = o[k]; }); render(); },
      save: function () { insSave(); render(); },
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
    function a(n) { return '<a href="' + (PAGE_OF[n[0]] || "#") + '" data-dest="' + esc(n[0]) + '" aria-current="' + (n[0] === active ? "page\" title=\"" + esc(n[0]) : "false") + '">' + icon(n[1], "o-icon-20") + "<span>" + esc(n[0]) + "</span></a>"; }
    return '<nav class="o-navrail" aria-label="Global navigation">' + NAV.map(a).join("") + '<span class="o-spacer"></span>' + NAV_FOOT.map(a).join("") + "</nav>";
  }

  // The Mass planner itself lives in ordo-planner.js, which adds OrdoKit.planner.

  window.OrdoKit = {
    panes: panes, paneTier: paneTier,
    MASS: MASS, ITEMS: ITEMS, CUES: CUES, QI_SONGS: QI_SONGS, projectable: projectable, cueLabel: cueLabel, KIND_LABEL: KIND_LABEL,
    esc: esc, icon: icon, canvas: canvas, slide: slide, statusLabel: statusLabel, icons: icons, fit: fit, presenter: presenter,
    navrail: navrail
  };
  document.addEventListener("DOMContentLoaded", icons);

  // The keyboard behaviour DESIGN.md's Focus Navigation asks of every screen: one Tab stop per list
  // with arrow keys inside it, F6 between major regions, and truncated text that names itself.
  // The Presenter's own arrow keys stay in charge of its timeline.
  var ITEMS_SEL = "button.pr-tile, button.pr-row, button.pr-rail-row, button.pl-item, button.pl-cue, button.sg-result, button.sg-doc, button.sg-rev";
  var GROUP_SEL = ".pr-timeline, .pr-rail, .pl-outline, .pl-thumbs, .sg-browser, .pl-cues";
  var REGION_SEL = ".o-navrail, .pr-rail, .pr-timeline, .pr-dock, .o-pane";
  var MODAL_SEL = ".o-dialog, .o-slideover, .pr-ins-panel, .o-side-l.is-open, .o-side-r.is-open";
  function shown(e) { return e.getClientRects().length > 0; }
  function refresh(app) {
    app.querySelectorAll(GROUP_SEL).forEach(function (g) {
      var items = Array.prototype.filter.call(g.querySelectorAll(ITEMS_SEL), function (e) { return e.closest(GROUP_SEL) === g; });
      if (!items.length) return;
      var on = items.filter(function (e) { return e === document.activeElement; })[0] ||
        items.filter(function (e) { return e.getAttribute("aria-current") === "true"; })[0] || items[0];
      items.forEach(function (e) { e.tabIndex = e === on ? 0 : -1; });
    });
    // A scrolling panel with no control inside still has to be reachable to scroll by keyboard.
    app.querySelectorAll(".pr-ins-prev, .pr-ins-main, .o-overlay-body, .pl-insp-body, .pl-ed-body, .sg-detail-body").forEach(function (e) {
      if (e.scrollHeight > e.clientHeight && !e.querySelector("a[href], button, input, select, textarea")) e.tabIndex = 0;
      else e.removeAttribute("tabindex");
    });
    // Header rows follow the workspace's logical width: under 1000px a header's tabs take a second row.
    var ws = app.querySelector(".o-body > .o-grow"), sc = parseFloat(app.style.getPropertyValue("--o-scale")) || 1;
    if (ws) ws.toggleAttribute("data-narrow", ws.clientWidth / sc < 1000);
    // In a narrow workspace an editor header's secondary actions show their icon alone (the label stays
    // the accessible name), so the title keeps its room and the header keeps one row.
    app.querySelectorAll(":is(.sh-edhead, .sg-edhead) > .o-btn:not(:first-child):not(.o-btn--primary)").forEach(function (b) {
      if (!b.querySelector(":scope > svg") || b.querySelector(":scope > .o-btn-label")) return;
      var text = "";
      [].slice.call(b.childNodes).forEach(function (n) { if (n.nodeType === 3 && n.textContent.trim()) { text += n.textContent; n.remove(); } });
      if (!text.trim()) return;
      var l = document.createElement("span"); l.className = "o-btn-label"; l.textContent = text.trim(); b.appendChild(l);
      if (!b.title) b.title = text.trim();
    });
    app.querySelectorAll(".o-ellipsis, .sh-edname > .o-title, .sh-edname > .o-meta, .sg-edhead > .o-grow > *, .sh-head > .o-meta").forEach(function (e) {
      if (e.scrollWidth > e.clientWidth + 1) e.title = e.textContent.trim(); else e.removeAttribute("title");
    });
  }
  function move(g, from, dir) {
    var items = Array.prototype.filter.call(g.querySelectorAll(ITEMS_SEL), function (e) { return e.closest(GROUP_SEL) === g && shown(e); });
    var next = items[items.indexOf(from) + dir];
    if (!next) return;
    from.tabIndex = -1; next.tabIndex = 0; next.focus();
  }
  document.addEventListener("keydown", function (e) {
    var t = e.target, app = t.closest && t.closest(".o-app");
    if (!app) return;
    if (e.key === "F6") {
      if ([].some.call(app.querySelectorAll(MODAL_SEL), shown)) { e.preventDefault(); e.stopPropagation(); return; }   // a modal surface keeps focus
      var rs = Array.prototype.filter.call(app.querySelectorAll(REGION_SEL), shown);
      rs = rs.filter(function (r) { return !rs.some(function (o) { return o !== r && o.contains(r); }); });
      if (rs.length < 2) return;
      var at = rs.findIndex(function (r) { return r.contains(t); });
      var r = rs[at < 0 ? (e.shiftKey ? rs.length - 1 : 0) : (at + (e.shiftKey ? -1 : 1) + rs.length) % rs.length];
      var f = r.querySelector('[tabindex="0"], a[href], button:not([disabled]):not([tabindex="-1"]), input:not([disabled]), select, textarea');
      if (f) f.focus();
      e.preventDefault(); e.stopPropagation();
      return;
    }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;   // Alt+arrows belong to the workspace
    if (!t.matches(ITEMS_SEL) || t.closest(".pr-timeline")) return;
    var g = t.closest(GROUP_SEL);
    if (!g) return;
    move(g, t, e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : -1);
    e.preventDefault(); e.stopPropagation();
  }, true);
  document.addEventListener("DOMContentLoaded", function () {
    var queued = false;
    function refreshAll() { queued = false; document.querySelectorAll(".o-app").forEach(refresh); }
    function queue() {
      if (queued) return;
      queued = true;
      requestAnimationFrame(refreshAll);
    }
    new MutationObserver(queue).observe(document.body, { childList: true, subtree: true });
    window.addEventListener("load", queue);
    if (document.fonts && document.fonts.ready) document.fonts.ready.then(queue);
    queue();
  });
})();

// ── Motion (Decision 301) ─────────────────────────────────────────────────────────────────────
// The workspace scripts re-render with innerHTML, so motion is inferred here from what each render
// changed: a tab or rail destination that moved, a slideover or side pane that opened or closed.
// Presenter is excluded: nothing in it moves like a page, so nothing can be mistaken for a slide change.
(function () {
  var STD = 160, OPEN = 400, CLOSE = 200, EASE = "cubic-bezier(0.2, 0, 0, 1)", EXIT = "cubic-bezier(0.3, 0, 1, 1)";
  var reduce = window.matchMedia ? window.matchMedia("(prefers-reduced-motion: reduce)") : null;
  function still() { return !!(reduce && reduce.matches); }
  function play(el, frames, ms, ease) {
    if (!el || !el.animate) return null;
    try { return el.animate(frames, { duration: ms, easing: ease }); } catch (e) { return null; }
  }
  function enter(el, dx, dy) {
    if (still()) return play(el, [{ opacity: 0 }, { opacity: 1 }], 100, "linear");
    return play(el, [{ opacity: 0, transform: "translate(" + dx + "px, " + dy + "px)" }, { opacity: 1, transform: "none" }], STD, EASE);
  }
  // What a tab strip changes: everything after the header it sits in (or after the strip itself).
  function panel(strip) {
    var at = strip.closest("header") || strip, out = [];
    for (var n = at.nextElementSibling; n; n = n.nextElementSibling) if (!n.matches(".o-scrim, .o-slideover, .o-dialog, .o-toast, .o-ghost")) out.push(n);
    return out;
  }
  function save(v) { try { sessionStorage.setItem("ordo-motion", JSON.stringify(v)); } catch (e) {} }
  function take() { try { var v = sessionStorage.getItem("ordo-motion"); sessionStorage.removeItem("ordo-motion"); return v ? JSON.parse(v) : null; } catch (e) { return null; } }

  // Rail: one pill per rail that travels to the current entry. The entry's icon slides to the middle
  // and its label fades out in CSS; --o-shift is how far the icon travels for that entry's label.
  function railPill(nav, animate) {
    var pill = nav.querySelector(":scope > .o-navpill");
    if (!pill) { pill = document.createElement("span"); pill.className = "o-navpill"; pill.setAttribute("aria-hidden", "true"); nav.appendChild(pill); }
    nav.querySelectorAll(":scope > a").forEach(function (a) {
      var s = a.querySelector("span"); if (s) a.style.setProperty("--o-shift", (s.offsetHeight + 2) / 2 + "px");
    });
    var a = nav.querySelector(':scope > a[aria-current="page"]');
    if (!a) { pill.style.opacity = "0"; return; }
    var at = "translate(" + a.offsetLeft + "px, " + (a.offsetTop + (a.offsetHeight - pill.offsetHeight) / 2) + "px)";
    if (!animate && pill.dataset.at === at) return;            // a later render must not cut a pill in flight
    pill.style.transition = animate && !still() ? "transform " + STD + "ms " + EASE : "none";
    pill.style.transform = at; pill.style.opacity = "1"; pill.dataset.at = at;
  }

  // Tabs: the selected tab's underline is one pill that slides between tabs.
  function tabs(strip) { return [].slice.call(strip.querySelectorAll('[role="tab"]')); }
  function selected(strip) { return tabs(strip).findIndex(function (t) { return t.getAttribute("aria-selected") === "true"; }); }
  function tabPill(strip, from) {
    var pill = strip.querySelector(":scope > .o-tabpill");
    if (!pill) { pill = document.createElement("span"); pill.className = "o-tabpill"; pill.setAttribute("aria-hidden", "true"); strip.appendChild(pill); }
    strip.classList.add("has-pill");
    var t = tabs(strip)[selected(strip)];
    if (!t) { pill.style.opacity = "0"; return null; }
    var to = { x: t.offsetLeft, w: t.offsetWidth };
    if (pill.dataset.at === to.x + "/" + to.w && !from) return to;
    if (from && !still()) {
      pill.style.transition = "none"; pill.style.transform = "translateX(" + from.x + "px)"; pill.style.width = from.w + "px";
      void pill.offsetWidth;
      pill.style.transition = "transform " + STD + "ms " + EASE + ", width " + STD + "ms " + EASE;
    } else pill.style.transition = "none";
    pill.style.transform = "translateX(" + to.x + "px)"; pill.style.width = to.w + "px"; pill.style.opacity = "1";
    pill.dataset.at = to.x + "/" + to.w;
    return to;
  }

  // Slideovers and side panes: open in 400ms fast-start/slow-finish, close in 200ms accelerating.
  // A closed surface is already gone from the DOM, so its last render is brought back as an inert
  // ghost just long enough to leave.
  function slide(el, side) { return side === "l" ? -(el.offsetWidth + 16) : el.offsetWidth + 16; }
  function opened(el, side, scrim) {
    if (still()) { play(el, [{ opacity: 0 }, { opacity: 1 }], 100, "linear"); return; }
    play(el, [{ transform: "translateX(" + slide(el, side) + "px)" }, { transform: "none" }], OPEN, EASE);
    if (scrim) play(scrim, [{ opacity: 0 }, { opacity: 1 }], OPEN, EASE);
  }
  function ghost(node, into, before) {
    var g = node.isConnected ? node.cloneNode(true) : node;
    g.classList.add("o-ghost"); g.setAttribute("inert", ""); g.setAttribute("aria-hidden", "true");
    g.querySelectorAll("[id]").forEach(function (x) { x.removeAttribute("id"); });
    if (before && before.parentNode === into) into.insertBefore(g, before); else into.appendChild(g);
    return g;
  }
  function closed(app, key, rec) {
    var side = key === "side:l" ? "l" : "r", host, g, gs = null;
    if (key.indexOf("side:") === 0) {
      host = app.dataset.tier && app.querySelector(".o-side-host");
      if (!host) return;                                         // the pane became fixed: nothing leaves
      g = ghost(rec.el, host); g.classList.add("is-open");
    } else {
      if (rec.scrim) gs = ghost(rec.scrim, app);
      g = ghost(rec.el, app);
    }
    var done = function () { g.remove(); if (gs) gs.remove(); };
    var a = still() ? play(g, [{ opacity: 1 }, { opacity: 0 }], 100, "linear")
      : play(g, [{ transform: "none" }, { transform: "translateX(" + slide(g, side) + "px)" }], CLOSE, EXIT);
    if (gs) play(gs, [{ opacity: 1 }, { opacity: 0 }], still() ? 100 : CLOSE, still() ? "linear" : EXIT);
    if (a) a.onfinish = done; else done();
  }

  // Full-workspace views rise from the bottom over the workspace they replace, and sink
  // back over it. The replaced workspace is already gone, so its last render stays underneath meanwhile.
  var FULL = ".sg-import, .sh-themeed";   // Import .ordo, and a Theme opened for editing
  function under(ws, nodes, ms) {
    if (!nodes || !nodes.length) return;
    var u = document.createElement("div"); u.className = "o-ghost o-under"; u.setAttribute("inert", ""); u.setAttribute("aria-hidden", "true");
    nodes.forEach(function (n) { if (!n.isConnected) { n.querySelectorAll("[id]").forEach(function (x) { x.removeAttribute("id"); }); u.appendChild(n); } });
    ws.insertBefore(u, ws.firstChild); setTimeout(function () { u.remove(); }, ms);
  }
  function rise(app, st, el) {
    var ws = el.parentElement; if (!ws) return;
    if (still()) { play(el, [{ opacity: 0 }, { opacity: 1 }], 100, "linear"); return; }
    ws.classList.add("o-lift"); under(ws, st.kids, OPEN);
    var a = play(el, [{ transform: "translateY(" + ws.offsetHeight + "px)" }, { transform: "none" }], OPEN, EASE);
    var end = function () { ws.classList.remove("o-lift"); }; if (a) a.onfinish = end; else end();
  }
  function sink(app, rec) {
    var ws = app.querySelector(".o-body > .o-grow"); if (!ws) return;
    var g = ghost(rec.el, ws); g.classList.add("o-over"); ws.classList.add("o-lift");
    var a = still() ? play(g, [{ opacity: 1 }, { opacity: 0 }], 100, "linear")
      : play(g, [{ transform: "none" }, { transform: "translateY(" + ws.offsetHeight + "px)" }], CLOSE, EXIT);
    var end = function () { g.remove(); ws.classList.remove("o-lift"); }; if (a) a.onfinish = end; else end();
  }

  var state = new WeakMap();
  function scan(app) {
    if (app.classList.contains("o-presenter")) return;
    var st = state.get(app), first = !st;
    if (first) { st = { tabs: {}, open: {} }; state.set(app, st); }
    app.querySelectorAll(".o-navrail").forEach(function (n) { railPill(n, false); });
    var strips = [].slice.call(app.querySelectorAll('.o-tabs[role="tablist"]'));
    strips.forEach(function (strip, i) {
      var key = strip.className.replace(" has-pill", "") + "#" + i, prev = st.tabs[key], idx = selected(strip), n = tabs(strip).length;
      var moved = !!prev && idx >= 0 && prev.idx !== idx && prev.n === n;
      var to = tabPill(strip, moved ? prev : null);
      if (moved) panel(strip).forEach(function (el) { enter(el, 24 * (idx > prev.idx ? 1 : -1), 0); });
      st.tabs[key] = to ? { x: to.x, w: to.w, idx: idx, n: n } : null;
    });
    var now = {};
    app.querySelectorAll(".o-slideover:not(.o-ghost)").forEach(function (el) {
      var p = el.previousElementSibling;
      now["ov:" + (el.getAttribute("aria-labelledby") || el.className.replace(" is-receded", ""))] = { el: el, scrim: p && p.matches(".o-scrim:not(.o-ghost)") ? p : null };
    });
    app.querySelectorAll(".o-side-l.is-open:not(.o-ghost), .o-side-r.is-open:not(.o-ghost)").forEach(function (el) {
      now[el.classList.contains("o-side-l") ? "side:l" : "side:r"] = { el: el, scrim: null };
    });
    var full = app.querySelector(FULL + ":not(.o-ghost)");
    if (full) now["full"] = { el: full, scrim: null };
    // Dialogs, menus and popovers, and toasts float over the workspace: they fade and settle in place.
    app.querySelectorAll(".o-dialog:not(.o-ghost), .o-menu:not(.o-ghost), .o-toast:not(.o-ghost)").forEach(function (el) { if (el.closest(".o-ghost")) return;
      var kind = el.classList.contains("o-dialog") ? "dlg" : el.classList.contains("o-menu") ? "menu" : "toast", p = el.previousElementSibling;
      now[kind + ":" + (kind === "toast" ? "" : el.getAttribute("aria-labelledby") || el.className)] =
        { el: el, kind: kind, scrim: kind === "dlg" && p && p.matches(".o-scrim:not(.o-ghost)") ? p : null, path: path(app, el.parentElement), pc: el.parentElement.className };
    });
    if (!first) {
      Object.keys(now).forEach(function (k) {
        if (st.open[k]) return;
        if (k === "full") rise(app, st, now[k].el); else if (now[k].kind) appear(now[k]); else opened(now[k].el, k === "side:l" ? "l" : "r", now[k].scrim);
      });
      Object.keys(st.open).forEach(function (k) { if (!now[k]) { if (k === "full") sink(app, st.open[k]); else if (st.open[k].kind) vanish(app, st.open[k]); else closed(app, k, st.open[k]); } });
    }
    st.open = now;
    var ws = app.querySelector(".o-body > .o-grow");
    st.kids = ws ? [].filter.call(ws.children, function (c) { return !c.classList.contains("o-ghost"); }) : null;
    // Disclosure rows (Diagnostics details): the body opens and shuts in 160ms and the chevron turns.
    var disc = {};
    app.querySelectorAll(".sh-drow").forEach(function (row) {
      var h = row.querySelector(":scope > [aria-expanded]"); if (!h) return;
      var b = h.nextElementSibling;
      disc[h.dataset.id || h.textContent] = { row: row, head: h, open: h.getAttribute("aria-expanded") === "true", body: b && !b.classList.contains("o-ghost") ? b : null };
    });
    if (!first && st.disc) Object.keys(disc).forEach(function (id) {
      var was = st.disc[id], d = disc[id];
      if (!was || was.open === d.open) return;
      turn(d.head.querySelector(":scope > svg"), d.open);
      if (d.open && d.body) fold(d.body, true);
      else if (!d.open && was.body) { var g = ghost(was.body, d.row); fold(g, false, function () { g.remove(); }); }
    });
    st.disc = disc;
  }
  // Floating surfaces. A dialog zooms in from 95% with the overlay timing (400ms in, 200ms out); a menu, popover
  // or toast with the standard 160ms. Each keeps its own centring transform underneath the motion.
  var FLOAT = { dlg: { inMs: OPEN, outMs: CLOSE, from: "scale(0.95)" }, menu: { inMs: STD, outMs: 120, from: "translateY(-4px) scale(0.95)", origin: "top" }, toast: { inMs: STD, outMs: STD, from: "translateY(8px)" } };
  function base(el) { var t = getComputedStyle(el).transform; return t && t !== "none" ? t + " " : ""; }
  function appear(rec) {
    var f = FLOAT[rec.kind], el = rec.el;
    if (still()) { play(el, [{ opacity: 0 }, { opacity: 1 }], 100, "linear"); if (rec.scrim) play(rec.scrim, [{ opacity: 0 }, { opacity: 1 }], 100, "linear"); return; }
    var b = base(el);
    play(el, [{ opacity: 0, transform: b + f.from, transformOrigin: f.origin || "center" }, { opacity: 1, transform: b || "none", transformOrigin: f.origin || "center" }], f.inMs, EASE);
    if (rec.scrim) play(rec.scrim, [{ opacity: 0 }, { opacity: 1 }], f.inMs, EASE);
  }
  function vanish(app, rec) {
    var f = FLOAT[rec.kind], host = resolve(app, rec.path);
    if (!host || host.className !== rec.pc) return;              // its container went too: nothing to leave from
    var gs = rec.scrim ? ghost(rec.scrim, host) : null, g = ghost(rec.el, host);
    var b = base(g), ms = still() ? 100 : f.outMs, ease = still() ? "linear" : EXIT;
    var a = play(g, still() ? [{ opacity: 1 }, { opacity: 0 }] : [{ opacity: 1, transform: b || "none", transformOrigin: f.origin || "center" }, { opacity: 0, transform: b + f.from, transformOrigin: f.origin || "center" }], ms, ease);
    if (gs) play(gs, [{ opacity: 1 }, { opacity: 0 }], ms, ease);
    var done = function () { g.remove(); if (gs) gs.remove(); };
    if (a) a.onfinish = done; else done();
  }
  function path(app, el) { var p = []; for (var n = el; n && n !== app; n = n.parentElement) p.unshift([].indexOf.call(n.parentElement.children, n)); return p; }
  function resolve(app, p) { var n = app; for (var i = 0; n && i < p.length; i++) n = n.children[p[i]]; return n || null; }
  function fold(el, open, done) {
    var h = el.offsetHeight, cs = getComputedStyle(el), a;
    if (still()) a = play(el, open ? [{ opacity: 0 }, { opacity: 1 }] : [{ opacity: 1 }, { opacity: 0 }], 100, "linear");
    else {
      var shut = { height: "0px", paddingTop: "0px", paddingBottom: "0px", marginTop: "0px", marginBottom: "0px", opacity: 0 };
      var full = { height: h + "px", paddingTop: cs.paddingTop, paddingBottom: cs.paddingBottom, marginTop: cs.marginTop, marginBottom: cs.marginBottom, opacity: 1 };
      el.style.overflow = "hidden";
      a = play(el, open ? [shut, full] : [full, shut], STD, open ? EASE : EXIT);
    }
    var end = function () { el.style.overflow = ""; if (done) done(); };
    if (a) a.onfinish = end; else end();
  }
  function turn(svg, open) {
    if (!svg || still()) return;
    play(svg, [{ transform: "rotate(" + (open ? -90 : 90) + "deg)" }, { transform: "none" }], STD, EASE);
  }
  // Native disclosures (<details>): the same motion. Closing waits for the content to shut.
  document.addEventListener("toggle", function (e) {
    var d = e.target;
    if (!d.matches || !d.matches("details") || !d.open || d.closest(".o-presenter")) return;
    [].slice.call(d.children).forEach(function (c) { if (c.tagName !== "SUMMARY") fold(c, true); });
  }, true);
  document.addEventListener("click", function (e) {
    var s = e.target.closest && e.target.closest("details[open] > summary");
    if (!s || still() || s.closest(".o-presenter")) return;
    var d = s.parentElement;
    e.preventDefault();
    if (d.dataset.shutting) return;
    d.dataset.shutting = "1";
    var kids = [].slice.call(d.children).filter(function (c) { return c.tagName !== "SUMMARY"; }), left = kids.length;
    if (!left) { d.open = false; delete d.dataset.shutting; return; }
    kids.forEach(function (c) { fold(c, false, function () { if (--left === 0) { d.open = false; delete d.dataset.shutting; } }); });
  });
  var busy = false;
  function scanAll() { if (busy) return; busy = true; try { document.querySelectorAll(".o-app").forEach(scan); } finally { busy = false; } }

  // Rail destinations and linked tabs are other mockup pages: the pill moves first, then the page
  // loads, and the arriving workspace finishes the motion.
  document.addEventListener("click", function (e) {
    if (e.defaultPrevented || e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) return;
    var a = e.target.closest && e.target.closest('.o-navrail > a[href], .o-tabs a[role="tab"][href]');
    if (!a || a.getAttribute("aria-current") === "page" || a.getAttribute("aria-selected") === "true" || !a.closest(".o-app")) return;
    e.preventDefault();
    var rail = a.closest(".o-navrail");
    if (rail) {
      var cur = rail.querySelector(':scope > a[aria-current="page"]');
      if (cur) { cur.setAttribute("aria-current", "false"); cur.removeAttribute("title"); }
      a.setAttribute("aria-current", "page"); railPill(rail, true); save({ kind: "dest" });
    } else {
      var strip = a.closest(".o-tabs"), from = selected(strip), list = tabs(strip), prev = list[from], to = list.indexOf(a);
      var at = prev ? { x: prev.offsetLeft, w: prev.offsetWidth } : null;
      if (prev) prev.setAttribute("aria-selected", "false");
      a.setAttribute("aria-selected", "true"); tabPill(strip, at); save({ kind: "tab", dir: to > from ? 1 : -1 });
    }
    var href = a.href;
    setTimeout(function () { location.href = href; }, still() ? 0 : STD + 40);
  });

  window.addEventListener("load", function () {
    scanAll();
    var arrive = take();
    if (arrive) document.querySelectorAll(".o-app:not(.o-presenter)").forEach(function (app) {
      var ws = app.querySelector(".o-body > .o-grow");
      if (!ws) return;
      if (arrive.kind === "tab") {
        var strip = ws.querySelector('.o-tabs[role="tablist"]');
        if (strip) panel(strip).forEach(function (el) { enter(el, 24 * arrive.dir, 0); });
      } else {
        // The header, its tabs and the rail stay put; what is under the header rises.
        var head = ws.querySelector("header, .pl-head"), parts = head ? panel(head) : [];
        if (!parts.length) parts = [ws];
        parts.forEach(function (el) { enter(el, 0, 12); });
      }
    });
    new MutationObserver(scanAll).observe(document.body, { childList: true, subtree: true });
  });
})();
