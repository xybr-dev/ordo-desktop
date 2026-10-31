// Ordo mockups: the preparation shell's other destinations on the shared kit (load after
// ordo-kit.js): Masses index and first-run setup, Themes and the Theme Editor, Transfer & Backup,
// Diagnostics, Help and Settings. Each instance owns a working copy of its fixtures, so flows can
// edit and Undo without touching other mockups. Mockup behaviour only.
(function () {
  "use strict";
  var K = window.OrdoKit, esc = K.esc, icon = K.icon, statusLabel = K.statusLabel;
  function copy(x) { return JSON.parse(JSON.stringify(x)); }

  // ── fixtures ──────────────────────────────────────────────────────────────────────────────
  // Today in the fixture is Wed 28 Oct 2026.
  var PARISH = { name: "Parokya ni San Roque", location: "San Roque Parish Church", presiders: ["Fr. Ramon Abellana", "Fr. Jun Tabada"], presider: "Fr. Ramon Abellana" };
  function m(id, time, lang, choir, inc, warn, o) { o = o || {}; return { id: id, time: time, lang: lang, choir: choir, inc: inc, warn: warn, presider: o.presider || "Fr. Ramon Abellana", ant: !!o.ant, archived: !!o.archived, presented: o.presented || null }; }
  var CELS = [
    { id: "c1", when: "upcoming", date: "Sun 1 Nov 2026", title: "All Saints", season: "Solemnity", color: "White", hex: "#F4F1EA", masses: [
      m("m1", "Sat 31 Oct · 18:00", "Cebuano", "Koro San Roque", 0, 0, { ant: true }), m("m2", "Sun 1 Nov · 06:00", "Cebuano", "Koro Sta. Cecilia", 0, 1),
      m("m3", "Sun 1 Nov · 09:00", "English", "Youth Choir", 1, 2, { presider: "Fr. Jun Tabada" }), m("m4", "Sun 1 Nov · 17:00", "Tagalog", "", 0, 0)] },
    { id: "c2", when: "upcoming", date: "Mon 2 Nov 2026", title: "All Souls", season: "Commemoration", color: "Violet", hex: "#6B4C8A", masses: [
      m("m5", "Mon 2 Nov · 06:00", "Cebuano", "Koro San Roque", 0, 2), m("m6", "Mon 2 Nov · 18:00", "Cebuano", "", 3, 0)] },
    { id: "c3", when: "upcoming", date: "Sun 8 Nov 2026", title: "Ika-32 nga Domingo sa Ordinaryong Panahon", season: "Ordinary Time", color: "Green", hex: "#3F7A4E", installed: true, masses: [
      m("m7", "Sun 8 Nov · 06:00", "Cebuano", "Koro Sta. Cecilia", 0, 0), m("m8", "Sun 8 Nov · 09:00", "Cebuano", "Koro San Roque", 0, 0)] },
    { id: "c4", when: "upcoming", date: "Sun 15 Nov 2026", title: "Ika-33 nga Domingo sa Ordinaryong Panahon", season: "Ordinary Time", color: "Green", hex: "#3F7A4E", masses: [] },
    { id: "c5", when: "history", date: "Sun 25 Oct 2026", title: "Ika-30 nga Domingo sa Ordinaryong Panahon", season: "Ordinary Time", color: "Green", hex: "#3F7A4E", masses: [
      m("m9", "Sun 25 Oct · 06:00", "Cebuano", "Koro Sta. Cecilia", 0, 0, { presented: "Presented 25 Oct, 06:02" }), m("m10", "Sun 25 Oct · 09:00", "Cebuano", "Koro San Roque", 0, 0, { presented: "Presented 25 Oct, 09:01 · 2 local changes" }),
      m("m11", "Sun 25 Oct · 17:00", "English", "Youth Choir", 0, 1, { presented: "Presented 25 Oct, 17:04" })] },
    { id: "c6", when: "history", date: "Sun 18 Oct 2026", title: "Ika-29 nga Domingo sa Ordinaryong Panahon", season: "Ordinary Time", color: "Green", hex: "#3F7A4E", masses: [
      m("m12", "Sun 18 Oct · 06:00", "Cebuano", "Koro Sta. Cecilia", 0, 0, { presented: "Presented 18 Oct, 06:00" }), m("m13", "Sun 18 Oct · 09:00", "Cebuano", "Koro San Roque", 0, 0, { presented: "Presented 18 Oct, 09:03" })] },
    { id: "c7", when: "upcoming", archived: true, date: "Sat 7 Nov 2026", title: "Wedding rehearsal (test)", season: "Special", color: "White", hex: "#F4F1EA", masses: [
      m("m14", "Sat 7 Nov · 15:00", "English", "", 2, 0)] }
  ];
  var TEMPLATES = [
    { id: "t1", name: "Sunday Mass · Cebuano", lang: "Cebuano", slots: 18, used: 41 }, { id: "t2", name: "Sunday Mass · English", lang: "English", slots: 18, used: 12 },
    { id: "t3", name: "Weekday Mass · Cebuano", lang: "Cebuano", slots: 11, used: 23 }, { id: "t4", name: "Funeral Mass · Cebuano", lang: "Cebuano", slots: 14, used: 3 }
  ];
  function th(id, name, o) {
    return { id: id, name: name, builtIn: !!o.builtIn, archived: !!o.archived, plans: o.plans || 0, bg: o.bg || "#000000", text: o.text || "#FFFFFF", mode: o.image ? "image" : "solid",
      image: o.image || null, overlay: o.overlay || "#000000", opacity: o.opacity == null ? 35 : o.opacity, focal: o.focal || "Center", size: o.size || 90, min: o.min || 64, weight: o.weight || 800, lh: o.lh || 1.25,
      align: o.align || "Centered", vpos: o.vpos || "Middle", revs: o.revs || [] };
  }
  var THEMES = [
    th("ordo-black", "Ordo Black", { builtIn: true, plans: 3 }),
    th("parish-dark", "Parish Dark", { bg: "#15171C", text: "#F3EFE6", plans: 12, revs: [
      { when: "Tue 20 Oct 2026, 20:41", what: "Lyrics size 88 → 90 px", plans: 11 }, { when: "Sun 4 Oct 2026, 16:12", what: "Background #101216 → #15171C", plans: 9 },
      { when: "Sat 12 Sep 2026, 10:03", what: "Created as a copy of Ordo Black", plans: 0 }] }),
    th("parish-light", "Parish Light", { bg: "#F3ECDD", text: "#2A241E", plans: 4, weight: 700 }),
    th("ordinary-green", "Ordinary Time", { bg: "#1C3626", text: "#F6F1E4", plans: 6, image: { file: "nave-ordinary.jpg", w: 1600, h: 1200 }, opacity: 35 }),
    th("high-contrast", "High Contrast", { bg: "#000000", text: "#FFE45C", plans: 0 }),
    th("advent-violet", "Advent Violet", { bg: "#2B1D3A", text: "#F4EEFA", plans: 2, archived: true })
  ];
  var SPECIMENS = [
    ["lyrics", "Lyrics", { k: "lyrics", h: "Bersikulo 1", b: "Daygon ta ang Ginoo\nsa tanang kasingkasing" }],
    ["response", "Response", { k: "response", h: "TUBAG:", b: "Ug maanaa usab\nsa imong espiritu." }],
    ["ident", "Identification", { k: "song-id", t: "Daygon Ta", s: "R. Villanueva · Koro San Roque" }],
    ["reading", "Reading", { k: "reading", b: "Mao kini ang giingon sa Ginoo: Pag-awit sa kalipay alang kang Jacob.", f: "Jeremias 31:7-9" }]
  ];
  var BACKUPS = [
    { id: "b1", kind: "portable", file: "Parokya-ni-San-Roque_Backup_2026-10-19_1930.ordo", where: "USB drive (E:)", when: "Mon 19 Oct 2026, 19:30", size: "412 MB", state: "Unavailable" },
    { id: "b2", kind: "portable", file: "Parokya-ni-San-Roque_Backup_2026-09-27_2010.ordo", where: "Documents\\Ordo backups", when: "Sun 27 Sep 2026, 20:10", size: "398 MB", state: "Verified" },
    { id: "b3", kind: "safety", file: "Before update to Ordo 0.9.2", where: "Ordo data", when: "Tue 20 Oct 2026, 08:14", size: "409 MB", state: "Verified" },
    { id: "b4", kind: "safety", file: "Before schema update 14", where: "Ordo data", when: "Fri 2 Oct 2026, 21:55", size: "401 MB", state: "Verified" }
  ];
  var PACKAGES = [
    { id: "p1", cel: "c3", title: "Ika-32 nga Domingo sa Ordinaryong Panahon", date: "Sun 8 Nov 2026", versions: [
      { v: 3, when: "Tue 27 Oct 2026, 21:02", from: "Home PC", plans: 2, fp: "7c1e·94b0", current: true },
      { v: 2, when: "Sun 25 Oct 2026, 22:40", from: "Home PC", plans: 2, fp: "51aa·0d3f" }, { v: 1, when: "Thu 22 Oct 2026, 19:15", from: "Home PC", plans: 1, fp: "e09c·2b71" }] },
    { id: "p2", cel: "c5", title: "Ika-30 nga Domingo sa Ordinaryong Panahon", date: "Sun 25 Oct 2026", versions: [
      { v: 2, when: "Sun 25 Oct 2026, 12:30", from: "This computer", plans: 3, fp: "a3f2·61c8", current: true, exported: true }, { v: 1, when: "Fri 23 Oct 2026, 20:05", from: "Home PC", plans: 3, fp: "2d90·c4e7" }] }
  ];
  // Diagnostics: one finding list per concern (Decision 207).
  var DIAG = [
    { id: "data", name: "Data & assets", facts: [["Database", "Schema 15 · integrity check passed"], ["Managed assets", "1,284 files · 1 missing"], ["Storage", "Ordo data · 41.2 GB free"]],
      issues: [{ id: "i-img", t: "A Theme background image is missing", why: "Ordinary Time shows its fallback colour instead of nave-ordinary.jpg. 6 Mass plans use it, and their Celebration Packages can't be exported until the image is replaced or removed.", link: "Open Ordinary Time", go: "themes" }] },
    { id: "fonts", name: "Fonts & rendering", facts: [["Projection fonts", "Atkinson Hyperlegible Next, Noto Sans · verified"], ["Font recovery copy", "Present"], ["Renderer", "Canonical 1920 × 1080 probe matched"]], issues: [] },
    { id: "displays", name: "Displays", facts: [["Congregation display", "EPSON EB-X51 · Display 2"], ["Output Fit", "3%"], ["Last calibration check", "Sun 18 Oct 2026"]],
      issues: [{ id: "i-cal", t: "The congregation display changed since its last calibration check", why: "Display 2 reports a new resolution. Check the calibration pattern before Sunday so the congregation sees the whole frame.", link: "Open Displays", go: "settings", page: "displays" }] },
    { id: "transfer", name: "Transfer & backups", facts: [["Last portable Parish Backup", "Mon 19 Oct 2026 · USB drive (E:), not connected"], ["Changes since", "46"], ["Safety backups", "2 · latest Tue 20 Oct 2026"]],
      issues: [{ id: "i-bak", t: "No portable backup in 9 days", why: "46 changes exist only on this computer. A safety backup on this computer doesn't protect against losing it.", link: "Open Transfer & Backup", go: "transfer" }] },
    { id: "app", name: "Application & system", facts: [["Ordo", "0.9.2 (build 2026.10.18)"], ["System", "Windows 11 · Intel Core i5-8250U · 8 GB · Intel UHD 620"], ["Network", "Not used"]], issues: [] },
    { id: "recent", name: "Recent problems", facts: [["Last 30 days", "1 crash recovered (Sun 18 Oct, 09:41) · 0 failed exports"]], issues: [] }
  ];
  var SHORTCUTS = {
    Global: [["Ctrl+/", "Show or hide keyboard shortcuts"], ["Ctrl+Z", "Undo"], ["Ctrl+Shift+Z", "Redo"], ["Ctrl++  Ctrl+-", "Interface scale up or down"], ["Ctrl+0", "Interface scale 100%"]],
    Masses: [["Ctrl+N", "New Celebration"], ["Ctrl+F", "Search Masses"], ["Enter", "Open the selected Mass plan"]],
    Themes: [["Ctrl+S", "Save Theme"], ["Ctrl+D", "Duplicate Theme"], ["Esc", "Close a panel or dialog"]],
    Editing: [["Ctrl+Enter", "Commit the slide-over"], ["Alt+↑  Alt+↓", "Move the selected item"], ["F2", "Rename"]],
    Presenter: [["Space", "Next Cue"], ["B", "Black"], ["F", "Freeze"], ["Enter", "Take Preview to Program"], ["Esc", "Leave a field; output is unchanged"]]
  };
  var GUIDES = [
    { id: "g-sunday", t: "Prepare a Sunday Mass", b: ["Create the Celebration in Masses, then add a Mass plan from a template or reuse last week's.", "Fill the Song and Reading slots. Readiness says what's left.", "Before Mass, open Presenter: it starts with congregation output black."] },
    { id: "g-transfer", t: "Carry a Celebration to the church computer", b: ["In Masses, open the Celebration's … menu and choose Export Celebration Package.", "Save the .ordo file to a USB drive and eject it in Windows or Linux.", "On the church computer, open Transfer & Backup → Import .ordo, review, and Import celebration."] },
    { id: "g-backup", t: "Back up the parish", b: ["Transfer & Backup → Create parish backup saves everything to one .ordo file.", "Keep it off this computer: a USB drive or another computer.", "Ordo reminds you when there are changes and no backup in seven days."] },
    { id: "g-calibrate", t: "Calibrate the projector", b: ["Settings → Displays: choose the congregation display.", "Show calibration pattern and fix overscan in Windows and the projector first.", "Use Output Fit only for what remains."] }
  ];

  // ── helpers ───────────────────────────────────────────────────────────────────────────────
  function readiness(x) {
    if (!x.inc && !x.warn) return statusLabel("live", "Ready");
    var p = []; if (x.inc) p.push(x.inc + " incomplete"); if (x.warn) p.push(x.warn + " warning" + (x.warn > 1 ? "s" : ""));
    return statusLabel(x.inc ? "danger" : "warning", p.join(" · "), x.inc ? "diamond" : "square");
  }
  function lum(hex) {
    var c = hex.replace("#", ""); if (c.length !== 6) return 0;
    return [0, 2, 4].map(function (i) { var v = parseInt(c.substr(i, 2), 16) / 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); })
      .reduce(function (a, v, i) { return a + v * [0.2126, 0.7152, 0.0722][i]; }, 0);
  }
  function contrast(a, b) { var x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); }
  function hexOk(h) { return /^#[0-9a-fA-F]{6}$/.test(h); }
  function rgba(hex, op) { var c = hex.replace("#", ""); return "rgba(" + [0, 2, 4].map(function (i) { return parseInt(c.substr(i, 2), 16); }).join(",") + "," + (op / 100) + ")"; }
  // A stand-in for a photograph: the mockup never ships images.
  var PHOTO = "radial-gradient(ellipse at 28% 18%, #b9c98f 0, #6f8f5a 18%, #2e5a3e 46%, #10241a 80%)";
  function effective(t) {
    // What the renderer composes (Decisions 100, 224): an image under its overlay, or the fallback colour.
    var img = t.mode === "image" && t.image && !t.image.missing;
    var bgc = img ? mix(t.bg, t.overlay, t.opacity) : t.bg;
    return { img: img, bgc: bgc, ratio: contrast(t.text, bgc) };
  }
  function mix(a, b, op) {
    var p = function (h, i) { return parseInt(h.replace("#", "").substr(i, 2), 16); }, k = op / 100;
    return "#" + [0, 2, 4].map(function (i) { var v = Math.round(p(a, i) * (1 - k) + p(b, i) * k); return ("0" + v.toString(16)).slice(-2); }).join("");
  }
  function themeSlide(t, cue) {
    var e = effective(t), pos = { Center: "50% 50%", Top: "50% 20%", Bottom: "50% 80%", Left: "20% 50%", Right: "80% 50%" }[t.focal] || "50% 50%";
    var bg = e.img ? "linear-gradient(" + rgba(t.overlay, t.opacity) + "," + rgba(t.overlay, t.opacity) + ")," + PHOTO.replace("ellipse at 28% 18%", "ellipse at " + pos) : t.bg;
    var st = "--o-projection-bg:" + (hexOk(t.bg) ? t.bg : "#000") + ";--o-projection-text:" + (hexOk(t.text) ? t.text : "#fff") + ";background:" + bg +
      ";--sh-size:" + (t.size / 19.2) + ";--sh-rsize:" + (t.size * 140 / 90 / 19.2) + ";--sh-w:" + t.weight + ";--sh-lh:" + t.lh +
      ";--sh-align:" + (t.align === "Left" ? "left" : "center") + ";--sh-items:" + (t.align === "Left" ? "flex-start" : "center") + ";--sh-v:" + (t.vpos === "Upper" ? "flex-start" : "center");
    return K.slide(cue).replace('class="o-slide"', 'class="o-slide sh-th" style="' + st + '"');
  }
  function sel(path, value, options, label) {
    return '<select class="pl-input pl-input--sel" data-in="' + path + '" aria-label="' + esc(label) + '">' +
      options.map(function (o) { var v = Array.isArray(o) ? o[0] : o, l = Array.isArray(o) ? o[1] : o; return "<option value=\"" + esc(v) + "\"" + (String(value) === String(v) ? " selected" : "") + ">" + esc(l) + "</option>"; }).join("") + "</select>";
  }
  function field(label, html, hint) { return '<label class="sh-field"><span class="o-label">' + label + "</span>" + html + (hint ? '<span class="o-meta">' + hint + "</span>" : "") + "</label>"; }
  function input(path, value, o) { o = o || {}; return '<input class="pl-input' + (o.cls ? " " + o.cls : "") + '" data-in="' + path + '" data-focus-key="' + path + '" value="' + esc(value) + '"' + (o.ph ? ' placeholder="' + esc(o.ph) + '"' : "") + (o.ro ? " readonly" : "") + ">"; }
  function seg(path, value, options, label) {
    return '<div class="o-segmented" role="group" aria-label="' + esc(label) + '">' + options.map(function (o) { return '<button data-a="set" data-path="' + path + '" data-v="' + esc(o) + '" aria-pressed="' + (String(value) === String(o)) + '">' + esc(o) + "</button>"; }).join("") + "</div>";
  }
  function closeTab(label) { return '<button class="pl-close-tab" data-a="close" aria-label="' + label + '" title="' + label + '">' + icon("chevron-right") + "</button>"; }
  function kbd(k) { return k.split("  ").map(function (one) { return one.replace(/\+\+$/, "+PLUS").split("+").map(function (x) { return x === "PLUS" ? "+" : x; }).map(function (x) { return '<kbd class="o-kbd">' + esc(x || "+") + "</kbd>"; }).join("+"); }).join(" "); }

  var DESTS = { Masses: "masses", Themes: "themes", "Transfer & Backup": "transfer", Diagnostics: "diagnostics", Help: "help", Settings: "settings" };
  var RAIL = { masses: "Masses", themes: "Themes", transfer: "Transfer & Backup", diagnostics: "Diagnostics", help: "Help", settings: "Settings" };

  function shell(wrapper, opts) {
    opts = opts || {};
    var W = opts.w || 1366, H = opts.h || 768;
    var app = document.createElement("div");
    app.className = "o-app sh-app"; app.tabIndex = -1;
    wrapper.classList.add("o-fit"); wrapper.appendChild(app);

    var S = {
      dest: opts.dest || "masses", scale: opts.scale || 1, toast: null, undo: null, overlay: null, dialog: null, menu: null, keys: false,
      parish: copy(PARISH), firstRun: false, setup: { mode: null, name: "", loc: "", pres: [], draft: "" },
      cels: copy(CELS), templates: copy(TEMPLATES), mtab: "upcoming", q: "", f: { lang: "", choir: "", ready: "" }, collapsed: {}, newCel: null,
      themes: copy(THEMES), dflt: "parish-dark", tarch: false, tview: "library", ed: null, review: null, hist: null, dup: null,
      backup: { days: 9, changes: 46 }, backups: copy(BACKUPS), packages: copy(PACKAGES), xview: "home", exp: null, mk: null, imp: null, rst: null, damaged: false,
      diag: { state: "done", when: "Tue 27 Oct 2026, 21:14", open: {}, pct: 0, cur: null, cancelled: null, healthy: false },
      help: "keys", spage: opts.spage || "parish", disp: { count: 2, cong: "2", unavailable: false, fit: 3, pattern: false, identify: false, changed: false, checked: "Sun 18 Oct 2026, 08:40" }
    };
    var history = [];
    function snapshot() { history.push(JSON.stringify({ cels: S.cels, themes: S.themes, dflt: S.dflt, parish: S.parish, backups: S.backups, packages: S.packages, backup: S.backup })); }
    function done(msg, undoable) { S.toast = msg; S.undo = undoable ? true : null; }
    function undo() { var h = history.pop(); if (!h) return; var o = JSON.parse(h); Object.keys(o).forEach(function (k) { S[k] = o[k]; }); S.toast = "Undone"; S.undo = null; if (S.ed) S.ed = null, S.tview = "library"; }
    function cel(id) { return S.cels.filter(function (c) { return c.id === id; })[0]; }
    function theme(id) { return S.themes.filter(function (t) { return t.id === id; })[0]; }
    function remindOn() { return S.backup.days >= 7 && S.backup.changes > 0; }

    // ── Masses index (Decisions 37, 64, 83–85, 218, 250) ──────────────────────────────────────
    function reminder() {
      return remindOn() ? '<button class="o-btn o-btn--warning sh-remind" data-a="go" data-dest="transfer" title="Open Transfer & Backup">' + icon("triangle-alert") + "No backup in " + S.backup.days + " days · " + S.backup.changes + " changes</button>" : "";
    }
    function mhead() {
      var tabs = [["upcoming", "Upcoming"], ["history", "History"], ["archive", "Archive"], ["templates", "Templates"]];
      return '<header class="sh-head"><h1 class="o-title">Masses</h1><div class="o-tabs sh-tabs" role="tablist">' +
        tabs.map(function (t) { return '<button role="tab" data-a="mtab" data-v="' + t[0] + '" aria-selected="' + (S.mtab === t[0]) + '">' + t[1] + "</button>"; }).join("") + "</div>" +
        '<span class="o-grow"></span>' + reminder() + (S.mtab === "templates" ? '<a class="o-btn o-btn--primary" href="mass-templates.html">' + icon("plus") + "New template</a>"
        : '<button class="o-btn o-btn--primary" data-a="newcel">' + icon("plus") + "New Celebration</button>") + "</header>";
    }
    function matches(x) {
      if (S.f.lang && x.lang !== S.f.lang) return false;
      if (S.f.choir && (x.choir || "No choir") !== S.f.choir) return false;
      if (S.f.ready === "Ready" && (x.inc || x.warn)) return false;
      if (S.f.ready === "Needs work" && !(x.inc || x.warn)) return false;
      return true;
    }
    function mtools() {
      return '<div class="sh-tools"><label class="o-field sh-search">' + icon("search") + input("q", S.q, { ph: "Search Celebrations, choirs, presiders" }) + "</label>" +
        sel("f.lang", S.f.lang, [["", "All languages"], "Cebuano", "English", "Tagalog"], "Language") +
        sel("f.choir", S.f.choir, [["", "All choirs"], "Koro San Roque", "Koro Sta. Cecilia", "Youth Choir", "No choir"], "Choir") +
        sel("f.ready", S.f.ready, [["", "Any readiness"], "Ready", "Needs work"], "Readiness") + "</div>";
    }
    function massRow(c, x, past) {
      return '<div class="o-row sh-mass" data-a="open" data-id="' + x.id + '" role="link" tabindex="0">' +
        '<span class="sh-time">' + esc(x.time) + (x.ant ? ' <span class="pl-chip">Anticipated</span>' : "") + "</span>" +
        '<span class="sh-lang">' + esc(x.lang) + "</span><span class=\"sh-choir" + (x.choir ? "" : " o-muted") + '">' + esc(x.choir || "No choir") + "</span>" +
        '<span class="sh-pres o-muted">' + esc(x.presider) + "</span>" +
        '<span class="sh-rd">' + (past ? '<span class="o-meta">' + esc(x.presented || "Not presented") + "</span>" : readiness(x)) + "</span>" +
        (past ? '<button class="o-btn o-btn--quiet" data-a="reuseplan" data-id="' + x.id + '">Reuse</button>' : "") +
        icon("chevron-right", "sh-go") + "</div>";
    }
    function rollup(c) {
      var ms = c.masses.filter(function (x) { return !x.archived; }), ok = ms.filter(function (x) { return !x.inc && !x.warn; }).length;
      if (!ms.length) return statusLabel("danger", "No Mass plans", "diamond");
      return ok === ms.length ? statusLabel("live", "All " + ms.length + " ready") : statusLabel(ms.some(function (x) { return x.inc; }) ? "danger" : "warning", ok + " of " + ms.length + " ready", ms.some(function (x) { return x.inc; }) ? "diamond" : "square");
    }
    function celMenu(c) {
      var it = [["addmass", "plus", "Add another Mass"], ["reuse", "copy", "Reuse for another Celebration"], ["export", "package", "Export Celebration Package"], ["archivecel", "archive", "Archive Celebration"]];
      return '<div class="o-menu sh-menu" role="menu">' + it.map(function (i) { return '<button class="o-row" role="menuitem" data-a="' + i[0] + '" data-id="' + c.id + '">' + icon(i[1]) + i[2] + "</button>"; }).join("") + "</div>";
    }
    function celGroup(c, past) {
      var ms = c.masses.filter(function (x) { return !x.archived && matches(x); });
      var coll = S.collapsed[c.id] == null ? past : S.collapsed[c.id];
      var head = '<div class="sh-celhead">' + '<button class="sg-expander" data-a="collapse" data-id="' + c.id + '" aria-expanded="' + !coll + '" aria-label="' + (coll ? "Expand" : "Collapse") + '">' + icon(coll ? "chevron-right" : "chevron-down") + "</button>" +
        '<span class="sh-swatch" style="background:' + c.hex + '" title="' + esc(c.color) + '"></span>' +
        '<div class="o-grow"><div class="sh-celtitle"><b>' + esc(c.title) + "</b>" + (c.installed ? ' <span class="pl-chip">' + icon("package") + "Installed package</span>" : "") + "</div>" +
        '<div class="o-meta">' + esc(c.date) + " · " + esc(c.season) + " · " + esc(c.color) + " · " + c.masses.length + " Mass" + (c.masses.length === 1 ? "" : "es") + "</div></div>" +
        (past ? "" : rollup(c)) +
        (past ? "" : '<button class="o-btn o-btn--quiet" data-a="addmass" data-id="' + c.id + '">' + icon("plus") + "Add Mass</button>") +
        '<span class="sh-rel"><button class="o-btn o-btn--quiet o-btn--icon" data-a="celmenu" data-id="' + c.id + '" aria-label="More for ' + esc(c.title) + '" aria-expanded="' + (S.menu === c.id) + '">' + icon("ellipsis") + "</button>" + (S.menu === c.id ? celMenu(c) : "") + "</span></div>";
      var rows = coll ? "" : ms.length ? ms.map(function (x) { return massRow(c, x, past); }).join("") :
        c.masses.length ? '<div class="sh-none o-meta">No Mass plans match the filters.</div>' : '<div class="sh-none"><span class="o-meta">No Mass plans yet. A Celebration needs one before it can be presented or exported.</span><button class="o-btn" data-a="addmass" data-id="' + c.id + '">' + icon("plus") + "Add Mass</button></div>";
      return '<section class="sh-cel">' + head + rows + "</section>";
    }
    function masses() {
      if (S.mtab === "templates") return '<div class="sh-page">' + mhead() + '<div class="sh-scroll"><div class="sh-col">' +
        '<p class="o-meta">A template sets the slots a new Mass plan starts with. Changing a template never changes plans already made from it (Decision 76).</p><div class="sh-list">' +
        S.templates.map(function (t) { return '<div class="o-row sh-trow">' + icon("layout-template") + '<div class="o-grow"><b>' + esc(t.name) + '</b><span class="o-meta">' + t.slots + " slots · used for " + t.used + " Mass plans</span></div>" +
          '<a class="o-btn" href="mass-templates.html">Edit</a></div>'; }).join("") + "</div></div></div></div>";
      if (S.mtab === "archive") {
        var arch = S.cels.filter(function (c) { return c.archived; });
        return '<div class="sh-page">' + mhead() + '<div class="sh-scroll"><div class="sh-col"><p class="o-meta">Archived plans are hidden from Upcoming and History but stay searchable. Restore brings one back; deleting is permanent and only possible here (Decision 37).</p>' +
          (arch.length ? arch.map(function (c) { return '<section class="sh-cel"><div class="sh-celhead"><span class="sh-swatch" style="background:' + c.hex + '"></span><div class="o-grow"><b>' + esc(c.title) + '</b><div class="o-meta">' + esc(c.date) + " · archived " + c.masses.length + " Mass</div></div>" +
            '<button class="o-btn" data-a="restorecel" data-id="' + c.id + '">Restore</button><button class="o-btn o-btn--danger" data-a="toast" data-msg="Asks to confirm, listing the plan-owned content that will be removed">Delete permanently…</button></div></section>'; }).join("")
            : '<div class="pl-empty"><b>Nothing archived</b><span class="o-meta">Archive a Celebration from its … menu.</span></div>') + "</div></div></div>";
      }
      var past = S.mtab === "history", q = S.q.trim().toLowerCase();
      var list = S.cels.filter(function (c) { return !c.archived && c.when === (past ? "history" : "upcoming"); }).filter(function (c) {
        return !q || (c.title + " " + c.date + " " + c.masses.map(function (x) { return x.choir + " " + x.presider + " " + x.lang; }).join(" ")).toLowerCase().indexOf(q) >= 0;
      });
      if (!past && !S.cels.some(function (c) { return !c.archived; })) return '<div class="sh-page">' + mhead() + '<div class="sh-scroll"><div class="sh-col sh-welcome">' +
        '<h2 class="sh-h2">' + esc(S.parish.name) + " is ready</h2><p>Nothing is planned yet. Start with the next Celebration, bring one in from another computer, or set up the templates new Mass plans start from.</p>" +
        '<div class="sh-choices"><button class="sh-choice" data-a="newcel">' + icon("calendar-plus") + "<b>Create first Celebration</b><span class=\"o-meta\">A date and title, then its Masses.</span></button>" +
        '<button class="sh-choice" data-a="go" data-dest="transfer" data-x="import">' + icon("file-down") + "<b>Import .ordo</b><span class=\"o-meta\">A Celebration Package prepared on another computer.</span></button>" +
        '<button class="sh-choice" data-a="mtab" data-v="templates">' + icon("layout-template") + "<b>Create or manage templates</b><span class=\"o-meta\">The slots a new Mass plan starts with.</span></button></div></div></div></div>";
      var body = list.length ? list.map(function (c) { return celGroup(c, past); }).join("") : '<div class="pl-empty"><b>No Celebrations match</b><button class="o-btn" data-a="clearm">Clear search and filters</button></div>';
      return '<div class="sh-page">' + mhead() + mtools() + '<div class="sh-scroll"><div class="sh-col sh-col--wide">' +
        (past ? '<p class="o-meta">Celebrations whose date has passed. A past plan opens read-only; <b>Edit past plan</b> is a deliberate choice (Decision 37).</p>' : "") + body + "</div></div></div>";
    }
    function newCel() {
      var n = S.newCel, ok = n.title.trim() && n.date.trim();
      return '<div class="o-scrim" data-a="close"></div><div class="o-slideover sh-over" role="dialog" aria-modal="true" aria-labelledby="sh-nc">' + closeTab("Cancel") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-title" id="sh-nc">New Celebration</h2><div class="o-meta">The Celebration comes first; its Masses hang under it (Decision 84).</div></div></div>' +
        '<div class="o-overlay-body sh-form">' + field("Date", input("newCel.date", n.date)) + field("Title", input("newCel.title", n.title, { ph: "e.g. Ika-34 nga Domingo · Christ the King" })) +
        '<div class="sh-two">' + field("Season or occasion", sel("newCel.season", n.season, ["Ordinary Time", "Advent", "Christmas", "Lent", "Easter", "Solemnity", "Feast", "Special"], "Season")) +
        field("Liturgical colour", sel("newCel.color", n.color, ["Green", "White", "Violet", "Red", "Rose", "Black"], "Colour")) + "</div>" +
        '<div class="o-label">First Mass plan</div><div class="sh-radios">' +
        [["template", "Create from template", "Next: its time, language, choir, presider and template."], ["reuse", "Reuse an earlier Mass plan", "Next: pick the plan and its new time."], ["later", "Do this later", "The Celebration shows <b>No Mass plans</b> until one is added."]]
          .map(function (r) { return '<label class="sh-radio"><input type="radio" name="nc-first" data-a="set" data-path="newCel.first" data-v="' + r[0] + '"' + (n.first === r[0] ? " checked" : "") + '><span><b>' + r[1] + '</b><br><span class="o-meta">' + r[2] + "</span></span></label>"; }).join("") + "</div></div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-a="close">Cancel</button><button class="o-btn o-btn--primary" data-a="createcel"' + (ok ? "" : " disabled") + ">" + (n.first === "later" ? "Create Celebration" : "Next: add the Mass") + "</button></div></div>";
    }
    // ── Add Mass to a Celebration (Decisions 74, 79, 81, 84, 85; Decision 288) ──────────
    var DAYN = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"], MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    var CHOIRS = ["Koro San Roque", "Koro Sta. Cecilia", "Youth Choir"], VISITING = ["Msgr. Cris Garcia", "Fr. Noel Ybañez"];
    function short(d) { return DAYN[d.getDay()] + " " + d.getDate() + " " + MON[d.getMonth()]; }
    function celDay(c, prev) { var d = new Date(c.date.replace(/^\w+ /, "")); if (isNaN(d)) return c.date; if (prev) d.setDate(d.getDate() - 1); return short(d); }
    function recTemplate(c, lang) {
      var ts = S.templates.filter(function (t) { return t.lang === lang && !/Funeral/.test(t.name); });
      var weekday = !/^Sun/.test(c.date) && c.season !== "Solemnity";
      return ts.filter(function (t) { return weekday ? /Weekday/.test(t.name) : /Sunday/.test(t.name); })[0] || ts[0] || null;
    }
    function plansOf(list) { var out = []; list.forEach(function (c) { c.masses.forEach(function (x) { if (!x.archived) out.push({ c: c, x: x }); }); }); return out; }
    function planById(id) { return plansOf(S.cels).filter(function (p) { return p.x.id === id; })[0] || null; }
    function presiderField(path, v) {
      var regular = S.parish.presiders, names = regular.concat(VISITING);
      return field("Presider", '<input class="pl-input" list="sh-pres" data-in="' + path + '" data-focus-key="' + path + '" value="' + esc(v) + '" placeholder="Type or pick a name"><datalist id="sh-pres">' +
        names.map(function (n) { return '<option value="' + esc(n) + '">'; }).join("") + "</datalist>",
        !v.trim() ? "Can stay empty until it’s known." : v === S.parish.presider ? "The parish default." : regular.indexOf(v) >= 0 ? "A regular presider." : "A visiting presider: kept on this Mass only, not added to the regular list.");
    }
    function openAddMass(celId, method) {
      var c = cel(celId), first = c.masses[0];
      S.am = { cel: celId, method: method || "template", day: "same", ant: false, time: c.masses.length ? "18:00" : "09:00", lang: first ? first.lang : "Cebuano", choir: "",
        presider: S.parish.presider || "", location: S.parish.location || "", tpl: "", src: null };
      var r = recTemplate(c, S.am.lang); S.am.tpl = r ? r.id : "";
      S.overlay = "addmass";
    }
    function addMassOver() {
      var a = S.am, c = cel(a.cel), rec = recTemplate(c, a.lang), src = a.src ? planById(a.src) : null;
      var methods = [["template", "From a template"], ["reuse", "Reuse an earlier plan"]].concat(c.masses.length ? [["dup", "Duplicate a Mass here"]] : []);
      var start = '<div class="sh-field"><span class="o-label">Start from</span><div class="o-segmented" role="group" aria-label="Start from">' + methods.map(function (mt) {
        return '<button data-a="ammethod" data-v="' + mt[0] + '" aria-pressed="' + (a.method === mt[0]) + '">' + mt[1] + "</button>"; }).join("") + "</div></div>";
      var when = '<div class="sh-field"><span class="o-label">Date</span><div class="o-segmented" role="group" aria-label="Date">' +
        [["same", celDay(c)], ["prev", celDay(c, true) + ", the evening before"]].map(function (d) { return '<button data-a="set" data-path="am.day" data-v="' + d[0] + '" aria-pressed="' + (a.day === d[0]) + '">' + esc(d[1]) + "</button>"; }).join("") + "</div>" +
        (a.day === "prev" ? '<label class="sh-check"><input type="checkbox" data-a="amant"' + (a.ant ? " checked" : "") + "> Anticipated Mass of this Celebration</label>" +
          '<span class="o-meta">' + (a.ant ? "Shows as Anticipated and uses this Celebration’s readings." : "Ordo doesn’t decide this from the time. Tick it to confirm, or the plan is listed without the Anticipated label.") + "</span>" : "") + "</div>";
      var who = '<div class="sh-two">' + field("Time", input("am.time", a.time, { ph: "e.g. 18:00" }), /^\d{1,2}:\d{2}$/.test(a.time.trim()) ? "" : "Use 24-hour time, like 17:30.") +
        field("Language", sel("am.lang", a.lang, ["Cebuano", "English", "Tagalog"], "Language")) + "</div>" +
        '<div class="sh-two">' + field("Choir", sel("am.choir", a.choir, [["", "No choir yet"]].concat(CHOIRS), "Choir"), "Filters Song choices in the plan.") + presiderField("am.presider", a.presider) + "</div>" +
        field("Location", input("am.location", a.location, { ph: "None" }), a.location && a.location === S.parish.location ? "The parish default." : "");
      var how = "";
      if (a.method === "template") {
        var ts = S.templates.filter(function (t) { return t.lang === a.lang; });
        how = ts.length ? field("Template", sel("am.tpl", a.tpl, ts.map(function (t) { return [t.id, t.name + (rec && t.id === rec.id ? " · Recommended" : "")]; }).concat([["", "No template: start empty"]]), "Template"),
            a.tpl ? (rec && a.tpl === rec.id ? "The best fit for " + esc(a.lang) + " on this day. Any template can be chosen." : "Chosen instead of the recommendation.") + " Song and Reading slots start empty." : "An empty plan, with a readiness warning until something is added.")
          : '<div class="o-notice" data-state="warning">' + icon("info") + "<div><b>No " + esc(a.lang) + " template yet.</b> The plan starts empty with a readiness warning, rather than borrowing another language’s template (Decision 79).</div></div>";
      } else {
        var pool = a.method === "dup" ? plansOf([c]) : plansOf(S.cels.filter(function (x) { return x.when === "history" && !x.archived; }));
        how = '<div class="sh-field"><span class="o-label">' + (a.method === "dup" ? "Mass to duplicate" : "Earlier Mass plan") + '</span><div class="sh-list">' + pool.map(function (p) {
          return '<label class="o-row sh-trow sh-pick"><input type="radio" name="am-src" data-a="amsrc" data-id="' + p.x.id + '"' + (a.src === p.x.id ? " checked" : "") + '><div class="o-grow"><b>' + esc(p.x.time) + " · " + esc(p.x.lang) + "</b>" +
            '<span class="o-meta">' + (a.method === "reuse" ? esc(p.c.title) + " · " : "") + esc(p.x.choir || "No choir") + " · " + esc(p.x.presider) + "</span></div></label>";
        }).join("") + "</div></div>";
        if (src) {
          var other = src.x.lang !== a.lang, keep, left;
          if (a.method === "reuse") { keep = ["Mass items, Songs and Arrangements", "Responses, Cues and Theme choices"]; left = ["Readings, marked Needs review: they belong to " + esc(src.c.date)]; }
          else if (other) { keep = ["Song slots, matched by role", "Arrangements that have a version in " + esc(a.lang)]; left = ["Responses come from the " + esc(a.lang) + " template, not translated", "Readings and other " + esc(src.x.lang) + " text"]; }
          else { keep = ["Everything in the plan, as an independent copy"]; left = []; }
          how += '<div class="sh-summary"><div><b>Kept</b><ul class="pl-changes">' + keep.map(function (k) { return "<li>" + k + "</li>"; }).join("") + "</ul></div>" +
            (left.length ? '<div><b>Left to finish</b><ul class="pl-changes">' + left.map(function (k) { return "<li>" + k + "</li>"; }).join("") + "</ul></div>" : "") + "</div>";
        }
      }
      var ok = /^\d{1,2}:\d{2}$/.test(a.time.trim()) && (a.method === "template" || src);
      return '<div class="o-scrim" data-a="close"></div><div class="o-slideover sh-over" role="dialog" aria-modal="true" aria-labelledby="sh-am">' + closeTab("Cancel") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-title" id="sh-am">' + (c.masses.length ? "Add another Mass" : "Add a Mass") + '</h2><div class="o-meta">' + esc(c.title) + " · " + esc(c.date) + "</div></div></div>" +
        '<div class="o-overlay-body sh-form">' + start + (a.method === "template" ? when + who + how : how.split('<div class="sh-summary">')[0] + when + who + (src ? '<div class="sh-summary">' + how.split('<div class="sh-summary">')[1] : "")) + "</div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-a="close">Cancel</button><button class="o-btn o-btn--primary" data-a="createmass"' + (ok ? "" : " disabled") + ">" +
        (a.method === "dup" ? "Duplicate Mass" : a.method === "reuse" ? "Reuse plan" : "Create Mass plan") + "</button></div></div>";
    }
    function sortMasses(c) { c.masses.sort(function (x, y) { return x.time.slice(-5).localeCompare(y.time.slice(-5)) || 0; }); c.masses.sort(function (x, y) { return (y.ant ? 1 : 0) - (x.ant ? 1 : 0); }); }
    function createMass() {
      var a = S.am, c = cel(a.cel), src = a.src ? planById(a.src) : null, inc = 0, warn = 0; snapshot();
      var label = (a.day === "prev" ? celDay(c, true) : celDay(c)) + " · " + a.time.trim();
      if (a.method === "template") { if (a.tpl) inc = 8; else warn = 1; }
      else if (a.method === "reuse") warn = 2;
      else if (src.x.lang !== a.lang) { inc = 4; warn = src.x.warn; } else { inc = src.x.inc; warn = src.x.warn; }
      c.masses.push(m("n" + Date.now(), label, a.lang, a.choir, inc, warn, { presider: a.presider.trim() || "No presider", ant: a.day === "prev" && a.ant })); sortMasses(c);
      S.collapsed[a.cel] = false; S.overlay = null;
      done(label + " · " + a.lang + " added to " + c.title + ". It opens in the Mass Planner.", true);
    }

    // ── Reuse for another Celebration (Decisions 36, 74; Decision 288) ───────────────
    function openReuse(celId, planId) {
      var c = cel(celId), target = S.cels.filter(function (x) { return x.when === "upcoming" && !x.archived && x.id !== celId && !x.masses.length; })[0] ||
        S.cels.filter(function (x) { return x.when === "upcoming" && !x.archived && x.id !== celId; })[0];
      S.ru = { src: celId, pick: {}, times: {}, into: target ? "existing" : "new", target: target ? target.id : "", nd: "Sun 22 Nov 2026", nt: "", ns: "Solemnity" };
      c.masses.forEach(function (x) { if (!x.archived) { S.ru.pick[x.id] = !planId || planId === x.id; S.ru.times[x.id] = x.time.slice(-5); } });
      S.overlay = "reuse";
    }
    function reuseTarget() { var r = S.ru; return r.into === "existing" ? cel(r.target) : { title: r.nt.trim() || "the new Celebration", date: r.nd, season: r.ns }; }
    function reuseOver() {
      var r = S.ru, c = cel(r.src), tgt = reuseTarget(), ms = c.masses.filter(function (x) { return !x.archived; }), n = ms.filter(function (x) { return r.pick[x.id]; }).length;
      var targets = S.cels.filter(function (x) { return x.when === "upcoming" && !x.archived && x.id !== r.src; });
      var plans = '<div class="sh-field"><span class="o-label">Mass plans to copy</span><div class="sh-list">' + ms.map(function (x) {
        return '<div class="o-row sh-trow sh-pick"><input type="checkbox" data-a="rupick" data-id="' + x.id + '"' + (r.pick[x.id] ? " checked" : "") + ' aria-label="Copy ' + esc(x.time) + '">' +
          '<div class="o-grow"><b>' + esc(x.time.slice(-5)) + " · " + esc(x.lang) + '</b><span class="o-meta">' + esc(x.choir || "No choir") + " · " + esc(x.presider) + "</span></div>" +
          (r.pick[x.id] ? '<label class="sh-rutime"><span class="o-meta">New time</span>' + input("ru.times." + x.id, r.times[x.id], { cls: "sh-time-in" }) + "</label>" : "") + "</div>";
      }).join("") + "</div></div>";
      var into = '<div class="sh-field"><span class="o-label">Into</span><div class="o-segmented" role="group" aria-label="Into">' +
        [["existing", "An upcoming Celebration"], ["new", "A new Celebration"]].map(function (o) { return '<button data-a="set" data-path="ru.into" data-v="' + o[0] + '" aria-pressed="' + (r.into === o[0]) + '">' + o[1] + "</button>"; }).join("") + "</div></div>" +
        (r.into === "existing" ? field("Celebration", sel("ru.target", r.target, targets.map(function (x) { return [x.id, x.title + " · " + x.date + (x.masses.length ? " · " + x.masses.length + " Mass" + (x.masses.length === 1 ? "" : "es") + " already" : "")]; }), "Celebration"))
          : field("Date", input("ru.nd", r.nd)) + field("Title", input("ru.nt", r.nt, { ph: "e.g. Christ the King" })) + field("Season or occasion", sel("ru.ns", r.ns, ["Ordinary Time", "Advent", "Christmas", "Lent", "Easter", "Solemnity", "Feast", "Special"], "Season")));
      var mismatch = tgt.season && tgt.season !== c.season;
      var summary = '<div class="sh-summary"><div><b>Copied</b><ul class="pl-changes"><li>Mass items in order, Songs and Arrangements</li><li>Responses, Cues, Theme choices and changes made for that Mass</li><li>Language, choir, presider and location</li></ul></div>' +
        '<div><b>Needs review</b><ul class="pl-changes"><li>Readings: they were for ' + esc(c.date) + ", so each plan gets <b>Needs review</b> until they’re replaced or confirmed</li>" +
        (mismatch ? "<li>Seasonal items: the source was " + esc(c.season) + ", the target is " + esc(tgt.season) + ". Check the Gloria, Creed and seasonal Songs</li>" : "") + "</ul></div>" +
        '<div><b>Not copied</b><ul class="pl-changes"><li>The old Celebration and date</li><li>Presentation snapshots, sessions and history</li></ul></div></div>';
      var ok = n && (r.into === "existing" ? !!r.target : r.nt.trim() && r.nd.trim()) && ms.every(function (x) { return !r.pick[x.id] || /^\d{1,2}:\d{2}$/.test(r.times[x.id].trim()); });
      return '<div class="o-scrim" data-a="close"></div><div class="o-slideover sh-over" role="dialog" aria-modal="true" aria-labelledby="sh-ru">' + closeTab("Cancel") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-title" id="sh-ru">Reuse for another Celebration</h2><div class="o-meta">From ' + esc(c.title) + " · " + esc(c.date) + "</div></div></div>" +
        '<div class="o-overlay-body sh-form">' + plans + into + summary + '<p class="o-meta">The copies are independent: later edits to either plan don’t affect the other. ' + esc(c.title) + " keeps its plans.</p></div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-a="close">Cancel</button><button class="o-btn o-btn--primary" data-a="doreuse"' + (ok ? "" : " disabled") + ">Reuse " + n + " plan" + (n === 1 ? "" : "s") + "</button></div></div>";
    }
    function doReuse() {
      var r = S.ru, c = cel(r.src), tgt; snapshot();
      if (r.into === "existing") tgt = cel(r.target);
      else {
        tgt = { id: "c" + Date.now(), when: "upcoming", date: r.nd.trim(), title: r.nt.trim(), season: r.ns, color: "White", hex: "#F4F1EA", masses: [] };
        S.cels.splice(S.cels.filter(function (x) { return x.when === "upcoming" && !x.archived; }).length, 0, tgt);
      }
      var k = 0;
      c.masses.forEach(function (x) { if (!x.archived && r.pick[x.id]) tgt.masses.push(m("r" + Date.now() + k++, celDay(tgt) + " · " + r.times[x.id].trim(), x.lang, x.choir, 0, 2, { presider: x.presider })); });
      sortMasses(tgt); S.collapsed[tgt.id] = false; S.mtab = "upcoming"; S.overlay = null;
      done(k + " plan" + (k === 1 ? "" : "s") + " reused into " + tgt.title + ". Readings need review.", true);
    }
    function setupView() {
      var s = S.setup, card = function (mode, ic, t, d) { return '<button class="sh-choice' + (s.mode === mode ? " is-on" : "") + '" data-a="set" data-path="setup.mode" data-v="' + mode + '" aria-pressed="' + (s.mode === mode) + '">' + icon(ic) + "<b>" + t + '</b><span class="o-meta">' + d + "</span></button>"; };
      var form = "";
      if (s.mode === "new") form = '<div class="sh-setup-form">' + field("Parish name", input("setup.name", s.name, { ph: "e.g. Parokya ni San Roque" }), "Required. Shown on Parish Backups and in Settings.") +
        field("Default Mass location", input("setup.loc", s.loc, { ph: "Optional" })) +
        '<div class="sh-field"><span class="o-label">Regular presiders</span><div class="sg-chips">' + s.pres.map(function (p, i) { return '<span class="pl-chip">' + esc(p) + (i === 0 ? " · default" : "") + '<button class="sh-x" data-a="unpres" data-i="' + i + '" aria-label="Remove ' + esc(p) + '">' + icon("x") + "</button></span>"; }).join("") + "</div>" +
        '<div class="sh-inline">' + input("setup.draft", s.draft, { ph: "Add a presider (optional)" }) + '<button class="o-btn" data-a="addpres"' + (s.draft.trim() ? "" : " disabled") + ">Add</button></div>" +
        '<span class="o-meta">The first one becomes the default presider for new Mass plans. All of this can change later in Settings → Parish.</span></div>' +
        '<div class="sh-row-end"><button class="o-btn o-btn--primary o-btn--lg" data-a="startparish"' + (s.name.trim() ? "" : " disabled") + ">Start parish</button></div></div>";
      if (s.mode === "restore") form = '<div class="sh-setup-form"><p>Choose a Parish Backup <code>.ordo</code> file. Ordo checks it completely before anything is written, then opens the restore review. There is no current parish to replace.</p>' +
        '<div class="sh-row-end"><button class="o-btn o-btn--primary o-btn--lg" data-a="setuprestore">' + icon("folder-open") + "Choose backup file…</button></div></div>";
      return '<div class="sh-setup"><div class="sh-setup-card"><div class="sh-brand">' + icon("church", "o-icon-20") + "Ordo</div><h1 class=\"o-title\">Set up Ordo</h1>" +
        "<p>Ordo keeps one parish's Masses, Songs and Themes on this computer. Nothing is sent anywhere.</p>" +
        '<div class="sh-choices sh-choices--2">' + card("new", "plus", "Start a new parish", "An empty parish with Ordo's built-in Theme.") + card("restore", "archive-restore", "Restore a parish backup", "Move a parish from another computer.") + "</div>" + form + "</div></div>";
    }

    // ── Themes (Decisions 20, 22, 95–104, 222, 224, 225) ─────────────────────────────────────
    function tcard(t) {
      var st = [];
      if (t.builtIn) st.push('<span class="pl-chip">' + icon("lock") + "Built in</span>");
      if (S.dflt === t.id) st.push('<span class="pl-chip sh-chip-def">' + icon("star") + "Parish default</span>");
      if (t.image && t.image.missing) st.push(statusLabel("warning", "Degraded", "square"));
      if (t.archived) st.push('<span class="pl-chip">' + icon("archive") + "Archived</span>");
      return '<div class="sh-tcard' + (t.archived ? " is-subdued" : "") + '">' + '<button class="sh-tprev" data-a="edit" data-id="' + t.id + '" aria-label="Edit ' + esc(t.name) + '">' + themeSlide(t, SPECIMENS[0][2]) + "</button>" +
        '<div class="sh-tmeta"><div class="o-grow"><b>' + esc(t.name) + '</b><span class="o-meta">' + (t.plans ? "Used by " + t.plans + " Mass plan" + (t.plans === 1 ? "" : "s") : "Not used yet") + '</span><div class="sg-chips">' + st.join("") + "</div></div>" +
        (t.archived ? '<button class="o-btn" data-a="restoreth" data-id="' + t.id + '">Restore</button>' : '<button class="o-btn" data-a="edit" data-id="' + t.id + '">' + (t.builtIn ? "View" : "Edit") + "</button>") +
        (t.archived ? "" : '<span class="sh-rel"><button class="o-btn o-btn--quiet o-btn--icon" data-a="thmenu" data-id="' + t.id + '" aria-label="More for ' + esc(t.name) + '">' + icon("ellipsis") + "</button>" + (S.menu === t.id ? thMenu(t) : "") + "</span>") + "</div></div>";
    }
    function thMenu(t) {
      var it = t.builtIn ? [["duplicate", "copy", "Duplicate"], ["setdefault", "star", "Set as parish default"]] :
        [["rename", "pencil", "Rename"], ["duplicate", "copy", "Duplicate"], ["setdefault", "star", "Set as parish default"], ["archiveth", "archive", "Archive"]];
      return '<div class="o-menu sh-menu" role="menu">' + it.map(function (i) {
        var off = (i[0] === "setdefault" && S.dflt === t.id) || (i[0] === "archiveth" && S.dflt === t.id);
        return '<button class="o-row" role="menuitem" data-a="' + i[0] + '" data-id="' + t.id + '"' + (off ? " disabled" : "") + ">" + icon(i[1]) + i[2] + (i[0] === "archiveth" && S.dflt === t.id ? '<span class="o-meta">Choose another default first</span>' : "") + "</button>";
      }).join("") + "</div>";
    }
    function themesLib() {
      var sys = S.themes.filter(function (t) { return t.builtIn; }), par = S.themes.filter(function (t) { return !t.builtIn && (S.tarch || !t.archived); });
      var nArch = S.themes.filter(function (t) { return t.archived; }).length;
      return '<div class="sh-page"><header class="sh-head"><h1 class="o-title">Themes</h1><span class="o-grow"></span>' +
        (nArch ? '<label class="sh-check"><input type="checkbox" data-a="toggle" data-path="tarch"' + (S.tarch ? " checked" : "") + "> Show archived (" + nArch + ")</label>" : "") +
        '<button class="o-btn o-btn--primary" data-a="newtheme">' + icon("plus") + "Create parish theme</button></header>" +
        '<div class="sh-scroll"><div class="sh-col sh-col--wide"><h2 class="sh-sec">System Theme</h2><div class="sh-tgrid">' + sys.map(tcard).join("") + "</div>" +
        '<h2 class="sh-sec">Parish Themes</h2>' + (par.length ? '<div class="sh-tgrid">' + par.map(tcard).join("") + "</div>" :
        '<div class="pl-empty sh-tempty"><b>No parish themes yet</b><span class="o-meta">Masses use Ordo Black until you make your own. A new Theme starts as a copy of it, so it’s always complete.</span><button class="o-btn o-btn--primary" data-a="newtheme">' + icon("plus") + "Create parish theme</button></div>") +
        "</div></div></div>";
    }
    function tdirty() { var e = S.ed; return e && JSON.stringify(e.draft) !== JSON.stringify(theme(e.id)); }
    function editor() {
      var e = S.ed, t = e.draft, saved = theme(e.id), ro = saved.builtIn, eff = effective(t), dirty = tdirty();
      var spec = SPECIMENS.filter(function (s) { return s[0] === e.spec; })[0];
      var groups = [["typography", "Typography"], ["appearance", "Appearance"], ["layout", "Layout"]];
      var head = '<header class="sh-edhead"><button class="o-btn o-btn--quiet" data-a="tback">' + icon("arrow-left") + (e.from ? "Back to Mass Theme" : "Themes") + "</button>" +
        '<div class="o-grow sh-edname"><b class="o-title">' + esc(t.name) + "</b>" + (ro ? '<span class="pl-chip">' + icon("lock") + "Built in</span>" : "") + (saved.image && saved.image.missing ? statusLabel("warning", "Degraded", "square") : "") +
        (dirty ? statusLabel("warning", "Unsaved changes", "square") : ro ? "" : '<span class="o-meta pl-saved">' + icon("check") + "Saved</span>") + "</div>" +
        (ro ? "" : '<button class="o-btn" data-a="thist">' + icon("history") + "History</button>") +
        '<button class="o-btn" data-a="duplicate" data-id="' + t.id + '">' + icon("copy") + "Duplicate Theme</button>" +
        (ro ? "" : '<button class="o-btn o-btn--primary" data-a="tsave"' + (dirty ? "" : " disabled") + ">Save</button>") + "</header>";
      var share = !ro && saved.plans > 1 && dirty ? '<div class="o-notice sh-banner" data-state="warning">' + icon("users") + "<div><b>Shared Theme.</b> Saving updates the " + saved.plans + " Mass plans that use " + esc(saved.name) + ". To change only some, use <b>Duplicate Theme</b>.</div></div>" : "";
      var roNote = ro ? '<div class="o-notice sh-banner" data-state="info">' + icon("lock") + "<div><b>Ordo Black can't be changed.</b> It's the safety Theme every Mass can fall back on. Duplicate it to make a parish Theme you can edit.</div></div>" : "";
      var ctl;
      if (e.group === "typography") ctl = field("Text role", sel("ed.role", e.role, ["Lyrics and responses", "Titles, credits and labels"], "Text role"), "Titles and labels use the secondary SemiBold weight unless changed.") +
        field("Typeface", '<input class="pl-input" value="Atkinson Hyperlegible Next" readonly>', "Bundled with Ordo; Noto Sans fills missing characters.") +
        '<div class="sh-two">' + field("Preferred size", '<span class="sh-unit">' + input("ed.draft.size", t.size, { ro: ro }) + "px</span>") + field("Minimum size", '<span class="sh-unit">' + input("ed.draft.min", t.min, { ro: ro }) + "px</span>") + "</div>" +
        field("Weight", ro ? '<input class="pl-input" value="ExtraBold" readonly>' : seg("ed.draft.weight", t.weight, [600, 700, 800], "Weight"), "600 SemiBold · 700 Bold · 800 ExtraBold") +
        field("Line spacing", ro ? '<input class="pl-input" value="' + t.lh + '" readonly>' : seg("ed.draft.lh", t.lh, [1.15, 1.25, 1.4], "Line spacing"));
      else if (e.group === "appearance") {
        var img = t.image, ar = img ? img.w / img.h : 0, notWide = img && Math.abs(ar - 16 / 9) / (16 / 9) > 0.001, soft = img && (img.w < 1920 || img.h < 1080);
        ctl = '<div class="sh-two">' + field("Text colour", colour("ed.draft.text", t.text, ro)) + field(t.mode === "image" ? "Fallback colour" : "Background colour", colour("ed.draft.bg", t.bg, ro), t.mode === "image" ? "Shown if the image is missing." : "") + "</div>" +
          field("Background", ro ? '<input class="pl-input" value="Solid colour" readonly>' : seg("ed.draft.mode", t.mode === "image" ? "Image" : "Solid colour", ["Solid colour", "Image"], "Background"));
        if (t.mode === "image") {
          if (!img || img.missing) ctl += '<div class="sh-imgbox is-missing">' + icon("image-off") + "<div><b>" + (img ? esc(img.file) + " is missing" : "No image chosen") + "</b><br><span class=\"o-meta\">" + (img ? "Slides use the fallback colour until it's replaced. Celebration Packages that use this Theme can't be exported yet." : "Choose an image; 16:9 at 1920 × 1080 or larger works best.") + "</span></div></div>" +
            '<div class="sh-inline"><button class="o-btn" data-a="replaceimg">' + icon("image-up") + (img ? "Replace image…" : "Choose image…") + "</button>" + (img ? '<button class="o-btn" data-a="removeimg">Remove image</button>' : "") + "</div>";
          else ctl += '<div class="sh-imgbox"><span class="sh-thumb" style="background:' + PHOTO + '"></span><div><b>' + esc(img.file) + '</b><br><span class="o-meta">' + img.w + " × " + img.h + "</span></div>" + (ro ? "" : '<button class="o-btn" data-a="replaceimg">Replace…</button>') + "</div>" +
            (notWide ? '<div class="sh-adv">' + icon("crop") + "Not 16:9 · Parts of this image will be cropped</div>" : "") + (soft ? '<div class="sh-adv">' + icon("info") + "May appear soft on higher-resolution displays</div>" : "") +
            field("Focal point", seg("ed.draft.focal", t.focal, ["Top", "Center", "Bottom"], "Focal point"), "Which part survives the Cover crop.") +
            '<div class="sh-two">' + field("Overlay colour", colour("ed.draft.overlay", t.overlay, ro)) + field("Overlay opacity", '<span class="sh-unit"><input type="range" min="0" max="90" step="5" data-in="ed.draft.opacity" value="' + t.opacity + '"' + (ro ? " disabled" : "") + "> " + t.opacity + "%</span>") + "</div>";
        }
        var r = eff.ratio, okc = r >= 7, mid = r >= 4.5;
        ctl += '<div class="sh-contrast" data-state="' + (okc ? "ok" : mid ? "warn" : "bad") + '">' + (okc ? statusLabel("live", "Contrast " + r.toFixed(1) + " : 1 · Good from the back of the nave") : mid ? statusLabel("warning", "Contrast " + r.toFixed(1) + " : 1 · May wash out on a bright projector", "square") : statusLabel("danger", "Contrast " + r.toFixed(1) + " : 1 · Too low to read reliably", "diamond")) + "</div>";
      } else ctl = field("Lyrics and responses", ro ? '<input class="pl-input" value="Centered" readonly>' : seg("ed.draft.align", t.align, ["Centered", "Left"], "Alignment")) +
        field("Vertical placement", ro ? '<input class="pl-input" value="Middle" readonly>' : seg("ed.draft.vpos", t.vpos, ["Middle", "Upper"], "Vertical placement")) +
        '<p class="o-meta">The safe area (120 × 80 px) is fixed. An Arrangement may ask for centred lyrics; it never sets colours (Decision 20).</p>';
      var specBar = '<div class="o-segmented sh-specs" role="group" aria-label="Specimen">' + SPECIMENS.map(function (s) { return '<button data-a="set" data-path="ed.spec" data-v="' + s[0] + '" aria-pressed="' + (e.spec === s[0]) + '">' + s[1] + "</button>"; }).join("") +
        (e.from ? '<button data-a="set" data-path="ed.spec" data-v="current" aria-pressed="' + (e.spec === "current") + '">Current Mass slide</button>' : "") + "</div>";
      var cue = e.spec === "current" ? { k: "lyrics", h: "Koro", b: "Daygon, daygon ta\nang ngalan sa Ginoo" } : spec[2];
      return '<div class="sh-page">' + head + share + roNote + '<div class="sh-edpanes"><div class="o-pane sh-ctl"><div class="o-tabs sh-gtabs" role="tablist">' +
        groups.map(function (g) { return '<button role="tab" data-a="set" data-path="ed.group" data-v="' + g[0] + '" aria-selected="' + (e.group === g[0]) + '">' + g[1] + "</button>"; }).join("") + "</div>" +
        '<div class="o-pane-body sh-form">' + ctl + "</div></div>" +
        '<div class="sh-stage">' + specBar + '<div class="sh-big">' + themeSlide(t, cue) + '</div><div class="o-meta">Canonical 1920 × 1080 · the same frame Presenter projects. The specimen is test content only.</div></div></div></div>';
    }
    function colour(path, v, ro) {
      return '<span class="sh-colour"><span class="sh-chipc" style="background:' + (hexOk(v) ? v : "transparent") + '"></span>' + input(path, v, { ro: ro, cls: hexOk(v) ? "" : "is-bad" }) + "</span>";
    }
    function deltas(before, after) {
      // Exception-based review (Decision 99): only new problems are listed.
      var out = [];
      if (after.size >= 104 && before.size < 104) out.push(["Mass plans gain overflow", [["Sun 1 Nov · 09:00 · English", "Gloria · verse 2 now needs 3 slides (was 2)"], ["Mon 2 Nov · 06:00 · Cebuano", "Entrance Song · Koro won't fit at 104 px; uses minimum " + after.min + " px"]]]);
      if (effective(after).ratio < 4.5 && effective(before).ratio >= 4.5) out.push(["New contrast warnings", [["Every plan using " + after.name, "Text is hard to read on this background"]]]);
      return out;
    }
    function reviewOver() {
      var r = S.review;
      return '<div class="o-scrim" data-a="close"></div><div class="o-slideover sh-over" role="dialog" aria-modal="true" aria-labelledby="sh-rv">' + closeTab("Close review") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-title" id="sh-rv">Review affected plans</h2><div class="o-meta">' + esc(r.name) + " is saved. Only plans with new problems are listed; the rest re-rendered cleanly.</div></div></div>" +
        '<div class="o-overlay-body">' + r.list.map(function (g) { return '<div class="pl-fgroup"><div class="o-label">' + esc(g[0]) + "</div>" + g[1].map(function (f) { return '<div class="pl-find">' + statusLabel("warning", "", "square") + '<div class="o-grow"><span class="pl-find-t">' + esc(f[0]) + '</span><span class="o-meta">' + esc(f[1]) + '</span><button class="pl-link" data-a="toast" data-msg="Opens the plan at that item and slide">Open at this slide</button></div></div>'; }).join("") + "</div>"; }).join("") + "</div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-a="undo">Undo the save</button><button class="o-btn o-btn--primary" data-a="close">Done</button></div></div>';
    }
    function histOver() {
      var t = theme(S.ed.id), h = S.hist, rev = t.revs[h.i];
      return '<div class="o-scrim" data-a="close"></div><div class="o-slideover sh-over" role="dialog" aria-modal="true" aria-labelledby="sh-th">' + closeTab("Close History") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-title" id="sh-th">History · ' + esc(t.name) + '</h2><div class="o-meta">Each Save is a revision. Previewing doesn\'t change the editor.</div></div></div>' +
        '<div class="o-overlay-body"><div class="sh-list">' + t.revs.map(function (r, i) { return '<button class="o-row sh-rev' + (i === h.i ? " is-sel" : "") + '" data-a="rev" data-i="' + i + '" aria-pressed="' + (i === h.i) + '"><div class="o-grow"><b>' + esc(r.when) + (i === 0 ? " · current" : "") + '</b><span class="o-meta">' + esc(r.what) + " · " + r.plans + " plans updated</span></div></button>"; }).join("") + "</div>" +
        '<div class="sh-revprev">' + themeSlide(i0(t, h.i), SPECIMENS[0][2]) + "</div></div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-a="close">Close</button><button class="o-btn o-btn--primary" data-a="restorerev"' + (h.i === 0 ? " disabled" : "") + ">Restore this revision</button></div></div>";
      function i0(t, i) { var x = copy(t); if (i === 1) x.size = 88; if (i === 2) { x.size = 88; x.bg = "#101216"; } if (i === 3) { x.bg = "#000000"; x.text = "#FFFFFF"; } return x; }
    }

    // ── Transfer & Backup (Decisions 17, 180–201, 231) ───────────────────────────────────────
    function xfer() {
      if (S.xview === "import") return importView();
      if (S.xview === "restore") return restoreView();
      var last = S.backups.filter(function (b) { return b.kind === "portable"; })[0];
      var strip = remindOn() ? '<div class="o-notice sh-banner" data-state="warning">' + icon("triangle-alert") + "<div><b>No portable backup in " + S.backup.days + " days.</b> " + S.backup.changes + " changes exist only on this computer.</div><button class=\"o-btn o-btn--warning\" data-a=\"mkbackup\">Create parish backup</button></div>" : "";
      var protect = '<section class="sh-card"><h2 class="sh-sec">Protect this parish</h2><dl class="sg-meta sh-meta3"><div><dt>Last portable backup</dt><dd>' + esc(last.when) + "</dd><dd class=\"o-meta\">" + esc(last.where) + "</dd></div>" +
        "<div><dt>Changes since</dt><dd>" + S.backup.changes + '</dd><dd class="o-meta">' + (S.backup.changes ? "Not yet protected off this computer" : "Everything is in the last backup") + "</dd></div>" +
        '<div><dt>Automatic safety backups</dt><dd>2 on this computer</dd><dd class="o-meta">Made before updates and restores</dd></div></dl>' +
        '<div class="sh-inline"><button class="o-btn o-btn--primary" data-a="mkbackup">' + icon("hard-drive-download") + 'Create parish backup</button><button class="o-btn" data-a="restore">' + icon("archive-restore") + "Restore parish backup…</button></div></section>";
      var pk = '<section class="sh-card"><h2 class="sh-sec">Celebration Packages</h2><p class="o-meta">A Celebration Package carries one Celebration and all its Mass plans to another computer, ready to present offline.</p>' +
        '<div class="sh-inline"><button class="o-btn" data-a="exportpick">' + icon("package") + 'Export celebration…</button><button class="o-btn" data-a="import">' + icon("file-down") + "Import .ordo…</button></div>" +
        '<div class="o-label sh-sub">Package history</div><div class="sh-list">' + S.packages.map(function (p) {
          var cur = p.versions.filter(function (v) { return v.current; })[0], open = S.menu === "pk:" + p.id;
          return '<div class="sh-pk"><div class="o-row sh-trow">' + icon("package") + '<div class="o-grow"><b>' + esc(p.title) + '</b><span class="o-meta">' + esc(p.date) + " · Current version " + cur.v + " · " + (cur.exported ? "exported " : "imported ") + esc(cur.when) + " · " + esc(cur.from) + "</span></div>" +
            statusLabel("live", "Verified") + '<button class="o-btn o-btn--quiet" data-a="pkopen" data-id="' + p.id + '" aria-expanded="' + open + '">' + p.versions.length + " versions " + icon(open ? "chevron-up" : "chevron-down") + "</button></div>" +
            (open ? '<div class="sh-vers">' + p.versions.map(function (v) { return '<div class="o-row"><span class="sh-time">Version ' + v.v + (v.current ? " · Current" : "") + '</span><span class="o-meta o-grow">' + esc(v.when) + " · " + esc(v.from) + " · " + v.plans + " Mass plan" + (v.plans > 1 ? "s" : "") + " · " + v.fp + "</span>" + (v.current ? "" : '<button class="o-btn o-btn--quiet" data-a="toast" data-msg="Opens version ' + v.v + ' read-only, with Restore this version">Inspect</button>') + "</div>"; }).join("") + "</div>" : "") + "</div>";
        }).join("") + "</div></section>";
      var bk = '<section class="sh-card"><h2 class="sh-sec">Parish Backups</h2><div class="sh-list">' + S.backups.map(function (b) {
        var st = b.state === "Verified" ? statusLabel("live", "Verified") : b.state === "Damaged" ? statusLabel("danger", "Damaged", "diamond") : statusLabel("unavailable", "Unavailable", "ring");
        var acts = b.state === "Damaged" ? '<button class="o-btn" data-a="toast" data-msg="Opens a file chooser; the copy must match this backup exactly">Locate another copy…</button><button class="o-btn" data-a="mkbackup">Create new backup</button>'
          : b.state === "Unavailable" ? '<button class="o-btn o-btn--quiet" data-a="toast" data-msg="Connect the drive and Ordo checks it again">Check again</button>'
          : b.kind === "safety" ? '<button class="o-btn o-btn--quiet" data-a="toast" data-msg="Asks to confirm: deleting removes this recovery copy">Delete…</button>' : '<button class="o-btn o-btn--quiet" data-a="toast" data-msg="Opens the folder">Show in folder</button>';
        return '<div class="o-row sh-trow">' + icon(b.kind === "safety" ? "shield" : "hard-drive") + '<div class="o-grow"><b>' + esc(b.kind === "safety" ? "Safety backup · " + b.file : b.file) + '</b><span class="o-meta">' + esc(b.when) + " · " + esc(b.where) + " · " + b.size +
          (b.state === "Unavailable" ? " · The drive isn't connected; nothing is known to be wrong" : "") + (b.state === "Damaged" ? " · Checksum failed for 2 files. It can't be restored or count as a backup." : "") + "</span></div>" + st + acts + "</div>";
      }).join("") + "</div></section>";
      return '<div class="sh-page"><header class="sh-head"><h1 class="o-title">Transfer &amp; Backup</h1><span class="o-grow"></span></header>' + strip + '<div class="sh-scroll"><div class="sh-col">' + protect + pk + bk + "</div></div></div>";
    }
    function expOver() {
      var x = S.exp, c = x.cel ? cel(x.cel) : null;
      var head = '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-title" id="sh-ex">' + (x.done ? "Export complete" : "Export Celebration Package") + "</h2>" + (c ? '<div class="o-meta">' + esc(c.title) + " · " + esc(c.date) + "</div>" : "") + "</div></div>";
      var body, foot;
      if (!c) {
        body = '<p class="o-meta">Choose the Celebration to export. The package is always the whole Celebration with every Mass plan under it.</p><div class="sh-list">' +
          S.cels.filter(function (c) { return !c.archived; }).map(function (c) { return '<button class="o-row sh-trow" data-a="expcel" data-id="' + c.id + '"><span class="sh-swatch" style="background:' + c.hex + '"></span><div class="o-grow"><b>' + esc(c.title) + '</b><span class="o-meta">' + esc(c.date) + " · " + c.masses.length + " Mass plan" + (c.masses.length === 1 ? "" : "s") + "</span></div>" + icon("chevron-right") + "</button>"; }).join("") + "</div>";
        foot = '<button class="o-btn" data-a="close">Cancel</button>';
      } else if (x.done) {
        var fn = c.date.replace(/^\w+ /, "").replace(/ /g, "-") + "_" + c.title.replace(/[^A-Za-z0-9]+/g, "-").replace(/-$/, "") + ".ordo";
        body = '<div class="sh-done">' + icon("circle-check", "o-icon-20") + "<div><b>Verified</b> · read back from the drive and checked.</div></div><dl class=\"pl-meta\"><dt>File</dt><dd>" + esc(fn) + "</dd><dt>Saved to</dt><dd>USB drive (E:)</dd><dt>Size</dt><dd>" + (40 + c.masses.length * 18) + " MB</dd>" +
          "<dt>Package version</dt><dd>" + (x.same ? "Version 3 · 7c1e·94b0 · existing version copied" : "New version 4 · b81d·3e07") + "</dd><dt>Mass plans</dt><dd>" + c.masses.length + "</dd></dl>" +
          '<div class="o-notice" data-state="info">' + icon("usb") + "<div>Ordo has closed the file. Use <b>Eject</b> in Windows or Linux before you remove the drive.</div></div>";
        foot = '<button class="o-btn" data-a="toast" data-msg="Opens the folder">Show in folder</button><button class="o-btn" data-a="toast" data-msg="Copies the same verified file to another place; no new version">Copy to another location…</button><button class="o-btn o-btn--primary" data-a="close">Done</button>';
      } else {
        var ms = c.masses, blocked = !ms.length, warn = ms.some(function (y) { return y.inc || y.warn; });
        var sum = blocked ? statusLabel("danger", "Cannot export", "diamond") : warn ? statusLabel("warning", "Ready with warnings", "square") : statusLabel("live", "Ready");
        body = '<div class="sh-expsum">' + sum + '<span class="o-meta">' + (blocked ? "This Celebration has no Mass plans, so there's nothing to present elsewhere." : x.same ? "No changes since Package version 3 · the existing version will be copied" : "New version · 7 changes since the version received from Home PC") + "</span></div>" +
          (blocked ? '<button class="o-btn" data-a="addmass" data-id="' + c.id + '">' + icon("plus") + "Add a Mass plan</button>" :
          '<div class="o-label sh-sub">Included Mass plans</div><div class="sh-list">' + ms.map(function (y) { return '<div class="o-row sh-trow"><span class="sh-time">' + esc(y.time) + '</span><span class="o-grow">' + esc(y.lang) + " · " + esc(y.choir || "No choir") + "</span>" + readiness(y) + "</div>"; }).join("") + "</div>" +
          (warn ? '<p class="o-meta">Unfinished plans can travel; preparation continues on the other computer. Only problems that would make the file untrustworthy stop an export.</p>' : "") +
          '<dl class="pl-meta"><dt>Estimated size</dt><dd>' + (40 + ms.length * 18) + " MB</dd><dt>Destination</dt><dd>Chosen when you export</dd></dl>" +
          '<details class="sh-disc"><summary>Package contents</summary><ul class="pl-changes"><li>' + ms.length + " Presentation snapshots, one per Mass plan</li><li>14 Arrangements · 9 Responses &amp; Prayers · 2 General Presentations</li><li>Theme Parish Dark and its fonts</li><li>3 images, stored once each</li></ul></details>");
        foot = '<button class="o-btn" data-a="close">Cancel</button><button class="o-btn o-btn--primary" data-a="doexport"' + (blocked ? " disabled" : "") + ">" + icon("package") + "Export package…</button>";
      }
      return '<div class="o-scrim" data-a="close"></div><div class="o-slideover sh-over" role="dialog" aria-modal="true" aria-labelledby="sh-ex">' + closeTab("Close") + head + '<div class="o-overlay-body">' + body + '</div><div class="o-overlay-foot">' + foot + "</div></div>";
    }
    function mkOver() {
      var x = S.mk;
      var body = x.done ? '<div class="sh-done">' + icon("circle-check", "o-icon-20") + "<div><b>Verified</b> · read back from the drive and checked.</div></div><dl class=\"pl-meta\"><dt>File</dt><dd>Parokya-ni-San-Roque_Backup_2026-10-28_1402.ordo</dd><dt>Saved to</dt><dd>USB drive (E:)</dd><dt>Size</dt><dd>418 MB</dd></dl>" +
        '<div class="o-notice" data-state="info">' + icon("usb") + "<div>Ordo has closed the file. Use <b>Eject</b> in Windows or Linux before you remove the drive.</div></div>"
        : '<dl class="pl-meta"><dt>Parish</dt><dd>' + esc(S.parish.name) + "</dd><dt>Contents</dt><dd>38 Celebrations · 61 Mass plans · 6 templates · 212 Songs · 287 Arrangements · 4 Choirs · 6 Themes · 1,284 assets</dd><dt>History</dt><dd>Every revision, Presentation snapshot and Package version</dd>" +
          "<dt>Estimated size</dt><dd>418 MB</dd><dt>Integrity check</dt><dd>" + statusLabel("live", "Passed") + "</dd><dt>Destination</dt><dd>Chosen when you create it</dd></dl>" +
          '<p class="o-meta">A Parish Backup is always the whole parish. It doesn\'t include Ordo itself, caches, or the logs used by Diagnostics.</p><details class="sh-disc"><summary>Backup contents</summary><ul class="pl-changes"><li>Parish details and settings</li><li>Display calibration, kept for this computer\'s displays only</li><li>Package history, identity mappings and sync anchors</li></ul></details>';
      var foot = x.done ? '<button class="o-btn" data-a="toast" data-msg="Opens the folder">Show in folder</button><button class="o-btn" data-a="toast" data-msg="Copies the same verified file elsewhere">Copy to another location…</button><button class="o-btn o-btn--primary" data-a="close">Done</button>'
        : '<button class="o-btn" data-a="close">Cancel</button><button class="o-btn o-btn--primary" data-a="domk">' + icon("hard-drive-download") + "Create backup…</button>";
      return '<div class="o-scrim" data-a="close"></div><div class="o-slideover sh-over" role="dialog" aria-modal="true" aria-labelledby="sh-mk">' + closeTab("Close") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-title" id="sh-mk">' + (x.done ? "Backup complete" : "Create parish backup") + '</h2><div class="o-meta">Everything Ordo knows about ' + esc(S.parish.name) + ", in one .ordo file.</div></div></div>" +
        '<div class="o-overlay-body">' + body + '</div><div class="o-overlay-foot">' + foot + "</div></div>";
    }
    var STAGED = [
      ["Celebration", "Ika-33 nga Domingo sa Ordinaryong Panahon", "New"], ["Mass plan", "Sun 15 Nov · 06:00 · Cebuano", "New"], ["Mass plan", "Sun 15 Nov · 09:00 · Cebuano", "New"],
      ["Arrangement", "Daygon Ta · Koro San Roque", "Safe update"], ["Arrangement", "Ihalad Namo · Koro San Roque", "Safe update"], ["Song", "Amahan Namo (Walter)", "Needs decision"],
      ["Theme", "Parish Dark", "Unchanged"], ["Response", "Greeting · Cebuano", "Unchanged"]
    ];
    function importView() {
      var x = S.imp, rows = STAGED.map(function (r, i) {
        var st = r[2] === "Needs decision" && x.decided ? "Keep both" : r[2];
        var lab = st === "New" ? statusLabel("info", "New") : st === "Safe update" ? statusLabel("live", "Safe update") : st === "Unchanged" ? '<span class="o-meta">Unchanged</span>' : st === "Keep both" ? statusLabel("live", "Decided · keep both") : statusLabel("warning", "Needs decision", "square");
        return '<div class="o-row sh-trow"><span class="sh-kind o-meta">' + r[0] + '</span><span class="o-grow">' + esc(r[1]) + "</span>" + lab + (r[2] === "Needs decision" ? '<button class="o-btn" data-a="decide">' + (x.decided ? "Change" : "Decide…") + "</button>" : "") + "</div>";
      }).join("");
      return '<div class="sh-page"><header class="sh-edhead"><button class="o-btn o-btn--quiet" data-a="cancelimp">' + icon("x") + 'Cancel import</button><div class="o-grow sh-edname"><b class="o-title">Import Celebration Package</b><span class="o-meta">2026-11-15_Ika-33-nga-Domingo.ordo · copied from USB drive (E:)</span></div>' +
        '<button class="o-btn o-btn--primary" data-a="doimport"' + (x.decided ? "" : " disabled") + ">Import celebration</button></header>" +
        '<div class="sh-scroll"><div class="sh-col sh-col--wide"><div class="sh-two sh-layers"><section class="sh-card"><h2 class="sh-sec">' + icon("monitor-play") + 'Exact presentation</h2><p>' + statusLabel("live", "Verified · 14 checks passed") + '</p><p class="o-meta">2 Mass plan snapshots install exactly as prepared on Home PC, whatever you decide below.</p></section>' +
        '<section class="sh-card"><h2 class="sh-sec">' + icon("library") + "Library reconciliation</h2><p>3 new · 2 safe updates · 2 unchanged · " + (x.decided ? "0" : "1") + ' needs decision</p><p class="o-meta">Editable records for this parish\'s Library. Nothing changes until you import.</p></section></div>' +
        (x.decided ? "" : '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div><b>1 decision before import.</b> An incoming Song may be the same work as one already here.</div></div>") +
        '<div class="o-label sh-sub">Staged changes</div><div class="sh-list">' + rows + "</div></div></div></div>";
    }
    function decideOver() {
      return '<div class="o-scrim" data-a="close"></div><div class="o-slideover sh-over" role="dialog" aria-modal="true" aria-labelledby="sh-dc">' + closeTab("Close") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-title" id="sh-dc">Amahan Namo (Walter)</h2><div class="o-meta">Possible match: <b>Amahan Namo</b> · Walter · already in this Library</div></div></div>' +
        '<div class="o-overlay-body sh-radios">' + [["Keep both as separate Songs", "The incoming Song gets its own identity; Ordo remembers this choice for later imports."], ["Add as an Arrangement of Amahan Namo", "One Song work with both Arrangements."], ["Same Song: review the differences", "Compare content side by side before choosing."]]
          .map(function (o, i) { return '<label class="sh-radio"><input type="radio" name="dc"' + (i === 0 ? " checked" : "") + "><span><b>" + o[0] + '</b><br><span class="o-meta">' + o[1] + "</span></span></label>"; }).join("") +
        '<button class="pl-link" data-a="toast" data-msg="Opens the read-only comparison (Song import review)">Compare content</button></div>' +
        '<div class="o-overlay-foot"><button class="o-btn" data-a="close">Cancel</button><button class="o-btn o-btn--primary" data-a="decided">Use this decision</button></div></div>';
    }
    function restoreView() {
      var r = S.rst, diff = true;
      var col = function (t, rows) { return '<section class="sh-card"><h2 class="sh-sec">' + t + '</h2><dl class="pl-meta">' + rows.map(function (x) { return "<dt>" + x[0] + "</dt><dd>" + x[1] + "</dd>"; }).join("") + "</dl></section>"; };
      var gate = r.protected ? '<div class="o-notice" data-state="info">' + icon("shield-check") + "<div><b>Current parish protected.</b> Safety backup Wed 28 Oct 2026, 14:06 · Ordo data · verified.</div></div>"
        : '<div class="o-notice" data-state="warning">' + icon("shield") + "<div><b>Protect the current parish first.</b> 46 changes aren't in any backup that's connected now. Ordo makes and verifies a safety backup before it replaces anything.</div><button class=\"o-btn\" data-a=\"protect\">Make safety backup</button></div>";
      return '<div class="sh-page"><header class="sh-edhead"><button class="o-btn o-btn--quiet" data-a="cancelrst">' + icon("x") + 'Cancel</button><div class="o-grow sh-edname"><b class="o-title">Restore parish backup</b><span class="o-meta">Parokya-ni-San-Roque_Backup_2026-09-27_2010.ordo · ' + statusLabel("live", "Verified") + "</span></div>" +
        '<button class="o-btn o-btn--danger" data-a="rstconfirm"' + (r.protected ? "" : " disabled") + ">Restore parish…</button></header>" +
        '<div class="sh-scroll"><div class="sh-col sh-col--wide"><div class="o-notice" data-state="danger">' + icon("triangle-alert") + "<div><b>Restoring replaces the whole current parish.</b> Nothing is merged. Changes made after the backup was created are only kept in the safety backup.</div></div>" +
        '<div class="sh-two">' + col("Current parish", [["Parish", esc(S.parish.name)], ["Size", "418 MB"], ["Content", "38 Celebrations · 61 Mass plans · 212 Songs · 6 Themes"], ["Latest activity", "Wed 28 Oct 2026, 13:58"], ["Installation", "This computer (Church PC)"]]) +
        col("Backup", [["Parish", esc(S.parish.name) + " · same parish"], ["Size", "398 MB"], ["Content", "33 Celebrations · 52 Mass plans · 204 Songs · 5 Themes"], ["Created", "Sun 27 Sep 2026, 20:10"], ["Made on", "Home PC"]]) + "</div>" + (diff ? gate : "") + "</div></div></div>";
    }
    function rstDialog() {
      return '<div class="o-scrim"></div><div class="o-dialog" role="alertdialog" aria-modal="true" aria-labelledby="sh-rd"><div class="o-overlay-head"><h2 class="o-title" id="sh-rd">Replace the current parish?</h2></div>' +
        '<div class="o-overlay-body"><p>' + esc(S.parish.name) + " on this computer becomes the backup from Sun 27 Sep 2026. Ordo restarts into it. The safety backup from 14:06 today keeps what's here now.</p></div>" +
        '<div class="o-overlay-foot"><button class="o-btn o-btn--danger" data-a="dorestore">Restore parish</button><button class="o-btn o-btn--primary" data-a="closedlg" data-focus-key="keepcur">Keep current parish</button></div></div>';
    }

    // ── Diagnostics (Decisions 204–207, 226) ─────────────────────────────────────────────────
    function diag() {
      var d = S.diag, findings = d.healthy ? [] : DIAG.reduce(function (a, g) { return a.concat(g.issues); }, []);
      var scanning = d.state === "scan";
      var head = '<header class="sh-head"><h1 class="o-title">Diagnostics</h1><span class="o-meta">Last full check: ' + esc(d.when) + '</span><span class="o-grow"></span>' +
        (scanning ? '<button class="o-btn" data-a="dcancel">Cancel</button>' : '<button class="o-btn o-btn--primary" data-a="dscan">' + icon("activity") + "Run full diagnostics</button>") + "</header>" +
        (scanning ? '<div class="sh-progress" role="progressbar" aria-valuenow="' + d.pct + '" aria-valuemin="0" aria-valuemax="100"><span style="width:' + d.pct + '%"></span></div>' : "");
      var strip = '<div class="sh-health" data-state="' + (findings.length ? "warn" : "ok") + '">' + icon(findings.length ? "triangle-alert" : "circle-check", "o-icon-20") +
        "<div><b>" + (findings.length ? "Needs attention" : "Healthy") + "</b><br>" + (findings.length ? findings.length + " things need attention. Presenting still works; each item says what it affects." : "Everything Ordo checked is working. Nothing was sent anywhere.") + "</div></div>";
      var note = d.cancelled ? '<p class="o-meta">' + icon("circle-slash") + " A full check started at " + esc(d.cancelled) + " was cancelled. The result above is still the last completed one.</p>" : "";
      var issues = findings.length ? '<h2 class="sh-sec">Issues requiring attention</h2><div class="sh-list">' + findings.map(function (f) { return '<div class="sh-issue">' + statusLabel("warning", "", "square") + '<div class="o-grow"><b>' + esc(f.t) + '</b><span class="o-meta">' + esc(f.why) + '</span></div><button class="o-btn" data-a="go" data-dest="' + f.go + '"' + (f.page ? ' data-page="' + f.page + '"' : "") + ">" + esc(f.link) + "</button></div>"; }).join("") + "</div>" : "";
      var rows = DIAG.map(function (g, i) {
        var n = d.healthy ? 0 : g.issues.length, open = !!d.open[g.id], prog = scanning ? (i < d.cur ? statusLabel("live", "Checked") : i === d.cur ? statusLabel("info", "Checking…", "ring") : '<span class="o-meta">Waiting</span>') : n ? statusLabel("warning", n + " finding" + (n > 1 ? "s" : ""), "square") : statusLabel("live", "OK");
        return '<div class="sh-drow"><button class="o-row sh-dhead" data-a="dopen" data-id="' + g.id + '" aria-expanded="' + open + '">' + icon(open ? "chevron-down" : "chevron-right") + '<b class="o-grow">' + g.name + "</b>" + prog + "</button>" +
          (open ? '<dl class="pl-meta sh-dfacts">' + g.facts.map(function (f) { return "<dt>" + f[0] + "</dt><dd>" + esc(f[1]) + "</dd>"; }).join("") + "</dl>" : "") + "</div>";
      }).join("");
      return '<div class="sh-page">' + head + '<div class="sh-scroll"><div class="sh-col">' + strip + note + issues + '<h2 class="sh-sec">Details</h2><div class="sh-drows">' + rows + "</div>" +
        '<div class="sh-inline sh-trouble"><button class="o-btn" data-a="toast" data-msg="Summary copied: no parish names, titles, lyrics or full paths">' + icon("clipboard-copy") + 'Copy summary</button><button class="o-btn" data-a="toast" data-msg="Saves a diagnostic-report .ordo where you choose. Ordo never sends it.">' + icon("file-output") + "Export diagnostic report…</button>" +
        '<span class="o-meta">For whoever helps you with Ordo. Neither includes lyrics, readings, notes or the parish name.</span></div></div></div></div>';
    }
    var scanTimer = null;
    function scanStep() {
      var d = S.diag; d.pct = Math.min(100, d.pct + 8); d.cur = Math.floor(d.pct / (100 / DIAG.length));
      if (d.pct >= 100) { clearInterval(scanTimer); scanTimer = null; d.state = "done"; d.when = "Wed 28 Oct 2026, 14:10"; d.cancelled = null; done("Full diagnostics finished · " + (d.healthy ? "Healthy" : "Needs attention")); }
      render();
    }

    // ── Help (Decision 232; Decision 281) ─────────────────────────────────────────────
    function help() {
      var topics = [["keys", "keyboard", "Keyboard shortcuts"]].concat(GUIDES.map(function (g) { return [g.id, "book-open", g.t]; })).concat([["about", "info", "About Ordo"]]);
      var cur = S.help, body;
      if (cur === "keys") body = '<h2 class="o-title">Keyboard shortcuts</h2><p class="o-meta">Press <kbd class="o-kbd">Ctrl</kbd>+<kbd class="o-kbd">/</kbd> anywhere in Ordo to open this list beside what you\'re doing.</p>' + keyGroups("Masses") + '<button class="o-btn" data-a="keys">' + icon("panel-right") + "Open as a panel</button>";
      else if (cur === "about") body = '<h2 class="o-title">About Ordo</h2><dl class="pl-meta"><dt>Version</dt><dd>0.9.2 (build 2026.10.18)</dd><dt>Parish</dt><dd>' + esc(S.parish.name) + "</dd><dt>Data</dt><dd>Stored on this computer only. Ordo works without internet and sends nothing.</dd><dt>Fonts</dt><dd>Atkinson Hyperlegible Next and Noto Sans, SIL Open Font License 1.1</dd></dl>" +
        '<div class="sh-inline"><button class="o-btn" data-a="toast" data-msg="Shows the open-source licence texts bundled with Ordo">Open-source licences</button><button class="o-btn" data-a="go" data-dest="diagnostics">' + icon("activity") + "Diagnostics</button></div>";
      else { var g = GUIDES.filter(function (x) { return x.id === cur; })[0]; body = '<h2 class="o-title">' + esc(g.t) + '</h2><ol class="sh-steps">' + g.b.map(function (s) { return "<li>" + esc(s) + "</li>"; }).join("") + '</ol><p class="o-meta">Guides are part of Ordo and work offline.</p>'; }
      return '<div class="sh-page"><header class="sh-head"><h1 class="o-title">Help</h1></header><div class="sg-panes"><div class="o-pane sh-nav"><div class="o-pane-body">' +
        topics.map(function (t) { return '<button class="o-row sh-navrow" data-a="set" data-path="help" data-v="' + t[0] + '" aria-current="' + (cur === t[0]) + '">' + icon(t[1]) + t[2] + "</button>"; }).join("") +
        '</div></div><div class="o-pane sg-detail"><div class="o-pane-body sh-doc">' + body + "</div></div></div></div>";
    }
    function keyGroups(ctx) {
      var order = [ctx, "Global", "Editing", "Presenter"];
      return order.map(function (g) { return '<div class="sh-kgroup"><div class="o-label">' + (g === ctx ? g + " · this workspace" : g) + "</div>" + SHORTCUTS[g].map(function (k) { return '<div class="sh-krow"><span>' + esc(k[1]) + "</span><span>" + kbd(k[0]) + "</span></div>"; }).join("") + "</div>"; }).join("");
    }
    function keysPanel() {
      var ctx = S.dest === "themes" ? "Themes" : "Masses";
      return '<div class="o-slideover sh-keys" role="dialog" aria-labelledby="sh-ks"><div class="o-overlay-head"><div class="o-grow"><h2 class="o-title" id="sh-ks">Keyboard shortcuts</h2><div class="o-meta">' + kbd("Ctrl+/") + " or Esc to close</div></div>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-a="keys" aria-label="Close">' + icon("x") + '</button></div><div class="o-overlay-body">' + keyGroups(ctx) + "</div></div>";
    }

    // ── Settings (Decisions 46, 47, 50, 55, 89, 202, 234; Decision 280) ─────────────
    function settings() {
      var pages = [["parish", "church", "Parish"], ["displays", "monitor", "Displays"], ["interface", "type", "Interface"]], body;
      if (S.spage === "parish") {
        var p = S.parish;
        body = '<h2 class="o-title">Parish</h2>' + field("Parish name", input("parish.name", p.name), "Shown on Parish Backups and in About Ordo.") +
          '<h3 class="sh-sec">Defaults for new Mass plans</h3><p class="o-meta">A new Mass plan copies these once. Changing them never changes plans already made (Decision 89).</p>' +
          '<div class="sh-field"><span class="o-label">Regular presiders</span><div class="sh-list">' + p.presiders.map(function (x, i) { return '<div class="o-row sh-trow">' + icon("user") + '<span class="o-grow">' + esc(x) + "</span>" + (p.presider === x ? '<span class="pl-chip">' + icon("star") + "Default</span>" : '<button class="o-btn o-btn--quiet" data-a="defpres" data-i="' + i + '">Make default</button>') + '<button class="o-btn o-btn--quiet o-btn--icon" data-a="rmpres" data-i="' + i + '" aria-label="Remove ' + esc(x) + '">' + icon("x") + "</button></div>"; }).join("") + "</div>" +
          '<div class="sh-inline">' + input("pdraft", S.pdraft || "", { ph: "Add a regular presider" }) + '<button class="o-btn" data-a="addrpres"' + ((S.pdraft || "").trim() ? "" : " disabled") + ">Add</button></div><span class=\"o-meta\">Visiting priests are typed straight into a Mass plan; they don't need to be here.</span></div>" +
          field("Default Mass location", input("parish.location", p.location, { ph: "None" })) + '<span class="o-meta pl-saved">' + icon("check") + "Saved</span>";
      } else if (S.spage === "displays") body = displays();
      else body = '<h2 class="o-title">Interface</h2>' + field("Interface scale", seg("scaleset", Math.round(S.scale * 100) + "%", ["100%", "125%", "150%", "175%", "200%"], "Interface scale"), "Makes Ordo's own text and controls larger on this computer. Slides are never affected: they're always composed at 1920 × 1080.") +
        '<div class="sh-inline o-meta">' + kbd("Ctrl++") + " larger · " + kbd("Ctrl+-") + " smaller · " + kbd("Ctrl+0") + " 100%</div>" +
        '<div class="sh-scaleprev"><div class="o-label">Slides at this scale</div><div class="sh-mini">' + K.slide({ k: "lyrics", h: "Koro", b: "Daygon, daygon ta\nang ngalan sa Ginoo" }) + "</div></div>";
      return '<div class="sh-page"><header class="sh-head"><h1 class="o-title">Settings</h1></header><div class="sg-panes"><div class="o-pane sh-nav"><div class="o-pane-body">' +
        pages.map(function (t) { return '<button class="o-row sh-navrow" data-a="set" data-path="spage" data-v="' + t[0] + '" aria-current="' + (S.spage === t[0]) + '">' + icon(t[1]) + t[2] + "</button>"; }).join("") +
        '<div class="pl-menu-sep"></div><button class="o-row sh-navrow" data-a="go" data-dest="diagnostics">' + icon("activity") + "Diagnostics" + icon("arrow-up-right", "sh-ext") + "</button>" +
        '</div></div><div class="o-pane sg-detail"><div class="o-pane-body sh-doc">' + body + "</div></div></div></div>";
    }
    function displays() {
      var d = S.disp, two = d.count > 1, cong = two && !d.unavailable ? d.cong : two && d.unavailable ? d.cong : "";
      var tile = function (n, name, res, pos, op) {
        var isC = d.cong === n && two, un = isC && d.unavailable;
        return '<button class="sh-disp' + (isC ? " is-cong" : "") + (un ? " is-un" : "") + (op ? " is-op" : "") + '" data-a="' + (op ? "toast" : "setcong") + '" data-v="' + n + '" data-msg="This is the operator display; Ordo runs here"' + (op ? "" : ' aria-pressed="' + isC + '"') + ">" +
          (d.identify ? '<span class="sh-ident">' + n + "</span>" : "") + "<b>" + n + " · " + esc(name) + '</b><span class="o-meta">' + res + " · 16:9 · " + pos + "</span>" +
          (op ? '<span class="pl-chip">' + icon("monitor") + "Operator display</span>" : un ? statusLabel("unavailable", "Unavailable", "ring") : isC ? '<span class="pl-chip sh-chip-def">' + icon("presentation") + "Congregation</span>" : '<span class="o-meta">Recognised</span>') + "</button>";
      };
      var topo = '<div class="sh-topo">' + tile("1", "Built-in display", "1366 × 768", "Left", true) + (two ? tile("2", "EPSON EB-X51", "1920 × 1080", "Right") : "") + "</div>";
      var pick = '<div class="sh-inline"><span class="o-label">Congregation display</span>' + sel("disp.cong", two ? d.cong : "", two ? [["2", "2 · EPSON EB-X51"], ["", "Not assigned"]] : [["", "Not assigned"]], "Congregation display") +
        '<button class="o-btn o-btn--primary" data-a="identify">' + icon("scan-eye") + "Identify displays</button></div>";
      var note = !two ? '<div class="o-notice" data-state="info">' + icon("info") + "<div><b>Only one display is connected.</b> Presenter still opens, with congregation output off. Connect the projector to assign it.</div></div>"
        : d.unavailable ? '<div class="o-notice" data-state="warning">' + icon("unplug") + "<div><b>Display 2 isn't connected.</b> It stays the congregation display; Ordo won't pick another one by itself. Reconnect it, or choose again.</div></div>"
        : d.changed ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div><b>Check calibration.</b> The congregation display changed, so the saved Output Fit may not suit it.</div></div>" : "";
      var cal = two && d.cong && !d.unavailable ? '<h3 class="sh-sec">Calibration</h3><div class="sh-cal"><div class="sh-calctl">' +
        field("Output Fit", '<span class="sh-unit"><input type="range" min="0" max="10" step="1" data-in="disp.fit" value="' + d.fit + '"> ' + d.fit + "%</span>", "Shrinks the whole frame evenly for overscan the projector and Windows settings can't fix. The edge is black.") +
        '<div class="sh-inline">' + (d.pattern ? '<button class="o-btn o-btn--primary" data-a="pattern">' + icon("square") + "Return to black</button>" + statusLabel("live", "Display 2 is showing the calibration pattern") : '<button class="o-btn" data-a="pattern">' + icon("grid-3x3") + "Show calibration pattern</button>") + "</div>" +
        '<span class="o-meta">Last checked ' + esc(d.checked) + ". Settings never shows Mass content on the congregation display.</span></div>" +
        '<div class="sh-calprev"><div class="o-label">Preview · what Display 2 shows</div><div class="sh-fitbox"><div class="sh-fitted" style="transform:scale(' + (1 - d.fit / 100) + ')">' + (d.pattern ? calPattern() : '<div class="o-slide"><div class="o-canvas" data-t="black"></div></div>') + "</div></div></div></div>" : "";
      return '<h2 class="o-title">Displays</h2><p class="o-meta">For this computer only. Mass plans, Themes and packages never carry these settings.</p>' + topo + pick + note + cal;
    }
    function calPattern() {
      return '<div class="o-slide sh-pattern"><span class="sh-safe"></span><span class="sh-cross"></span><span class="sh-pl">1920 × 1080 · safe area 120 × 80</span></div>';
    }

    // ── frame, events ────────────────────────────────────────────────────────────────────────
    function render() {
      app.style.setProperty("--o-scale", S.scale);
      var keep = {}, ae = document.activeElement, fk = ae && app.contains(ae) && ae.dataset ? ae.dataset.focusKey : null, caret = fk && ae.selectionStart;
      [".sh-scroll", ".sh-form", ".o-overlay-body", ".sh-doc"].forEach(function (q) { var el = app.querySelector(q); if (el) keep[q] = el.scrollTop; });
      var body, ov = "";
      if (S.firstRun) body = setupView();
      else if (S.dest === "masses") body = masses();
      else if (S.dest === "themes") body = S.tview === "editor" ? editor() : themesLib();
      else if (S.dest === "transfer") body = xfer();
      else if (S.dest === "diagnostics") body = diag();
      else if (S.dest === "help") body = help();
      else body = settings();
      if (S.overlay === "newcel") ov = newCel();
      else if (S.overlay === "addmass") ov = addMassOver();
      else if (S.overlay === "reuse") ov = reuseOver();
      else if (S.overlay === "export") ov = expOver();
      else if (S.overlay === "mk") ov = mkOver();
      else if (S.overlay === "decide") ov = decideOver();
      else if (S.overlay === "review") ov = reviewOver();
      else if (S.overlay === "hist") ov = histOver();
      if (S.keys) ov += keysPanel();
      if (S.dialog) ov += dialogHtml();
      var toast = S.toast ? '<div class="o-toast" role="status">' + esc(S.toast) + (S.review && S.toastReview ? '<button class="o-btn" data-a="review">Review affected plans</button>' : "") + (S.undo ? '<button class="o-btn" data-a="undo">Undo</button>' : "") + "</div>" : "";
      app.innerHTML = S.firstRun ? body + ov + toast : '<div class="o-body">' + K.navrail(RAIL[S.dest]) + '<div class="o-grow sh-ws">' + body + "</div></div>" + ov + toast;
      K.icons();
      Object.keys(keep).forEach(function (q) { var el = app.querySelector(q); if (el) el.scrollTop = keep[q]; });
      var f = fk && app.querySelector('[data-focus-key="' + fk + '"]');
      if (S.dialog && S.dialog.kind === "restore") f = app.querySelector('[data-focus-key="keepcur"]');
      if (f) { f.focus(); if (caret != null && f.setSelectionRange) try { f.setSelectionRange(caret, caret); } catch (e) { /* number inputs */ } }
    }
    function dialogHtml() {
      var d = S.dialog;
      if (d.kind === "restore") return rstDialog();
      if (d.kind === "dirty") return '<div class="o-scrim"></div><div class="o-dialog" role="alertdialog" aria-modal="true" aria-labelledby="sh-dd"><div class="o-overlay-head"><h2 class="o-title" id="sh-dd">Save changes to ' + esc(S.ed.draft.name) + "?</h2></div>" +
        '<div class="o-overlay-body"><p>Leaving now would lose the changes that aren\'t saved.</p></div><div class="o-overlay-foot"><button class="o-btn" data-a="closedlg">Keep editing</button><button class="o-btn" data-a="discard">Discard changes</button><button class="o-btn o-btn--primary" data-a="savego">Save changes</button></div></div>';
      if (d.kind === "save") { var t = theme(S.ed.id); return '<div class="o-scrim"></div><div class="o-dialog" role="alertdialog" aria-modal="true" aria-labelledby="sh-sv"><div class="o-overlay-head"><h2 class="o-title" id="sh-sv">Save ' + esc(t.name) + " for " + t.plans + " Mass plans?</h2></div>" +
        '<div class="o-overlay-body"><p>Every Mass plan that uses ' + esc(t.name) + " will show the change. Presentations already made don't change, and Undo stays available after saving.</p></div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-a="closedlg">Keep editing</button><button class="o-btn" data-a="duplicate" data-id="' + t.id + '">Duplicate instead</button><button class="o-btn o-btn--primary" data-a="dosave">Save for ' + t.plans + " plans</button></div></div>"; }
      if (d.kind === "name") return '<div class="o-scrim"></div><div class="o-dialog" role="dialog" aria-modal="true" aria-labelledby="sh-nm"><div class="o-overlay-head"><h2 class="o-title" id="sh-nm">' + esc(d.title) + "</h2></div>" +
        '<div class="o-overlay-body">' + field("Name", input("dialog.value", d.value)) + (d.note ? '<p class="o-meta">' + d.note + "</p>" : "") + '</div><div class="o-overlay-foot"><button class="o-btn" data-a="closedlg">Cancel</button><button class="o-btn o-btn--primary" data-a="namego"' + (d.value.trim() ? "" : " disabled") + ">" + esc(d.ok) + "</button></div></div>";
      return "";
    }
    function setPath(path, v) {
      var parts = path.split("."), o = S;
      for (var i = 0; i < parts.length - 1; i++) o = o[parts[i]];
      var k = parts[parts.length - 1], old = o[k];
      if (typeof old === "number" && v !== "" && !isNaN(Number(v))) v = Number(v);
      o[k] = v;
    }
    function go(dest, el) {
      S.dest = dest; S.overlay = null; S.menu = null; S.dialog = null;
      if (dest === "themes") { S.tview = "library"; S.ed = null; }
      if (el && el.dataset.page) S.spage = el.dataset.page;
      if (dest === "transfer") { S.xview = "home"; if (el && el.dataset.x === "import") openImport(); }
    }
    function openImport() { S.xview = "import"; S.imp = { decided: false }; }
    function editTheme(id, from) { var t = theme(id); S.dest = "themes"; S.tview = "editor"; S.ed = { id: id, draft: copy(t), group: "typography", spec: "lyrics", role: "Lyrics and responses", from: from || null }; S.menu = null; }
    function leaveEditor() { S.tview = "library"; S.ed = null; S.dialog = null; }
    function newTheme(src, name) {
      snapshot();
      var base = theme(src), t = copy(base); t.id = "th" + Date.now(); t.name = name; t.builtIn = false; t.archived = false; t.plans = 0; t.revs = [{ when: "Wed 28 Oct 2026, 14:02", what: "Created as a copy of " + base.name, plans: 0 }];
      S.themes.push(t); return t;
    }
    function saveTheme() {
      var t = theme(S.ed.id), before = copy(t), after = S.ed.draft;
      snapshot();
      var nt = copy(after); nt.revs = [{ when: "Wed 28 Oct 2026, 14:0" + (t.revs.length % 10), what: summary(before, after), plans: t.plans }].concat(t.revs);
      S.themes[S.themes.indexOf(t)] = nt; S.ed.draft = copy(nt);
      var list = deltas(before, after), need = list.reduce(function (a, g) { return a + g[1].length; }, 0);
      S.review = list.length ? { name: nt.name, list: list } : null; S.toastReview = !!list.length;
      done(nt.name + " saved · " + nt.plans + " Mass plans updated" + (need ? " · " + need + " need review" : ""), true);
    }
    function summary(a, b) {
      var out = [];
      [["size", "Lyrics size", " px"], ["min", "Minimum size", " px"], ["weight", "Weight", ""], ["lh", "Line spacing", ""], ["text", "Text", ""], ["bg", "Background", ""], ["opacity", "Overlay", "%"], ["align", "Alignment", ""], ["vpos", "Placement", ""], ["focal", "Focal point", ""]]
        .forEach(function (f) { if (a[f[0]] !== b[f[0]]) out.push(f[1] + " " + a[f[0]] + " → " + b[f[0]] + f[2]); });
      if (a.mode !== b.mode) out.push("Background " + a.mode + " → " + b.mode);
      if (a.image && !b.image) out.push("Image removed");
      return out.join(" · ") || "No visible change";
    }

    app.addEventListener("click", function (e) {
      var rail = e.target.closest(".o-navrail a");
      if (rail) { var d = DESTS[rail.dataset.dest]; if (d) { e.preventDefault(); if (S.ed && tdirty()) { S.dialog = { kind: "dirty", then: d }; render(); return; } go(d); render(); } return; }
      var t = e.target.closest("[data-a]"); if (!t || t.disabled) return;
      var a = t.dataset.a, id = t.dataset.id, keepToast = false;
      if (t.tagName === "INPUT" && t.type === "radio" && t.dataset.path) { setPath(t.dataset.path, t.dataset.v); render(); return; }
      if (a !== "celmenu" && a !== "thmenu") S.menu = null;
      switch (a) {
        case "go": go(t.dataset.dest, t); break;
        case "toast": done(t.dataset.msg); keepToast = true; break;
        case "set": setPath(t.dataset.path, t.dataset.v);
          if (t.dataset.path === "ed.draft.mode") S.ed.draft.mode = t.dataset.v === "Image" ? "image" : "solid";
          if (t.dataset.path === "scaleset") S.scale = parseInt(t.dataset.v, 10) / 100;
          break;
        case "toggle": S[t.dataset.path] = !S[t.dataset.path]; break;
        case "close": S.overlay = null; break;
        case "closedlg": S.dialog = null; break;
        case "undo": undo(); S.overlay = null; S.review = null; keepToast = true; break;
        // Masses
        case "mtab": S.mtab = t.dataset.v; break;
        case "collapse": var c0 = cel(id); S.collapsed[id] = !(S.collapsed[id] == null ? c0.when === "history" : S.collapsed[id]); break;
        case "celmenu": S.menu = S.menu === id ? null : id; break;
        case "clearm": S.q = ""; S.f = { lang: "", choir: "", ready: "" }; break;
        case "open": var mx; S.cels.forEach(function (c) { c.masses.forEach(function (y) { if (y.id === id) mx = y; }); });
          if (opts.navigate) window.location.href = "mass-planning-flows.html"; else { done("Opens " + mx.time + " · " + mx.lang + " in the Mass Planner" + (mx.presented ? ", read-only" : "")); keepToast = true; } break;
        case "addmass": openAddMass(id); break;
        case "ammethod": S.am.method = t.dataset.v; S.am.src = null; break;
        case "amsrc": var ps = planById(id).x; S.am.src = id; if (S.am.method === "reuse") { S.am.lang = ps.lang; S.am.choir = ps.choir; } S.am.presider = ps.presider === "No presider" ? "" : ps.presider; break;
        case "amant": S.am.ant = !S.am.ant; break;
        case "createmass": createMass(); keepToast = true; break;
        case "reuse": openReuse(id); break;
        case "reuseplan": var rp = planById(id); openReuse(rp.c.id, id); break;
        case "rupick": S.ru.pick[id] = !S.ru.pick[id]; break;
        case "doreuse": doReuse(); keepToast = true; break;
        case "export": S.overlay = "export"; S.exp = { cel: id, same: id === "c3", done: false }; break;
        case "archivecel": snapshot(); cel(id).archived = true; done(cel(id).title + " archived", true); keepToast = true; break;
        case "restorecel": snapshot(); cel(id).archived = false; done(cel(id).title + " restored", true); keepToast = true; break;
        case "newcel": S.overlay = "newcel"; S.newCel = { date: "Sun 22 Nov 2026", title: "", season: "Solemnity", color: "White", first: "template", tpl: "t1", time: "09:00", lang: "Cebuano" }; break;
        case "createcel": snapshot(); var n = S.newCel, nc = { id: "c" + Date.now(), when: "upcoming", date: n.date, title: n.title, season: n.season, color: n.color, hex: { Green: "#3F7A4E", White: "#F4F1EA", Violet: "#6B4C8A", Red: "#B03A2E", Rose: "#D98BA6", Black: "#222" }[n.color], masses: [] };
          var idx = S.cels.filter(function (c) { return c.when === "upcoming" && !c.archived; }).length; S.cels.splice(idx, 0, nc); S.mtab = "upcoming"; S.overlay = null;
          done(nc.title + " created", true); keepToast = true; if (n.first !== "later") openAddMass(nc.id, n.first); break;
        // first run
        case "addpres": S.setup.pres.push(S.setup.draft.trim()); S.setup.draft = ""; break;
        case "unpres": S.setup.pres.splice(+t.dataset.i, 1); break;
        case "startparish": S.parish = { name: S.setup.name.trim(), location: S.setup.loc.trim(), presiders: S.setup.pres.slice(), presider: S.setup.pres[0] || "" };
          S.firstRun = false; S.cels = []; S.themes = S.themes.filter(function (x) { return x.builtIn; }); S.dflt = "ordo-black"; S.backup = { days: 0, changes: 0 }; S.backups = []; S.packages = [];
          done(S.parish.name + " is ready"); keepToast = true; break;
        case "setuprestore": done("Opens the file chooser, then the Restore workspace; there's no current parish to protect"); keepToast = true; break;
        // Themes
        case "thmenu": S.menu = S.menu === id ? null : id; break;
        case "edit": editTheme(id); break;
        case "newtheme": var nt = newTheme("ordo-black", "Parish theme " + S.themes.filter(function (x) { return !x.builtIn; }).length); editTheme(nt.id); done("Parish theme created from Ordo Black"); keepToast = true; break;
        case "duplicate": var src = S.ed && S.ed.id === id ? S.ed.draft : theme(id);
          S.dialog = { kind: "name", act: "dup", src: id, title: "Duplicate " + theme(id).name, value: src.name + " copy", ok: "Create copy", note: S.ed && tdirty() ? "The copy includes your unsaved changes. " + theme(id).name + " itself stays as it was." : "Mass plans using " + theme(id).name + " keep it." }; break;
        case "rename": S.dialog = { kind: "name", act: "rename", src: id, title: "Rename Theme", value: theme(id).name, ok: "Rename" }; break;
        case "namego": var dl = S.dialog;
          if (dl.act === "rename") { snapshot(); theme(dl.src).name = dl.value.trim(); done("Renamed to " + dl.value.trim(), true); }
          else { var draft = S.ed && S.ed.id === dl.src ? S.ed.draft : theme(dl.src), cp = newTheme(dl.src, dl.value.trim()); var keepId = cp.id, keepRevs = cp.revs; Object.assign(cp, copy(draft)); cp.id = keepId; cp.name = dl.value.trim(); cp.builtIn = false; cp.plans = 0; cp.revs = keepRevs; editTheme(cp.id, S.ed && S.ed.from); done(cp.name + " created · now editing the copy", true); }
          S.dialog = null; keepToast = true; break;
        case "setdefault": snapshot(); S.dflt = id; done(theme(id).name + " is the parish default for new Mass plans", true); keepToast = true; break;
        case "archiveth": snapshot(); theme(id).archived = true; done(theme(id).name + " archived · " + theme(id).plans + " Mass plans keep it", true); keepToast = true; break;
        case "restoreth": snapshot(); theme(id).archived = false; done(theme(id).name + " restored", true); keepToast = true; break;
        case "tback": if (tdirty()) S.dialog = { kind: "dirty", then: "library" }; else leaveEditor(); break;
        case "discard": var then = S.dialog.then; leaveEditor(); if (then !== "library") go(then); break;
        case "savego": var then2 = S.dialog.then; S.dialog = null; saveTheme(); leaveEditor(); if (then2 !== "library") go(then2); keepToast = true; break;
        case "tsave": if (theme(S.ed.id).plans > 1) S.dialog = { kind: "save" }; else { saveTheme(); keepToast = true; } break;
        case "dosave": S.dialog = null; saveTheme(); keepToast = true; break;
        case "review": S.overlay = "review"; S.toast = null; break;
        case "thist": S.overlay = "hist"; S.hist = { i: 0 }; break;
        case "rev": S.hist.i = +t.dataset.i; break;
        case "restorerev": var tt = theme(S.ed.id), r = tt.revs[S.hist.i]; snapshot(); tt.revs.unshift({ when: "Wed 28 Oct 2026, 14:05", what: "Restored " + r.when, plans: tt.plans }); if (S.hist.i === 1) tt.size = 88; S.ed.draft = copy(tt); S.overlay = null;
          done(tt.name + " restored to " + r.when + " · " + tt.plans + " Mass plans updated", true); keepToast = true; break;
        case "replaceimg": S.ed.draft.image = { file: "nave-ordinary-wide.jpg", w: 2560, h: 1440 }; done("Image chosen · 2560 × 1440, 16:9"); keepToast = true; break;
        case "removeimg": S.ed.draft.image = null; S.ed.draft.mode = "solid"; break;
        // Transfer & Backup
        case "exportpick": S.overlay = "export"; S.exp = { cel: null, done: false }; break;
        case "expcel": S.exp = { cel: id, same: id === "c3", done: false }; break;
        case "doexport": S.exp.done = true; if (!S.exp.same) { var pk = S.packages.filter(function (p) { return p.cel === S.exp.cel; })[0]; if (pk) { pk.versions.forEach(function (v) { v.current = false; }); pk.versions.unshift({ v: pk.versions.length + 1, when: "Wed 28 Oct 2026, 14:03", from: "This computer", plans: cel(S.exp.cel).masses.length, fp: "b81d·3e07", current: true, exported: true }); } } break;
        case "mkbackup": S.overlay = "mk"; S.mk = { done: false }; break;
        case "domk": S.mk.done = true; S.backup = { days: 0, changes: 0 }; S.backups.unshift({ id: "b" + Date.now(), kind: "portable", file: "Parokya-ni-San-Roque_Backup_2026-10-28_1402.ordo", where: "USB drive (E:)", when: "Wed 28 Oct 2026, 14:02", size: "418 MB", state: "Verified" }); break;
        case "pkopen": S.menu = S.menu === "pk:" + id ? null : "pk:" + id; break;
        case "import": openImport(); break;
        case "decide": S.overlay = "decide"; break;
        case "decided": S.imp.decided = true; S.overlay = null; break;
        case "cancelimp": S.xview = "home"; S.imp = null; done("Import cancelled · nothing changed"); keepToast = true; break;
        case "doimport": snapshot(); S.xview = "home"; S.packages.unshift({ id: "p" + Date.now(), cel: "c4", title: "Ika-33 nga Domingo sa Ordinaryong Panahon", date: "Sun 15 Nov 2026", versions: [{ v: 1, when: "Wed 28 Oct 2026, 14:04", from: "Home PC", plans: 2, fp: "c47a·9e12", current: true }] });
          var c4 = cel("c4"); if (c4 && !c4.masses.length) { c4.installed = true; c4.masses.push(m("i1", "Sun 15 Nov · 06:00", "Cebuano", "Koro Sta. Cecilia", 0, 0), m("i2", "Sun 15 Nov · 09:00", "Cebuano", "Koro San Roque", 0, 1)); }
          done("Celebration imported · 2 Mass plans ready in Masses"); keepToast = true; break;
        case "restore": S.xview = "restore"; S.rst = { protected: false }; break;
        case "protect": S.rst.protected = true; break;
        case "cancelrst": S.xview = "home"; break;
        case "rstconfirm": S.dialog = { kind: "restore" }; break;
        case "dorestore": S.dialog = null; S.xview = "home"; done("Restoring… Ordo restarts into the backup's parish"); keepToast = true; break;
        // Diagnostics
        case "dscan": S.diag.state = "scan"; S.diag.pct = 0; S.diag.cur = 0; S.diag.started = "14:08"; if (!opts.still) scanTimer = setInterval(scanStep, 300); break;
        case "dcancel": clearInterval(scanTimer); scanTimer = null; S.diag.state = "done"; S.diag.cancelled = S.diag.started || "14:08"; break;
        case "dopen": S.diag.open[id] = !S.diag.open[id]; break;
        // Help
        case "keys": S.keys = !S.keys; break;
        // Settings
        case "defpres": S.parish.presider = S.parish.presiders[+t.dataset.i]; done(S.parish.presider + " is the default presider for new Mass plans"); keepToast = true; break;
        case "rmpres": snapshot(); var gone = S.parish.presiders.splice(+t.dataset.i, 1)[0]; if (S.parish.presider === gone) S.parish.presider = S.parish.presiders[0] || ""; done(gone + " removed from regular presiders · plans that name them keep the name", true); keepToast = true; break;
        case "addrpres": S.parish.presiders.push(S.pdraft.trim()); S.pdraft = ""; break;
        case "setcong": if (S.disp.cong !== t.dataset.v) { S.disp.cong = t.dataset.v; S.disp.changed = true; } break;
        case "identify": S.disp.identify = true; if (!opts.still) setTimeout(function () { S.disp.identify = false; render(); }, 2500); done("Each display shows its number for a few seconds"); keepToast = true; break;
        case "pattern": S.disp.pattern = !S.disp.pattern; if (!S.disp.pattern) { S.disp.checked = "Wed 28 Oct 2026, 14:09"; S.disp.changed = false; } break;
      }
      if (!keepToast) { S.toast = null; S.undo = null; S.toastReview = false; }
      render();
    });
    function onInput(e) {
      var el = e.target, path = el.dataset && el.dataset.in; if (!path) return;
      setPath(path, el.value);
      if (path === "disp.cong") S.disp.changed = true;
      if (path === "am.lang") { var rt = recTemplate(cel(S.am.cel), S.am.lang); S.am.tpl = rt ? rt.id : ""; }
      if (path === "parish.name" || path === "parish.location") S.toast = null;
      render();
    }
    // Sliders commit on release, so re-rendering never interrupts a drag.
    app.addEventListener("input", function (e) { if (e.target.tagName !== "SELECT" && e.target.type !== "range") onInput(e); });
    app.addEventListener("change", function (e) { if (e.target.tagName === "SELECT" || e.target.type === "range") onInput(e); });
    app.addEventListener("keydown", function (e) {
      if (e.ctrlKey && e.key === "/") { e.preventDefault(); S.keys = !S.keys; render(); return; }
      if (e.ctrlKey && (e.key === "=" || e.key === "+" || e.key === "-" || e.key === "0")) {
        e.preventDefault(); var steps = [1, 1.25, 1.5, 1.75, 2], i = steps.indexOf(S.scale);
        S.scale = e.key === "0" ? 1 : steps[Math.max(0, Math.min(4, i + (e.key === "-" ? -1 : 1)))]; done("Interface scale " + Math.round(S.scale * 100) + "%"); render(); return;
      }
      if (e.ctrlKey && e.key === "s" && S.ed) { e.preventDefault(); if (tdirty() && !theme(S.ed.id).builtIn) { S.dialog = theme(S.ed.id).plans > 1 ? { kind: "save" } : null; if (!S.dialog) saveTheme(); render(); } return; }
      if (e.key === "Escape") { if (S.dialog) S.dialog = null; else if (S.keys) S.keys = false; else if (S.overlay) S.overlay = null; else if (S.menu) S.menu = null; else return; render(); }
      if (e.key === "Enter" && e.target.dataset && e.target.dataset.in === "setup.draft" && S.setup.draft.trim()) { S.setup.pres.push(S.setup.draft.trim()); S.setup.draft = ""; render(); }
    });

    var api = {
      dest: function (d, o) { go(d); if (o) Object.keys(o).forEach(function (k) { S[k] = o[k]; }); return api; },
      firstRun: function (mode, name) { S.firstRun = true; S.setup.mode = mode || null; S.setup.name = name || ""; if (name) S.setup.pres = ["Fr. Ramon Abellana"]; return api; },
      emptyParish: function () { S.cels = []; return api; }, tab: function (t) { S.mtab = t; return api; }, filter: function (k, v) { S.f[k] = v; return api; },
      menu: function (id) { S.menu = id; return api; },
      addMass: function (celId, o) { o = o || {}; openAddMass(celId, o.method); Object.keys(o).forEach(function (k) { S.am[k] = o[k]; }); if (o.lang && !("tpl" in o)) { var r = recTemplate(cel(celId), o.lang); S.am.tpl = r ? r.id : ""; } return api; },
      reuse: function (celId, o) { o = o || {}; openReuse(celId, o.plan); Object.keys(o).forEach(function (k) { if (k !== "plan") S.ru[k] = o[k]; }); return api; }, newCel: function (title) { S.overlay = "newcel"; S.newCel = { date: "Sun 22 Nov 2026", title: title || "", season: "Solemnity", color: "White", first: "template", tpl: "t1", time: "09:00", lang: "Cebuano" }; return api; },
      noParishThemes: function () { S.themes = S.themes.filter(function (t) { return t.builtIn; }); S.dflt = "ordo-black"; return api; }, themeArchived: function () { S.tarch = true; return api; },
      edit: function (id, o) { editTheme(id); if (o) Object.keys(o).forEach(function (k) { S.ed[k] = o[k]; }); return api; },
      draft: function (o) { Object.keys(o).forEach(function (k) { S.ed.draft[k] = o[k]; }); return api; },
      missingImage: function (id) { theme(id).image.missing = true; return api; },
      save: function () { saveTheme(); return api; }, review: function () { S.overlay = "review"; S.toast = null; return api; }, history: function (i) { S.overlay = "hist"; S.hist = { i: i || 0 }; return api; },
      dialog: function (kind, o) { S.dialog = Object.assign({ kind: kind }, o || {}); return api; },
      duplicateDialog: function (id) { var t = theme(id); S.dialog = { kind: "name", act: "dup", src: id, title: "Duplicate " + t.name, value: t.name + " copy", ok: "Create copy", note: "The copy includes your unsaved changes. " + t.name + " itself stays as it was." }; return api; },
      export: function (celId, o) { S.overlay = "export"; S.exp = Object.assign({ cel: celId || null, same: celId === "c3", done: false }, o || {}); return api; },
      makeBackup: function (isDone) { S.overlay = "mk"; S.mk = { done: !!isDone }; return api; },
      importView: function (o) { openImport(); Object.assign(S.imp, o || {}); return api; }, decide: function () { S.overlay = "decide"; return api; },
      restoreView: function (o) { S.xview = "restore"; S.rst = Object.assign({ protected: false }, o || {}); return api; },
      damaged: function () { S.backups[1].state = "Damaged"; return api; }, backedUp: function () { S.backup = { days: 0, changes: 0 }; return api; },
      openPackage: function (id) { S.menu = "pk:" + id; return api; },
      healthy: function () { S.diag.healthy = true; return api; }, scanning: function (pct) { S.diag.state = "scan"; S.diag.pct = pct; S.diag.cur = Math.floor(pct / (100 / DIAG.length)); return api; },
      cancelled: function () { S.diag.cancelled = "14:08"; return api; }, openDiag: function (id) { S.diag.open[id] = true; return api; },
      help: function (topic) { S.help = topic; return api; }, keys: function () { S.keys = true; return api; },
      settings: function (page) { S.dest = "settings"; S.spage = page; return api; }, disp: function (o) { Object.keys(o).forEach(function (k) { S.disp[k] = o[k]; }); return api; },
      scale: function (s) { S.scale = s; return api; }, quiet: function () { S.toast = null; S.undo = null; S.toastReview = false; return api; }
    };
    K.fit(wrapper, app, W, H);
    if (opts.setup) opts.setup(api);
    render();
    return api;
  }

  K.shell = shell;
})();
