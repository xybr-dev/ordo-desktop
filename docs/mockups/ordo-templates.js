// Ordo mockups: Masses → Templates and the Template Editor on the shared kit (load after
// ordo-kit.js). Each instance owns a working copy of the template fixture, so its flows can edit
// and Undo without touching other mockups. Mockup behaviour only.
(function () {
  "use strict";
  var K = window.OrdoKit, esc = K.esc, icon = K.icon, statusLabel = K.statusLabel;
  function copy(x) { return JSON.parse(JSON.stringify(x)); }
  function norm(s) { return String(s || "").toLowerCase().replace(/\s+/g, " ").trim(); }

  var SEASONS = ["Advent", "Christmas", "Ordinary Time", "Lent", "Easter"];
  var DAYS = ["Sunday", "Solemnity", "Feast", "Weekday"];
  var ROLES = ["First Reading", "Responsorial Psalm", "Second Reading", "Gospel Acclamation", "Gospel"];
  var MOMENTS = ["Entrance", "Penitential Act", "Gloria", "Preparation of the Gifts", "Sanctus", "Lord's Prayer", "Agnus Dei", "Communion", "Recessional"];
  var TYPES = { song: ["music", "Song slot"], reading: ["book-open", "Reading slot"], resp: ["messages-square", "Response sequence"], general: ["presentation", "General Presentation"], hold: ["square", "Hold"] };
  // Sample Celebrations for "Check against a Celebration": what a template's rules decide for each.
  var SAMPLES = [
    { id: "ot", name: "Ika-32 nga Domingo · Sun 8 Nov", season: "Ordinary Time", day: "Sunday" },
    { id: "adv", name: "Unang Domingo sa Adviento · Sun 29 Nov", season: "Advent", day: "Sunday" },
    { id: "lent", name: "Ikaduhang Domingo sa Kwaresma · Sun 28 Feb", season: "Lent", day: "Sunday" },
    { id: "sol", name: "All Saints · Sun 1 Nov", season: "Ordinary Time", day: "Solemnity" },
    { id: "wk", name: "Tuesday of week 31 · Tue 3 Nov", season: "Ordinary Time", day: "Weekday" },
    { id: "none", name: "A Celebration with no season set", season: "", day: "Sunday" }
  ];

  var nid = 100;
  function it(type, title, o) {
    o = o || {};
    return { id: "i" + (nid++), type: type, title: title, need: o.need || "Required", moment: o.moment || "", role: o.role || "", rule: o.rule || null,
      ex: o.ex || null, slides: o.slides || null, note: o.note || "" };
  }
  function rule(mode, seasons, days) { return { mode: mode, seasons: seasons || [], days: days || [] }; }
  function sunday() {
    return [
      { id: "s1", name: "Before Mass", items: [it("general", "Pahibalo", { need: "Optional", slides: ["Maayong buntag! Palihug isira o i-silent ang inyong mga cellphone."] })] },
      { id: "s2", name: "Introductory Rites", items: [
        it("song", "Entrance Song", { moment: "Entrance" }),
        it("resp", "Greeting", { ex: [["P", "Sa ngalan sa Amahan, ug sa Anak, ug sa Espiritu Santo."], [null, "Amen."], ["P", "Ang Ginoo maanaa kaninyo."], [null, "Ug maanaa usab sa imong espiritu."]] }),
        it("song", "Kyrie", { moment: "Penitential Act" }),
        it("song", "Gloria", { moment: "Gloria", need: "Conditional", rule: rule("omit", ["Advent", "Lent"], ["Weekday"]) })] },
      { id: "s3", name: "Liturgy of the Word", items: [
        it("reading", "First Reading", { role: "First Reading" }), it("reading", "Responsorial Psalm", { role: "Responsorial Psalm" }),
        it("reading", "Second Reading", { role: "Second Reading", need: "Conditional", rule: rule("include", [], ["Sunday", "Solemnity"]) }),
        it("reading", "Gospel Acclamation", { role: "Gospel Acclamation" }), it("reading", "Gospel", { role: "Gospel" }),
        it("resp", "Creed", { need: "Conditional", rule: rule("include", [], ["Sunday", "Solemnity"]), ex: [["P", "Nagatoo ako sa Dios…"], [null, "Nagatoo ako sa usa ka Dios, ang Amahan nga makagagahom…"]] }),
        it("resp", "Prayer of the Faithful", { ex: [["L", "Mag-ampo kita sa Ginoo."], [null, "Ginoo, patalinghugi ang among pag-ampo."]] })] },
      { id: "s4", name: "Liturgy of the Eucharist", items: [
        it("song", "Offertory", { moment: "Preparation of the Gifts" }), it("song", "Sanctus", { moment: "Sanctus" }),
        it("resp", "Memorial Acclamation", { ex: [["P", "Ang misteryo sa pagtoo."], [null, "Among ginasangyaw ang imong kamatayon, O Ginoo…"]] }),
        it("song", "Amahan Namo", { moment: "Lord's Prayer" }), it("song", "Agnus Dei", { moment: "Agnus Dei" }), it("song", "Communion", { moment: "Communion" })] },
      { id: "s5", name: "Concluding Rites", items: [
        it("resp", "Dismissal", { ex: [["D", "Lakaw kamo sa kalinaw sa Ginoo."], [null, "Salamat sa Dios."]] }), it("song", "Recessional", { moment: "Recessional", need: "Optional" })] },
      { id: "s6", name: "After Mass", items: [] }
    ];
  }
  function six() { return ["Before Mass", "Introductory Rites", "Liturgy of the Word", "Liturgy of the Eucharist", "Concluding Rites", "After Mass"].map(function (n, i) { return { id: "n" + (nid++), name: n, items: [] }; }); }
  var TEMPLATES = [
    { id: "t1", name: "Sunday Mass · Cebuano", lang: "Cebuano", purpose: "Every Sunday and Solemnity in Cebuano", seasons: [], days: ["Sunday", "Solemnity"], fallback: true, used: 41, edited: "Tue 20 Oct 2026", secs: sunday() },
    { id: "t2", name: "Sunday Mass · English", lang: "English", purpose: "The 9:00 English Mass", seasons: [], days: ["Sunday"], fallback: true, used: 12, edited: "Sun 4 Oct 2026", secs: sunday() },
    { id: "t3", name: "Weekday Mass · Cebuano", lang: "Cebuano", purpose: "Weekday morning Masses; no Gloria, Creed or Second Reading", seasons: [], days: ["Weekday"], fallback: false, used: 23, edited: "Mon 7 Sep 2026", secs: sunday() },
    { id: "t4", name: "Simbang Gabi · Cebuano", lang: "Cebuano", purpose: "The nine dawn Masses before Christmas", seasons: ["Advent"], days: ["Weekday"], fallback: false, used: 9, edited: "Wed 10 Dec 2025", secs: sunday() },
    { id: "t5", name: "Funeral Mass · Cebuano", lang: "Cebuano", purpose: "", seasons: [], days: [], fallback: false, used: 3, edited: "Fri 14 Aug 2026", secs: sunday(), archived: true }
  ];

  function templates(wrapper, opts) {
    opts = opts || {};
    var W = opts.w || 1366, H = opts.h || 768;
    var app = document.createElement("div");
    app.className = "o-app tp-app"; app.tabIndex = -1;
    wrapper.classList.add("o-fit"); wrapper.appendChild(app);

    var S = { list: copy(TEMPLATES), view: "list", lang: "", arch: false, cur: null, sel: null, secSel: null, menu: null, dialog: null, add: false,
      check: "", toast: null, undo: null, renaming: null, draft: "" };
    var hist = [];
    function snapshot() { hist.push(JSON.stringify(S.list)); }
    function done(msg, undoable) { S.toast = msg; S.undo = !!undoable; }
    function undo() { var h = hist.pop(); if (!h) return; S.list = JSON.parse(h); S.toast = "Undone"; S.undo = false; if (S.cur && !tpl()) S.view = "list"; }
    function tpl() { return S.list.filter(function (t) { return t.id === S.cur; })[0]; }
    function find(id) { var r = null; tpl().secs.forEach(function (s) { s.items.forEach(function (x, i) { if (x.id === id) r = { sec: s, i: i, it: x }; }); }); return r; }
    function sec(id) { return tpl().secs.filter(function (s) { return s.id === id; })[0]; }
    function slots(t) { var n = 0; t.secs.forEach(function (s) { s.items.forEach(function (x) { if (x.type === "song" || x.type === "reading") n++; }); }); return n; }
    function edit(msg) { tpl().edited = "Just now"; done(msg, true); }

    // A rule decides inclusion for one Celebration (Decision 75). Missing metadata leaves it unresolved.
    function decide(x, c) {
      if (!x.rule || x.need !== "Conditional") return null;
      var r = x.rule, hitS = r.seasons.length && c.season && r.seasons.indexOf(c.season) >= 0, hitD = r.days.length && r.days.indexOf(c.day) >= 0;
      if (r.seasons.length && !c.season) return { st: "unresolved", why: "The Celebration has no season" };
      var hit = hitS || hitD, why = hit ? (hitS ? c.season : c.day) : "";
      if (r.mode === "omit") return hit ? { st: "omit", why: "Omitted: " + why } : { st: "in", why: "Included" };
      return hit ? { st: "in", why: "Included: " + why } : { st: "omit", why: "Omitted: not " + r.days.concat(r.seasons).join(" or ") };
    }
    function ruleText(r) {
      if (!r) return "No rule yet";
      var on = r.seasons.concat(r.days);
      if (!on.length) return "No rule yet";
      return (r.mode === "omit" ? "Omit on " : "Include only on ") + on.join(", ");
    }
    function sample() { return SAMPLES.filter(function (s) { return s.id === S.check; })[0]; }

    // ── Masses → Templates (Decisions 77, 79, 220) ────────────────────────────────────────────
    function head() {
      var tabs = [["upcoming", "Upcoming"], ["history", "History"], ["archive", "Archive"], ["templates", "Templates"]];
      return '<header class="sh-head"><h1 class="o-title">Masses</h1><div class="o-tabs sh-tabs" role="tablist">' +
        tabs.map(function (t) { return '<a role="tab" class="tp-tab" href="masses-index.html" aria-selected="' + (t[0] === "templates") + '">' + t[1] + "</a>"; }).join("") +
        '</div><span class="o-grow"></span><button class="o-btn o-btn--primary" data-a="new">' + icon("plus") + "New template</button></header>";
    }
    function menu(t) {
      var items = t.archived ? [["restore", "archive-restore", "Restore"]] : [["dup", "copy", "Duplicate"], ["fallback", "star", t.fallback ? t.lang + " fallback" : "Use as " + t.lang + " fallback"], ["archive", "archive", "Archive"]];
      return '<div class="o-menu sh-menu" role="menu">' + items.map(function (i) {
        return '<button class="o-row" role="menuitem" data-a="' + i[0] + '" data-id="' + t.id + '"' + (i[0] === "fallback" && t.fallback ? " disabled" : "") + ">" + icon(i[1]) + i[2] + "</button>";
      }).join("") + "</div>";
    }
    function listView() {
      var langs = [], rows;
      S.list.forEach(function (t) { if (langs.indexOf(t.lang) < 0) langs.push(t.lang); });
      var shown = S.list.filter(function (t) { return (!S.lang || t.lang === S.lang) && (S.arch || !t.archived); }), nArch = S.list.filter(function (t) { return t.archived; }).length;
      if (!S.list.length) return '<div class="sh-page">' + head() + '<div class="sh-scroll"><div class="sh-col"><div class="pl-empty"><b>No templates yet</b>' +
        '<span class="o-meta">A Mass template holds the Mass sections, recurring responses and Song and Reading slots a new Mass plan starts with, for one language. Mass plans can still be made without one.</span>' +
        '<button class="o-btn o-btn--primary" data-a="new">' + icon("plus") + "Create template</button></div></div></div></div>";
      rows = shown.map(function (t) {
        var chips = t.days.concat(t.seasons).map(function (x) { return '<span class="pl-chip">' + esc(x) + "</span>"; }).join("");
        return '<div class="o-row sh-trow tp-row' + (t.archived ? " is-subdued" : "") + '">' + icon("layout-template") +
          '<div class="o-grow"><div class="sh-celtitle"><b>' + esc(t.name) + "</b>" + (t.fallback ? '<span class="pl-chip sh-chip-def">' + icon("star") + "Fallback for " + esc(t.lang) + "</span>" : "") + (t.archived ? '<span class="pl-chip">' + icon("archive") + "Archived</span>" : "") + "</div>" +
          '<span class="o-meta">' + esc(t.lang) + " · " + slots(t) + " slots · used for " + t.used + " Mass plans · edited " + esc(t.edited) + (t.purpose ? " · " + esc(t.purpose) : "") + "</span>" + (chips ? '<div class="sg-chips">' + chips + "</div>" : "") + "</div>" +
          (t.archived ? "" : '<button class="o-btn" data-a="open" data-id="' + t.id + '">Edit</button>') +
          '<span class="sh-rel"><button class="o-btn o-btn--quiet o-btn--icon" data-a="menu" data-id="' + t.id + '" aria-label="More for ' + esc(t.name) + '">' + icon("ellipsis") + "</button>" + (S.menu === t.id ? menu(t) : "") + "</span></div>";
      }).join("");
      return '<div class="sh-page">' + head() + '<div class="sh-tools"><select class="pl-input pl-input--sel" data-in="lang" aria-label="Language"><option value="">All languages</option>' +
        langs.map(function (l) { return "<option" + (S.lang === l ? " selected" : "") + ">" + esc(l) + "</option>"; }).join("") + "</select>" +
        (nArch ? '<label class="sh-check"><input type="checkbox" data-a="arch"' + (S.arch ? " checked" : "") + "> Show archived (" + nArch + ")</label>" : "") + "</div>" +
        '<div class="sh-scroll"><div class="sh-col sh-col--wide"><p class="o-meta">New Celebrations recommend the template whose language, days and seasons fit best; the language fallback is used when nothing fits better (Decision 79). Editing a template changes only plans made afterwards.</p>' +
        '<div class="sh-list">' + (rows || '<div class="sh-none o-meta">No templates in this language.</div>') + "</div></div></div></div>";
    }

    // ── Template Editor (Decisions 75–78, 112, 170, 172, 177) ────────────────────────────────
    function outline() {
      var t = tpl(), c = sample(), html = "";
      t.secs.forEach(function (s, si) {
        var open = S.menu === "sec:" + s.id, nm = S.renaming === s.id;
        html += '<div class="tp-sechead' + (S.secSel === s.id ? " is-target" : "") + '">' + icon("grip-vertical", "tp-grip") +
          (nm ? '<input class="pl-input tp-secname" data-in="draft" data-focus-key="secname" value="' + esc(S.draft) + '" aria-label="Section name">' :
          '<button class="tp-secbtn" data-a="secsel" data-id="' + s.id + '"><b>' + esc(s.name) + '</b><span class="o-meta">' + s.items.length + "</span></button>") +
          '<span class="sh-rel"><button class="o-btn o-btn--quiet o-btn--icon tp-mini" data-a="secmenu" data-id="' + s.id + '" aria-label="Section actions">' + icon("ellipsis") + "</button>" + (open ? secMenu(s, si) : "") + "</span></div>";
        s.items.forEach(function (x) {
          var d = c ? decide(x, c) : null;
          html += '<button class="o-row pl-item tp-item' + (d && d.st === "omit" ? " is-omitted" : "") + '" data-a="sel" data-id="' + x.id + '"' + (x.id === S.sel ? ' aria-selected="true"' : "") + ">" +
            icon(TYPES[x.type][0], "tp-ic") + '<span class="o-grow o-ellipsis">' + esc(x.title) + "</span>" +
            (d ? (d.st === "omit" ? '<span class="o-meta">Omitted</span>' : d.st === "unresolved" ? statusLabel("warning", "", "square") : "") : x.need === "Conditional" ? '<span class="o-meta" title="' + esc(ruleText(x.rule)) + '">' + icon("split") + "</span>" : x.need === "Optional" ? '<span class="o-meta">Optional</span>' : "") + "</button>";
        });
        if (!s.items.length) html += '<div class="tp-emptysec o-meta">No items</div>';
      });
      return '<section class="o-pane pl-outline tp-outline" aria-label="Template outline"><div class="o-pane-head"><b>Template outline</b><span class="o-grow"></span>' +
        '<span class="sh-rel"><button class="o-btn" data-a="add" aria-expanded="' + S.add + '">' + icon("plus") + "Add</button>" + (S.add ? addMenu() : "") + "</span></div>" +
        '<div class="o-pane-body o-rows--open">' + html + "</div></section>";
    }
    function secMenu(s, si) {
      var t = tpl(), last = si === t.secs.length - 1;
      var it = [["rename", "Rename"], ["above", "Add section above"], ["below", "Add section below"], ["up", "Move up", si === 0], ["down", "Move down", last], ["rmsec", "Remove section", s.items.length > 0 || t.secs.length === 1]];
      return '<div class="o-menu sh-menu tp-secmenu" role="menu">' + it.map(function (i) {
        return '<button class="o-row" role="menuitem" data-a="' + i[0] + '" data-id="' + s.id + '"' + (i[2] ? " disabled" : "") + ">" + i[1] +
          (i[0] === "rmsec" && s.items.length ? '<span class="o-meta">Move or remove its items first</span>' : "") + "</button>";
      }).join("") + "</div>";
    }
    function addMenu() {
      var target = S.sel ? find(S.sel).sec : S.secSel ? sec(S.secSel) : tpl().secs[0];
      return '<div class="o-menu sh-menu tp-addmenu" role="menu"><div class="o-meta tp-dest">Adds to <b>' + esc(target.name) + "</b>" + (S.sel ? ", after the selected item" : "") + "</div>" +
        Object.keys(TYPES).map(function (k) { return '<button class="o-row" role="menuitem" data-a="additem" data-v="' + k + '">' + icon(TYPES[k][0]) + TYPES[k][1] + "</button>"; }).join("") + "</div>";
    }
    function seg(path, value, options, label) {
      return '<div class="o-segmented" role="group" aria-label="' + esc(label) + '">' + options.map(function (o) { return '<button data-a="set" data-path="' + path + '" data-v="' + esc(o) + '" aria-pressed="' + (value === o) + '">' + esc(o) + "</button>"; }).join("") + "</div>";
    }
    function field(label, html, hint) { return '<label class="sh-field"><span class="o-label">' + label + "</span>" + html + (hint ? '<span class="o-meta">' + hint + "</span>" : "") + "</label>"; }
    function input(path, v, ph) { return '<input class="pl-input" data-in="' + path + '" data-focus-key="' + path + '" value="' + esc(v) + '"' + (ph ? ' placeholder="' + esc(ph) + '"' : "") + ">"; }
    function sel(path, v, list, label) { return '<select class="pl-input pl-input--sel" data-in="' + path + '" aria-label="' + label + '">' + list.map(function (o) { return "<option" + (o === v ? " selected" : "") + ">" + esc(o) + "</option>"; }).join("") + "</select>"; }
    function checks(path, on, list) {
      return '<div class="tp-checks">' + list.map(function (o) { return '<label class="sh-check"><input type="checkbox" data-a="tick" data-path="' + path + '" data-v="' + esc(o) + '"' + (on.indexOf(o) >= 0 ? " checked" : "") + "> " + esc(o) + "</label>"; }).join("") + "</div>";
    }
    function ruleBox(x) {
      var r = x.rule || (x.rule = rule("include", [], []));
      return '<div class="tp-rule"><div class="o-label">Rule</div>' + seg("it.rule.mode", r.mode === "omit" ? "Omit on" : "Include only on", ["Include only on", "Omit on"], "Rule") +
        '<div class="sh-two"><div><div class="o-meta">Days</div>' + checks("rule.days", r.days, DAYS) + '</div><div><div class="o-meta">Seasons</div>' + checks("rule.seasons", r.seasons, SEASONS) + "</div></div>" +
        '<div class="o-meta">' + icon("info") + " " + esc(ruleText(r)) + ". In a plan the result shows with its reason and can be overridden for that Mass. If a Celebration lacks the season or day this rule needs, the item stays unresolved and Readiness warns.</div>" +
        '<div class="tp-results">' + SAMPLES.map(function (c) { var d = decide(x, c) || { st: "in", why: "Included" }; return '<div class="tp-res"><span class="o-grow">' + esc(c.name) + "</span>" +
          (d.st === "in" ? statusLabel("live", d.why) : d.st === "omit" ? '<span class="o-meta">' + esc(d.why) + "</span>" : statusLabel("warning", "Unresolved · " + d.why, "square")) + "</div>"; }).join("") + "</div></div>";
    }
    function itemEditor() {
      var t = tpl();
      if (!S.sel) {
        if (S.secSel) { var s = sec(S.secSel); return '<section class="o-pane pl-editor"><div class="o-pane-body pl-ed-body"><div class="pl-empty"><b>' + esc(s.name) + '</b><span class="o-meta">Select an item, or use <b>Add</b> to put a new one at the end of this section.</span></div></div></section>'; }
        return '<section class="o-pane pl-editor"><div class="o-pane-body pl-ed-body"><div class="pl-empty"><b>Select an item</b><span class="o-meta">Song and Reading slots are filled in each Mass plan. Response sequences and presentations are copied into the plan as they are here.</span></div></div></section>';
      }
      var f = find(S.sel), x = f.it, body = "";
      body += field("Name in the outline", input("it.title", x.title));
      if (x.type === "song" || x.type === "reading" || x.type === "general") body += field("In a new plan", seg("it.need", x.need, ["Required", "Optional", "Conditional"], "Requirement"),
        x.need === "Required" ? "Readiness reports it until it's filled." : x.need === "Optional" ? "Left empty without a warning; removing it from a plan is one click." : "Included or omitted by the rule below.");
      else body += field("In a new plan", seg("it.need", x.need === "Conditional" ? "Conditional" : "Always", ["Always", "Conditional"], "Inclusion"));
      if (x.need === "Conditional") body += ruleBox(x);
      if (x.type === "song") body += field("Usual moment", sel("it.moment", x.moment, MOMENTS, "Usual moment"), "Filters the Song chooser in each plan to this moment and the plan's choir. Ordo never picks a Song by itself (Decision 76).") +
        '<div class="tp-slot">' + icon("music") + "<div><b>Song chosen in each plan</b><br><span class=\"o-meta\">The template holds the place, not the Song.</span></div></div>";
      if (x.type === "reading") body += field("Reading role", sel("it.role", x.role, ROLES, "Reading role"), "Each plan's readings fill this role. A plan can add more Reading slots for an unusual Celebration.") +
        '<div class="tp-slot">' + icon("book-open") + "<div><b>Text chosen in each plan</b><br><span class=\"o-meta\">Readings change with every Celebration.</span></div></div>";
      if (x.type === "resp") body += '<div class="o-label">Exchanges</div><div class="sh-list">' + x.ex.map(function (e, i) {
        return '<div class="o-row tp-ex"><span class="pl-chip">' + (e[0] ? esc(e[0]) + " · prompt" : "Response") + '</span><input class="pl-input" data-in="ex.' + i + '" data-focus-key="ex.' + i + '" value="' + esc(e[1]) + '"></div>';
      }).join("") + '</div><div class="sh-inline"><button class="o-btn" data-a="addex">' + icon("plus") + 'Add exchange</button></div><p class="o-meta">Copied into each new plan, where it can be edited for that Mass only (Decision 112).</p>' +
        '<div class="pl-thumbs">' + x.ex.filter(function (e) { return !e[0]; }).map(function (e) { return K.slide({ k: "response", h: t.lang === "Cebuano" ? "TUBAG:" : "ALL:", b: e[1] }); }).join("") + "</div>";
      if (x.type === "general") body += '<div class="o-label">Slides</div>' + (x.slides || []).map(function (s, i) { return field("Slide " + (i + 1), '<textarea class="pl-input" rows="2" data-in="slide.' + i + '" data-focus-key="slide.' + i + '">' + esc(s) + "</textarea>"); }).join("") +
        '<div class="pl-thumbs">' + (x.slides || []).map(function (s) { return K.slide({ k: "general", h: x.title, b: s }); }).join("") + "</div>";
      if (x.type === "hold") body += '<p class="o-meta">Program stays as it is until the operator moves on. Nothing is projected.</p>';
      return '<section class="o-pane pl-editor"><div class="o-pane-head"><span class="o-meta">' + icon(TYPES[x.type][0]) + " " + TYPES[x.type][1] + " · " + esc(f.sec.name) + '</span><span class="o-grow"></span>' +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-a="mv" data-v="-1" aria-label="Move up" title="Move up (Alt+↑)">' + icon("arrow-up") + '</button><button class="o-btn o-btn--quiet o-btn--icon" data-a="mv" data-v="1" aria-label="Move down" title="Move down (Alt+↓)">' + icon("arrow-down") + "</button>" +
        '<button class="o-btn o-btn--quiet" data-a="rmitem">' + icon("trash-2") + "Remove</button></div>" + '<div class="o-pane-body pl-ed-body sh-form">' + body + "</div></section>";
    }
    function inspector() {
      var t = tpl();
      return '<section class="o-pane pl-inspector tp-insp" aria-label="Template details"><div class="o-pane-head"><b>Template</b></div><div class="o-pane-body pl-insp-body sh-form">' +
        field("Name", input("t.name", t.name)) + field("Language", '<input class="pl-input" value="' + esc(t.lang) + '" readonly>', "Fixed once created; duplicate to make another language.") +
        field("Purpose", input("t.purpose", t.purpose, "What this template is for")) +
        '<div class="sh-field"><span class="o-label">Recommend for</span>' + checks("t.days", t.days, DAYS) + checks("t.seasons", t.seasons, SEASONS) +
        '<span class="o-meta">Ranks this template when a Celebration matches. Never stops anyone choosing another.</span></div>' +
        '<label class="sh-check"><input type="checkbox" data-a="fb"' + (t.fallback ? " checked" : "") + "> Fallback for " + esc(t.lang) + "</label>" +
        '<span class="o-meta">Used when no template fits better. One per language.</span>' +
        '<div class="tp-check"><div class="o-label">Check against a Celebration</div><select class="pl-input pl-input--sel" data-in="check" aria-label="Sample Celebration"><option value="">Show every item</option>' +
        SAMPLES.map(function (c) { return '<option value="' + c.id + '"' + (S.check === c.id ? " selected" : "") + ">" + esc(c.name) + "</option>"; }).join("") + "</select>" +
        '<span class="o-meta">Marks what the rules would omit for that Celebration. Nothing is saved.</span></div>' +
        '<div class="tp-usage o-meta">' + icon("history") + " Used for " + t.used + " Mass plans. They keep the copy they were made with.</div></div></section>";
    }
    function editorView() {
      var t = tpl();
      return '<div class="sh-page"><header class="sh-edhead"><a class="o-btn o-btn--quiet" href="#" data-a="back">' + icon("arrow-left") + 'Templates</a><div class="o-grow sh-edname"><b class="o-title">' + esc(t.name) + "</b>" +
        '<span class="o-meta">' + esc(t.lang) + " · " + slots(t) + ' slots</span><span class="o-meta pl-saved">' + icon("check") + "Saved</span></div>" +
        '<button class="o-btn" data-a="dup" data-id="' + t.id + '">' + icon("copy") + "Duplicate</button></header>" +
        (t.used ? '<div class="o-notice sh-banner" data-state="info">' + icon("info") + "<div>Changes here apply to Mass plans created from now on. The " + t.used + " plans already made from this template don't change (Decision 78).</div></div>" : "") +
        '<div class="pl-panes">' + outline() + itemEditor() + inspector() + "</div></div>";
    }
    function dialog() {
      var d = S.dialog;
      return '<div class="o-scrim"></div><div class="o-dialog" role="dialog" aria-modal="true" aria-labelledby="tp-d"><div class="o-overlay-head"><h2 class="o-title" id="tp-d">' + esc(d.title) + "</h2></div>" +
        '<div class="o-overlay-body sh-form">' + field("Name", input("dialog.name", d.name, "e.g. Sunday Mass · Tagalog")) +
        (d.kind === "new" || d.kind === "dup" ? field("Language", sel("dialog.lang", d.lang, ["Cebuano", "English", "Tagalog", "Latin"], "Language"), d.kind === "new" ? "Starts with the six Mass sections and nothing in them (Decision 220)." : "A copy in another language keeps the structure; translate its responses afterwards.") : "") +
        (d.clash ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div>A template called <b>" + esc(d.clash) + "</b> already exists. Names must differ.</div></div>" : "") +
        '</div><div class="o-overlay-foot"><button class="o-btn" data-a="cancel">Cancel</button><button class="o-btn o-btn--primary" data-a="ok"' + (d.name.trim() && !d.clash ? "" : " disabled") + ">" + esc(d.ok) + "</button></div></div>";
    }

    function render() {
      var keep = {}, ae = document.activeElement, fk = ae && app.contains(ae) && ae.dataset ? ae.dataset.focusKey : null, caret = fk && ae.selectionStart;
      [".sh-scroll", ".pl-outline .o-pane-body", ".pl-editor .o-pane-body", ".pl-inspector .o-pane-body"].forEach(function (q) { var el = app.querySelector(q); if (el) keep[q] = el.scrollTop; });
      var body = S.view === "editor" && tpl() ? editorView() : listView();
      var toast = S.toast ? '<div class="o-toast" role="status">' + esc(S.toast) + (S.undo ? '<button class="o-btn" data-a="undo">Undo</button>' : "") + "</div>" : "";
      app.innerHTML = '<div class="o-body">' + K.navrail("Masses") + '<div class="o-grow sh-ws">' + body + "</div></div>" + (S.dialog ? dialog() : "") + toast;
      K.icons();
      Object.keys(keep).forEach(function (q) { var el = app.querySelector(q); if (el) el.scrollTop = keep[q]; });
      var f = fk ? app.querySelector('[data-focus-key="' + fk + '"]') : null;
      if (!f && S.renaming) f = app.querySelector('[data-focus-key="secname"]');
      if (!f && S.dialog) f = app.querySelector('[data-focus-key="dialog.name"]');
      if (S.reveal) { var r = app.querySelector('.tp-item[aria-selected="true"]'); if (r) r.scrollIntoView({ block: "nearest" }); S.reveal = false; }
      if (f) { f.focus(); if (caret != null && f.setSelectionRange) try { f.setSelectionRange(caret, caret); } catch (e) { /* not text */ } }
    }
    function nameClash(name, except) { var n = norm(name); var c = S.list.filter(function (t) { return t.id !== except && norm(t.name) === n; })[0]; return c ? c.name : null; }
    function commitRename() {
      var s = sec(S.renaming), v = S.draft.trim(), clash = tpl().secs.some(function (o) { return o !== s && norm(o.name) === norm(v); });
      if (!v || clash) { done(!v ? "A section needs a name" : "Another section is already called " + v); S.renaming = null; return; }
      if (v !== s.name) { snapshot(); s.name = v; edit("Section renamed"); } S.renaming = null;
    }
    function open(id) { S.cur = id; S.view = "editor"; S.sel = null; S.secSel = null; S.menu = null; S.check = ""; }

    app.addEventListener("click", function (e) {
      var t = e.target.closest("[data-a]"); if (!t || t.disabled) return;
      if (t.tagName === "A") e.preventDefault();
      var a = t.dataset.a, id = t.dataset.id, keepToast = false, T = S.view === "editor" ? tpl() : null;
      if (S.renaming && a !== "set") commitRename();
      if (a !== "menu" && a !== "secmenu") S.menu = null;
      if (a !== "add") S.add = false;
      switch (a) {
        case "undo": undo(); keepToast = true; break;
        case "menu": S.menu = S.menu === id ? null : id; break;
        case "arch": S.arch = !S.arch; break;
        case "open": open(id); break;
        case "back": S.view = "list"; break;
        case "new": S.dialog = { kind: "new", title: "New template", name: "", lang: "Cebuano", ok: "Create template" }; break;
        case "dup": var src = S.list.filter(function (x) { return x.id === id; })[0]; S.dialog = { kind: "dup", src: id, title: "Duplicate " + src.name, name: src.name + " copy", lang: src.lang, ok: "Create copy" }; break;
        case "cancel": S.dialog = null; break;
        case "ok": var d = S.dialog; snapshot();
          var nt = d.kind === "new" ? { id: "t" + (nid++), name: d.name.trim(), lang: d.lang, purpose: "", seasons: [], days: [], fallback: !S.list.some(function (x) { return x.lang === d.lang && x.fallback && !x.archived; }), used: 0, edited: "Just now", secs: six() }
            : Object.assign(copy(S.list.filter(function (x) { return x.id === d.src; })[0]), { id: "t" + (nid++), name: d.name.trim(), lang: d.lang, fallback: false, used: 0, edited: "Just now", archived: false });
          S.list.push(nt); S.dialog = null; open(nt.id); done(nt.name + " created", true); keepToast = true; break;
        case "fallback": snapshot(); var ft = S.list.filter(function (x) { return x.id === id; })[0], was = S.list.filter(function (x) { return x.lang === ft.lang && x.fallback; })[0];
          S.list.forEach(function (x) { if (x.lang === ft.lang) x.fallback = x === ft; }); done(ft.name + " is the " + ft.lang + " fallback" + (was ? "; " + was.name + " no longer is" : ""), true); keepToast = true; break;
        case "archive": snapshot(); var at = S.list.filter(function (x) { return x.id === id; })[0]; at.archived = true; var wasFb = at.fallback; at.fallback = false;
          done(at.name + " archived · New Celebrations won't offer it" + (wasFb ? "; " + at.lang + " has no fallback now" : ""), true); keepToast = true; break;
        case "restore": snapshot(); S.list.filter(function (x) { return x.id === id; })[0].archived = false; done("Restored", true); keepToast = true; break;
        // editor
        case "sel": S.sel = id; S.secSel = null; break;
        case "secsel": S.secSel = id; S.sel = null; break;
        case "secmenu": S.menu = S.menu === "sec:" + id ? null : "sec:" + id; break;
        case "rename": S.renaming = id; S.draft = sec(id).name; break;
        case "above": case "below": snapshot(); var i0 = T.secs.indexOf(sec(id)), ns = { id: "n" + (nid++), name: "New section", items: [] }, k = 2;
          while (T.secs.some(function (o) { return norm(o.name) === norm(ns.name); })) ns.name = "New section " + k++;
          T.secs.splice(a === "above" ? i0 : i0 + 1, 0, ns); S.renaming = ns.id; S.draft = ns.name; S.secSel = ns.id; S.sel = null; edit("Section added"); break;
        case "up": case "down": snapshot(); var i1 = T.secs.indexOf(sec(id)), j = i1 + (a === "up" ? -1 : 1), tmp = T.secs[i1]; T.secs[i1] = T.secs[j]; T.secs[j] = tmp; edit("Section moved"); break;
        case "rmsec": snapshot(); T.secs.splice(T.secs.indexOf(sec(id)), 1); S.secSel = null; edit("Section removed"); keepToast = true; break;
        case "add": S.add = !S.add; break;
        case "additem": snapshot(); var target = S.sel ? find(S.sel) : null, tsec = target ? target.sec : S.secSel ? sec(S.secSel) : T.secs[0], v = t.dataset.v;
          var ni = it(v, { song: "Song", reading: "Reading", resp: "Response sequence", general: "Announcement", hold: "Hold" }[v], { need: v === "resp" || v === "hold" ? "Always" : "Required", moment: v === "song" ? "Communion" : "", role: v === "reading" ? "First Reading" : "",
            ex: v === "resp" ? [["P", ""], [null, ""]] : null, slides: v === "general" ? [""] : null });
          tsec.items.splice(target ? target.i + 1 : tsec.items.length, 0, ni); S.sel = ni.id; S.secSel = null; edit(TYPES[v][1] + " added to " + tsec.name); keepToast = true; break;
        case "rmitem": snapshot(); var fr = find(S.sel); fr.sec.items.splice(fr.i, 1); S.sel = null; edit(fr.it.title + " removed"); keepToast = true; break;
        case "mv": moveItem(+t.dataset.v); keepToast = true; break;
        case "set": var x = find(S.sel).it, p = t.dataset.path, val = t.dataset.v; snapshot();
          if (p === "it.need") { x.need = val === "Always" ? "Required" : val; if (val === "Conditional" && !x.rule) x.rule = rule("include", [], []); }
          if (p === "it.rule.mode") x.rule.mode = val === "Omit on" ? "omit" : "include";
          tpl().edited = "Just now"; break;
        case "tick": var on = t.checked, path = t.dataset.path, v2 = t.dataset.v, arr = path === "rule.days" ? find(S.sel).it.rule.days : path === "rule.seasons" ? find(S.sel).it.rule.seasons : path === "t.days" ? T.days : T.seasons;
          snapshot(); if (on && arr.indexOf(v2) < 0) arr.push(v2); if (!on) arr.splice(arr.indexOf(v2), 1); tpl().edited = "Just now"; break;
        case "fb": snapshot(); if (t.checked) { var prev = S.list.filter(function (o) { return o.lang === T.lang && o.fallback && o !== T; })[0]; S.list.forEach(function (o) { if (o.lang === T.lang) o.fallback = o === T; }); done(T.name + " is the " + T.lang + " fallback" + (prev ? "; " + prev.name + " no longer is" : ""), true); keepToast = true; } else T.fallback = false; break;
        case "addex": snapshot(); var xe = find(S.sel).it; xe.ex.push(["P", ""], [null, ""]); break;
      }
      if (!keepToast) { S.toast = null; S.undo = false; }
      render();
    });
    function moveItem(d) {
      var f = find(S.sel), T = tpl(); if (!f) return; snapshot();
      var si = T.secs.indexOf(f.sec), j = f.i + d;
      if (j >= 0 && j < f.sec.items.length) { f.sec.items.splice(f.i, 1); f.sec.items.splice(j, 0, f.it); }
      else { var ns = T.secs[si + d]; if (!ns) { hist.pop(); return; } f.sec.items.splice(f.i, 1); if (d < 0) ns.items.push(f.it); else ns.items.unshift(f.it); done("Moved to " + ns.name); return; }
      tpl().edited = "Just now";
    }
    function onInput(e) {
      var el = e.target, p = el.dataset && el.dataset.in; if (!p) return;
      var v = el.value;
      if (p === "lang") S.lang = v;
      else if (p === "check") S.check = v;
      else if (p === "draft") { S.draft = v; return; }
      else if (p.indexOf("dialog.") === 0) { S.dialog[p.slice(7)] = v; S.dialog.clash = nameClash(S.dialog.name); }
      else if (p === "t.name" || p === "t.purpose") tpl()[p.slice(2)] = v;
      else if (p.indexOf("it.") === 0) find(S.sel).it[p.slice(3)] = v;
      else if (p.indexOf("ex.") === 0) find(S.sel).it.ex[+p.slice(3)][1] = v;
      else if (p.indexOf("slide.") === 0) find(S.sel).it.slides[+p.slice(6)] = v;
      render();
    }
    app.addEventListener("input", function (e) { if (e.target.tagName !== "SELECT") onInput(e); });
    app.addEventListener("change", function (e) { if (e.target.tagName === "SELECT") onInput(e); });
    app.addEventListener("keydown", function (e) {
      if (S.renaming && e.target.dataset && e.target.dataset.in === "draft") {
        if (e.key === "Enter") { e.preventDefault(); commitRename(); render(); } else if (e.key === "Escape") { S.renaming = null; render(); } return;
      }
      if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown") && S.sel) { e.preventDefault(); moveItem(e.key === "ArrowUp" ? -1 : 1); render(); return; }
      if (e.key === "Escape") { if (S.dialog) S.dialog = null; else if (S.menu || S.add) { S.menu = null; S.add = false; } else return; render(); }
      if (e.key === "Enter" && S.dialog && e.target.dataset.focusKey === "dialog.name" && S.dialog.name.trim() && !S.dialog.clash) app.querySelector('[data-a="ok"]').click();
    });

    var api = {
      empty: function () { S.list = []; return api; }, menu: function (id) { S.menu = id; return api; }, archived: function () { S.arch = true; return api; },
      open: function (id) { open(id || "t1"); return api; }, sel: function (title) { tpl().secs.forEach(function (s) { s.items.forEach(function (x) { if (x.title === title) S.sel = x.id; }); }); S.reveal = true; return api; },
      secMenu: function (name) { var s = tpl().secs.filter(function (o) { return o.name === name; })[0]; S.menu = "sec:" + s.id; S.secSel = s.id; return api; },
      add: function () { S.add = true; return api; }, check: function (id) { S.check = id; return api; },
      newDialog: function (name) { S.dialog = { kind: "new", title: "New template", name: name || "", lang: "Tagalog", ok: "Create template" }; S.dialog.clash = nameClash(S.dialog.name); return api; },
      create: function (name, lang) { var nt = { id: "t" + (nid++), name: name, lang: lang, purpose: "", seasons: [], days: [], fallback: true, used: 0, edited: "Just now", secs: six() }; S.list.push(nt); open(nt.id); return api; }
    };
    K.fit(wrapper, app, W, H);
    if (opts.setup) opts.setup(api);
    render();
    return api;
  }

  K.templates = templates;
})();
