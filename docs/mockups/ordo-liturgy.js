// Ordo mockups: reviewing a Liturgy Import Document into a Celebration's Mass plans (load after
// ordo-kit.js). The file is validated, matched to a Celebration and compared entry by entry with
// each Mass plan in its language; nothing changes until Copy. Mockup behaviour only.
(function () {
  "use strict";
  var K = window.OrdoKit, esc = K.esc, icon = K.icon, statusLabel = K.statusLabel;
  function copy(x) { return JSON.parse(JSON.stringify(x)); }

  var DOC = {
    file: "Ika-33-Domingo_2026-11-15_ceb.ordo", made: "Made by a parish helper tool · Tue 27 Oct 2026", lang: "Cebuano",
    date: "Sun 15 Nov 2026", title: "Ika-33 nga Domingo sa Ordinaryong Panahon", season: "Ordinary Time",
    entries: [
      { id: "e1", role: "First Reading", icon: "book-open", title: "Pagbasa gikan sa Libro ni Propeta Daniel", ref: "Daniel 12:1-3",
        text: "Niadtong panahona motungha si Miguel, ang dakong prinsipe nga nagbantay sa imong katawhan. Unya moabot ang panahon sa kalisdanan nga wala pa mahitabo sukad nga may nasod.",
        prompt: "Ang Pulong sa Dios.", resp: "Salamat sa Dios." },
      { id: "e2", role: "Responsorial Psalm", icon: "music-2", title: "Salmo Responsoryo", ref: "Salmo 16:5, 8, 9-10, 11",
        text: "Bantayi ako, O Dios, kay kanimo modangop ako.", verses: ["Ginoo, ikaw ang akong kabilin ug ang akong kopa; ikaw ang naghupot sa akong kapalaran.", "Ako kanunayng nagatan-aw sa Ginoo; tungod kay anaa siya sa akong tuo, dili ako matarug.", "Ikaw magtudlo kanako sa dalan sa kinabuhi; diha kanimo ang kahingpitan sa kalipay."] },
      { id: "e3", role: "Second Reading", icon: "book-open", title: "Pagbasa gikan sa Sulat ngadto sa mga Hebreohanon", ref: "Hebreohanon 10:11-14, 18",
        text: "Ang matag pari nagatindog adlaw-adlaw sa pag-alagad, nga nagahalad sa mao rang mga halad nga dili gayod makapapas sa mga sala.",
        prompt: "Ang Pulong sa Dios.", resp: "Salamat sa Dios." },
      { id: "e4", role: "Gospel Acclamation", icon: "music-2", title: "Aleluya", ref: "Lucas 21:36",
        text: "Aleluya, aleluya. Pagbantay ug pag-ampo kanunay aron makabarug kamo atubangan sa Anak sa Tawo. Aleluya." },
      { id: "e5", role: "Gospel", icon: "book-open", title: "Ebanghelyo sumala kang San Marcos", ref: "Marcos 13:24-32",
        text: "Niadtong mga adlawa, human sa kalisdanan, ang adlaw mongitngit ug ang bulan dili na mohatag sa iyang kahayag.",
        prompt: "Ang Ebanghelyo sa Ginoo.", resp: "Pagdayeg kanimo, Ginoong Jesukristo." },
      { id: "e6", role: "Prayer of the Faithful", icon: "messages-square", title: "Pag-ampo sa Katawhan", ref: "",
        text: "Ginoo, patalinghugi ang among pag-ampo.", verses: ["Alang sa Simbahan, aron magmabinantayon kini sa paghulat sa Ginoo.", "Alang sa mga nagmando sa katilingban, aron magmatinud-anon sila sa pag-alagad.", "Alang sa mga masakiton ug mga nag-antos."] }
    ]
  };
  // What each Mass plan holds for each role today. review: copied by Reuse and marked Needs review;
  // edited: changed by hand in the plan; none: the plan has no item for that role.
  var GA_EDIT = "Aleluya, aleluya. Pagbantay ug pag-ampo kanunay aron makabarug kamo atubangan sa Anak sa Tawo. Aleluya, aleluya.";
  var PLANS = [
    { id: "p1", time: "Sun 15 Nov · 06:00", lang: "Cebuano", choir: "Koro Sta. Cecilia", from: "Reused from Ika-32 nga Domingo",
      have: { e1: { st: "review", ref: "1 Hari 17:10-16" }, e2: { st: "review", ref: "Salmo 146" }, e3: { st: "review", ref: "Hebreohanon 9:24-28" }, e4: { st: "review", ref: "Mateo 5:3" }, e5: { st: "review", ref: "Marcos 12:38-44" }, e6: { st: "same" } } },
    { id: "p2", time: "Sun 15 Nov · 09:00", lang: "Cebuano", choir: "Koro San Roque", from: "From Sunday Mass · Cebuano",
      have: { e1: { st: "empty" }, e2: { st: "empty" }, e3: { st: "empty" }, e4: { st: "edited", ref: "Lucas 21:36", text: GA_EDIT, when: "edited Thu 22 Oct" }, e5: { st: "empty" }, e6: { st: "same" } } },
    { id: "p3", time: "Sun 15 Nov · 17:00", lang: "Cebuano", choir: "Youth Choir", from: "From Weekday Mass · Cebuano",
      have: { e1: { st: "empty" }, e2: { st: "empty" }, e3: { st: "none" }, e4: { st: "empty" }, e5: { st: "empty" }, e6: { st: "none" } } },
    { id: "p4", time: "Sun 15 Nov · 10:30", lang: "English", choir: "Youth Choir", from: "From Sunday Mass · English", have: {} }
  ];
  var CELS = [
    { id: "c4", title: "Ika-33 nga Domingo sa Ordinaryong Panahon", date: "Sun 15 Nov 2026" },
    { id: "c3", title: "Ika-32 nga Domingo sa Ordinaryong Panahon", date: "Sun 8 Nov 2026" },
    { id: "c8", title: "Christ the King", date: "Sun 22 Nov 2026" }
  ];

  // Outcome of one entry for one plan (Decisions 188, 223; proposed Decision 289).
  function outcome(p, e) {
    var h = p.have[e.id] || { st: "empty" };
    return { empty: "fill", review: "replace", same: "same", edited: "decide", none: "noslot" }[h.st];
  }
  var OUT = {
    fill: ["info", "New", "Fills an empty item"], replace: ["live", "Replaces reused text", "Copied by Reuse and marked Needs review"], same: ["", "Unchanged", "Already the same"],
    decide: ["warning", "Needs decision", "Changed by hand in this plan"], noslot: ["warning", "Needs decision", "This plan has no item for it"]
  };
  // A tiny word diff for the side-by-side comparison.
  function diff(a, b) {
    var x = a.split(" "), y = b.split(" "), n = x.length, m = y.length, L = [], i, j;
    for (i = 0; i <= n; i++) { L.push([]); for (j = 0; j <= m; j++) L[i].push(0); }
    for (i = n - 1; i >= 0; i--) for (j = m - 1; j >= 0; j--) L[i][j] = x[i] === y[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1]);
    var A = [], B = []; i = 0; j = 0;
    while (i < n && j < m) { if (x[i] === y[j]) { A.push(esc(x[i])); B.push(esc(y[j])); i++; j++; } else if (L[i + 1][j] >= L[i][j + 1]) A.push("<del>" + esc(x[i++]) + "</del>"); else B.push("<ins>" + esc(y[j++]) + "</ins>"); }
    while (i < n) A.push("<del>" + esc(x[i++]) + "</del>"); while (j < m) B.push("<ins>" + esc(y[j++]) + "</ins>");
    return [A.join(" "), B.join(" ")];
  }

  function liturgy(wrapper, opts) {
    opts = opts || {};
    var W = opts.w || 1366, H = opts.h || 768;
    var app = document.createElement("div");
    app.className = "o-app li-app"; app.tabIndex = -1;
    wrapper.classList.add("o-fit"); wrapper.appendChild(app);

    var S = { view: "review", doc: copy(DOC), plans: copy(PLANS), cel: "c4", pick: { p1: true, p2: true, p3: true }, sel: "e1", D: {}, celMenu: false, toast: null, tech: false };
    function targets() { return S.plans.filter(function (p) { return p.lang === S.doc.lang && S.pick[p.id]; }); }
    function open(p, e) { var o = outcome(p, e); return (o === "decide" || o === "noslot") && !S.D[p.id + e.id]; }
    function unresolved() { var n = 0; targets().forEach(function (p) { S.doc.entries.forEach(function (e) { if (open(p, e)) n++; }); }); return n; }
    function counts() {
      var c = { fill: 0, replace: 0, same: 0, decide: 0, skip: 0 };
      targets().forEach(function (p) { S.doc.entries.forEach(function (e) {
        var o = outcome(p, e), d = S.D[p.id + e.id];
        if (o === "decide" || o === "noslot") { if (!d) c.decide++; else if (d === "keep" || d === "skip") c.skip++; else c[o === "noslot" ? "fill" : "replace"]++; } else c[o]++;
      }); });
      return c;
    }
    function cel() { return CELS.filter(function (c) { return c.id === S.cel; })[0]; }

    function head(title, meta, action) {
      return '<header class="sh-edhead"><button class="o-btn o-btn--quiet" data-a="cancel">' + icon("x") + 'Cancel import</button><div class="o-grow sh-edname"><b class="o-title">' + title + "</b>" +
        '<span class="o-meta">' + meta + "</span></div>" + (action || "") + "</header>";
    }
    function entryList() {
      return S.doc.entries.map(function (e) {
        var ps = targets(), n = ps.filter(function (p) { return open(p, e); }).length, same = ps.every(function (p) { return outcome(p, e) === "same"; });
        return '<button class="o-row pl-item li-entry" data-a="sel" data-id="' + e.id + '"' + (S.sel === e.id ? ' aria-selected="true"' : "") + ">" + icon(e.icon, "tp-ic") +
          '<span class="o-grow"><span class="li-role">' + esc(e.role) + '</span><span class="o-meta">' + esc(e.ref || e.text.slice(0, 32) + "…") + "</span></span>" +
          (n ? statusLabel("warning", n + " to decide", "square") : same ? '<span class="o-meta">Unchanged</span>' : statusLabel("live", "Ready")) + "</button>";
      }).join("");
    }
    function celCard() {
      var c = cel();
      return '<div class="li-cel">' + (c.id === "c4" ? statusLabel("live", "Matches by date") : statusLabel("warning", "Chosen by hand", "square")) +
        "<b>" + esc(c.title) + '</b><span class="o-meta">' + esc(c.date) + "</span>" +
        (c.date !== S.doc.date ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div>The file is for <b>" + esc(S.doc.date) + "</b>. Its readings may not suit this date.</div></div>" : "") +
        '<span class="sh-rel"><button class="o-btn o-btn--quiet" data-a="celmenu">Change Celebration</button>' + (S.celMenu ? '<div class="o-menu sh-menu li-celmenu" role="menu">' +
          CELS.map(function (x) { return '<button class="o-row" role="menuitem" data-a="setcel" data-id="' + x.id + '">' + esc(x.title) + '<span class="o-meta">' + esc(x.date) + "</span></button>"; }).join("") + "</div>" : "") + "</span></div>";
    }
    function preview(e) {
      var slides = e.role === "Responsorial Psalm" ? [{ k: "psalm", b: e.text, f: e.ref }] : e.role === "Gospel Acclamation" ? [{ k: "response", h: "TUBAG:", b: e.text }] :
        e.role === "Prayer of the Faithful" ? [{ k: "response", h: "TUBAG:", b: e.text }] : [{ k: "reading-id", b: e.title, f: e.ref }, { k: "response", h: "TUBAG:", b: e.resp }];
      return '<div class="li-doc"><div class="li-dochead"><b>' + esc(e.title) + "</b>" + (e.ref ? '<span class="o-meta">' + esc(e.ref) + "</span>" : "") + "</div>" +
        '<p class="li-text">' + esc(e.text) + "</p>" + (e.verses ? '<ol class="li-verses">' + e.verses.map(function (v) { return "<li>" + esc(v) + "</li>"; }).join("") + "</ol>" : "") +
        (e.prompt ? '<p class="o-meta">Closing: ' + esc(e.prompt) + " · " + esc(e.resp) + "</p>" : "") + '<div class="pl-thumbs">' + slides.map(function (s) { return K.slide(s); }).join("") + "</div></div>";
    }
    function planRows(e) {
      return S.plans.map(function (p) {
        if (p.lang !== S.doc.lang) return '<div class="o-row sh-trow li-plan is-off"><div class="o-grow"><b>' + esc(p.time) + " · " + esc(p.lang) + '</b><span class="o-meta">Not offered: the file is ' + esc(S.doc.lang) + ". Readings in each language are entered on their own (Decision 80).</span></div></div>";
        if (!S.pick[p.id]) return '<div class="o-row sh-trow li-plan is-off"><div class="o-grow"><b>' + esc(p.time) + '</b><span class="o-meta">Left out of this import</span></div></div>';
        var o = outcome(p, e), h = p.have[e.id] || {}, k = p.id + e.id, d = S.D[k], lab = OUT[o];
        var label = d ? statusLabel("live", { use: "Decided · use the file’s text", keep: "Decided · keep this plan’s", add: "Decided · add the item", skip: "Decided · skip" }[d]) : lab[0] ? statusLabel(lab[0], lab[1], lab[0] === "warning" ? "square" : null) : '<span class="o-meta">' + lab[1] + "</span>";
        var body = "";
        if (o === "replace") body = '<span class="o-meta">Now ' + esc(h.ref) + " from the reused plan, marked Needs review. The file’s text replaces it and clears that.</span>";
        if (o === "decide") {
          var dd = diff(h.text, e.text);
          body = '<div class="li-cmp"><div><div class="o-label">In this plan · ' + esc(h.when) + '</div><p class="li-text">' + dd[0] + '</p></div><div><div class="o-label">In the file</div><p class="li-text">' + dd[1] + "</p></div></div>" +
            '<div class="o-segmented" role="group" aria-label="Choose">' + [["keep", "Keep this plan’s"], ["use", "Use the file’s"]].map(function (c) { return '<button data-a="decide" data-id="' + k + '" data-v="' + c[0] + '" aria-pressed="' + (d === c[0]) + '">' + c[1] + "</button>"; }).join("") + "</div>";
        }
        if (o === "noslot") body = '<span class="o-meta">' + esc(p.from) + " has no " + esc(e.role) + " item.</span>" +
          '<div class="o-segmented" role="group" aria-label="Choose">' + [["add", "Add it to Liturgy of the Word"], ["skip", "Skip for this Mass"]].map(function (c) { return '<button data-a="decide" data-id="' + k + '" data-v="' + c[0] + '" aria-pressed="' + (d === c[0]) + '">' + c[1] + "</button>"; }).join("") + "</div>";
        return '<div class="o-row sh-trow li-plan' + (o === "decide" || o === "noslot" ? " is-open" : "") + '"><div class="li-planhead"><b>' + esc(p.time) + '</b><span class="o-meta">' + esc(p.choir) + " · " + esc(p.from) + "</span>" + label + "</div>" + body + "</div>";
      }).join("");
    }
    function side() {
      var c = counts(), left = unresolved();
      return '<section class="o-pane pl-inspector tp-insp" aria-label="Copy into"><div class="o-pane-head"><b>Copy into</b></div><div class="o-pane-body pl-insp-body sh-form">' +
        '<div class="sh-list">' + S.plans.map(function (p) {
          var off = p.lang !== S.doc.lang;
          return '<label class="o-row sh-trow sh-pick' + (off ? " is-off" : "") + '"><input type="checkbox" data-a="pick" data-id="' + p.id + '"' + (S.pick[p.id] && !off ? " checked" : "") + (off ? " disabled" : "") + '><div class="o-grow"><b>' + esc(p.time.slice(-5)) + " · " + esc(p.lang) + '</b><span class="o-meta">' + (off ? "Different language" : esc(p.choir)) + "</span></div></label>";
        }).join("") + "</div>" +
        '<div class="li-counts">' + [["info", c.fill, "new"], ["live", c.replace, "replace reused text"], ["", c.same, "unchanged"], ["", c.skip, "kept or skipped"], ["warning", c.decide, "need a decision"]].map(function (r) {
          return '<div class="li-count"><b>' + r[1] + "</b>" + (r[0] ? statusLabel(r[0], r[2], r[0] === "warning" ? "square" : null) : '<span class="o-meta">' + r[2] + "</span>") + "</div>"; }).join("") + "</div>" +
        (left ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div><b>" + left + " decision" + (left === 1 ? "" : "s") + " before copying.</b> Nothing changes until then.</div></div>" : "") +
        '<p class="o-meta">The copied text becomes ordinary content in each plan, editable there. The plan’s History notes that it came from ' + esc(S.doc.file) + ".</p></div></section>";
    }
    function review() {
      var e = S.doc.entries.filter(function (x) { return x.id === S.sel; })[0], n = targets().length, left = unresolved();
      var act = '<button class="o-btn o-btn--primary" data-a="commit"' + (left || !n ? " disabled" : "") + ">Copy into " + n + " Mass plan" + (n === 1 ? "" : "s") + "</button>";
      return '<div class="sh-page">' + head("Review liturgy import", esc(S.doc.file) + " · " + esc(S.doc.lang) + " · " + esc(S.doc.made), act) +
        '<div class="pl-panes"><section class="o-pane pl-outline li-outline" aria-label="From the file"><div class="o-pane-head"><b>From the file</b></div><div class="o-pane-body">' + celCard() +
        '<div class="o-section-label pl-sec">' + S.doc.entries.length + " entries</div>" + entryList() + "</div></section>" +
        '<section class="o-pane pl-editor"><div class="o-pane-head"><span class="o-meta">' + icon(e.icon) + " " + esc(e.role) + '</span></div><div class="o-pane-body pl-ed-body">' + preview(e) +
        '<div class="o-label li-sub">In each Mass plan</div><div class="sh-list">' + planRows(e) + "</div></div></section>" + side() + "</div></div>";
    }
    function done() {
      var ps = targets(), c = counts();
      return '<div class="sh-page">' + '<header class="sh-edhead"><a class="o-btn o-btn--quiet" href="masses-index.html">' + icon("arrow-left") + 'Masses</a><div class="o-grow sh-edname"><b class="o-title">Liturgy copied</b><span class="o-meta">' + esc(cel().title) + "</span></div></header>" +
        '<div class="sh-scroll"><div class="sh-col"><div class="o-notice" data-state="info">' + icon("check") + "<div><b>" + (c.fill + c.replace) + " items copied into " + ps.length + " Mass plans.</b> Reused readings are no longer marked Needs review. Undo is in each plan’s History.</div></div>" +
        '<div class="sh-list">' + ps.map(function (p) {
          var ready = p.id !== "p3" || S.D["p3e3"] === "add" ? statusLabel("live", "Readings ready") : statusLabel("warning", "Ready with warnings · no Second Reading", "square");
          return '<div class="o-row sh-trow"><div class="o-grow"><b>' + esc(p.time) + " · " + esc(p.lang) + '</b><span class="o-meta">' + esc(p.choir) + "</span></div>" + ready + '<a class="o-btn" href="mass-planning-flows.html">Open in Mass Planner</a></div>';
        }).join("") + "</div></div></div></div>";
    }
    function invalid(kind) {
      var newer = kind === "newer";
      return '<div class="sh-page">' + head("Review liturgy import", newer ? "Ika-34-Domingo_2026-11-22_ceb.ordo" : "Readings-Nov22.ordo") +
        '<div class="sh-scroll"><div class="sh-col"><section class="sh-card li-bad"><h2 class="sh-sec">' + icon(newer ? "circle-arrow-up" : "file-x") + (newer ? "Made by a newer Ordo" : "This file can’t be imported") + "</h2>" +
        (newer ? "<p>It uses liturgy-import format 3; this Ordo reads up to format 2. Update Ordo on this computer, or ask for the file in the older format. Nothing was changed.</p>" :
          '<p>' + statusLabel("danger", "Invalid", "diamond") + " The Gospel entry has no text, and the file’s checksum doesn’t match its contents. Ordo won’t import part of a liturgy file. Nothing was changed.</p>") +
        '<div class="sh-inline"><button class="o-btn o-btn--primary" data-a="another">Choose another file</button><button class="o-btn" data-a="tech" aria-expanded="' + S.tech + '">View technical details</button></div>' +
        (S.tech ? '<pre class="li-tech">' + (newer ? "kind: liturgy-import\nschemaVersion: 3 (supported: 1-2)\nproducer: ordo-liturgy-tools 1.4.0" : "kind: liturgy-import\nschemaVersion: 2\nentries[4].text: required, missing\nmanifest sha256: 9c1f…e0 ≠ payload sha256: 51aa…7d") + "</pre>" : "") +
        '<p class="o-meta">Readings can still be typed into each Mass plan by hand.</p></section></div></div></div>';
    }
    function nolang() {
      return '<div class="sh-page">' + head("Review liturgy import", "Ika-33-Domingo_2026-11-15_tgl.ordo · Tagalog") +
        '<div class="sh-scroll"><div class="sh-col"><section class="sh-card"><h2 class="sh-sec">' + icon("languages") + "No Tagalog Mass plan to copy into</h2>" +
        "<p>The file is for <b>Ika-33 nga Domingo · Sun 15 Nov 2026</b>, which has Cebuano and English plans. Readings only go into a plan in the same language.</p>" +
        '<div class="sh-inline"><a class="o-btn o-btn--primary" href="masses-index.html">' + icon("plus") + 'Add a Tagalog Mass first</a><button class="o-btn" data-a="cancel">Cancel import</button></div>' +
        '<p class="o-meta">After adding it, open the file again; nothing from it is kept until then.</p></section></div></div></div>';
    }

    function render() {
      var keep = {}; [".pl-ed-body", ".li-outline .o-pane-body"].forEach(function (q) { var el = app.querySelector(q); if (el) keep[q] = el.scrollTop; });
      var body = S.view === "done" ? done() : S.view === "invalid" || S.view === "newer" ? invalid(S.view) : S.view === "nolang" ? nolang() : review();
      app.innerHTML = '<div class="o-body">' + K.navrail("Masses") + '<div class="o-grow sh-ws">' + body + "</div></div>" + (S.toast ? '<div class="o-toast" role="status">' + esc(S.toast) + "</div>" : "");
      K.icons();
      Object.keys(keep).forEach(function (q) { var el = app.querySelector(q); if (el) el.scrollTop = keep[q]; });
    }
    app.addEventListener("click", function (e) {
      var t = e.target.closest("[data-a]"); if (!t || t.disabled) return;
      var a = t.dataset.a, id = t.dataset.id; S.toast = null;
      if (a !== "celmenu") S.celMenu = false;
      switch (a) {
        case "sel": S.sel = id; break;
        case "celmenu": S.celMenu = !S.celMenu; break;
        case "setcel": S.cel = id; break;
        case "pick": S.pick[id] = !S.pick[id]; break;
        case "decide": S.D[id] = S.D[id] === t.dataset.v ? undefined : t.dataset.v; break;
        case "commit": S.view = "done"; break;
        case "cancel": S.toast = "Import cancelled. Nothing was changed."; break;
        case "another": S.toast = "Opens the file picker for .ordo files"; break;
        case "tech": S.tech = !S.tech; break;
      }
      render();
    });
    app.addEventListener("keydown", function (e) { if (e.key === "Escape" && S.celMenu) { S.celMenu = false; render(); } });

    var api = {
      sel: function (id) { S.sel = id; return api; }, decide: function (k, v) { S.D[k] = v; return api; }, celMenu: function () { S.celMenu = true; return api; },
      cel: function (id) { S.cel = id; return api; }, resolveAll: function () { S.D = { p2e4: "use", p3e3: "add", p3e6: "add" }; return api; },
      done: function () { api.resolveAll(); S.view = "done"; return api; }, invalid: function (tech) { S.view = "invalid"; S.tech = !!tech; return api; },
      newer: function () { S.view = "newer"; return api; }, nolang: function () { S.view = "nolang"; return api; }
    };
    K.fit(wrapper, app, W, H);
    if (opts.setup) opts.setup(api);
    render();
    return api;
  }
  K.liturgy = liturgy;
})();
