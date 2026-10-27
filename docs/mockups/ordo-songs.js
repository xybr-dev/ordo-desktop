// Ordo mockups: the Library's Songs view, Arrangement editor, History and Song import on the
// shared kit (load after ordo-kit.js). Each instance owns a working copy of the sample Library,
// so its flows can edit, restore and Undo without touching other mockups. Mockup behaviour only.
(function () {
  "use strict";
  var K = window.OrdoKit, esc = K.esc, icon = K.icon, statusLabel = K.statusLabel;
  function copy(x) { return JSON.parse(JSON.stringify(x)); }
  function norm(s) { return String(s || "").toLowerCase().replace(/\s+/g, " ").trim(); }

  var CHOIRS = ["Koro San Roque", "Youth Choir", "Koro Sta. Cecilia"];
  var LANGS = ["Cebuano", "English", "Tagalog", "Latin"];
  var OCCASIONS = ["Ordinary Time", "Advent", "Christmas", "Lent", "Easter", "Special"];

  // ── Library fixture ─────────────────────────────────────────────────────────────────────────
  function sec(id, name, text, o) { o = o || {}; return { id: id, name: name, text: text, approved: o.approved !== false }; }
  function arr(id, o) {
    return { id: id, title: o.title || "", langs: o.langs, choirs: o.choirs || [], occ: o.occ || [], translator: o.translator || "", notes: o.notes || "",
      sections: o.sections, seq: o.seq, updated: o.updated || "3 weeks ago", archived: !!o.archived };
  }
  var KYRIE_CEB = [sec("k1", "Ginoo", "Ginoo, kaloy-i kami.\nGinoo, kaloy-i kami."), sec("k2", "Kristo", "Kristo, kaloy-i kami.\nKristo, kaloy-i kami.")];
  var SONGS = [
    { id: "s1", title: "Ginoo, Kaloy-i Kami", by: "Misa Bisaya · R. Villanueva", aliases: ["Kyrie · Misa Bisaya"], moments: ["Penitential Act"], notes: "", arrs: [
      arr("a1", { langs: ["Cebuano"], choirs: ["Koro San Roque", "Youth Choir"], occ: ["Ordinary Time", "Lent"], sections: KYRIE_CEB, seq: ["k1", "k2", "k1"], updated: "2 days ago" }),
      arr("a2", { langs: ["English"], choirs: ["Youth Choir"], translator: "Parish adaptation", occ: ["Ordinary Time"], updated: "3 months ago",
        sections: [sec("e1", "Lord", "Lord, have mercy.\nLord, have mercy."), sec("e2", "Christ", "Christ, have mercy.\nChrist, have mercy.")], seq: ["e1", "e2", "e1"] })] },
    { id: "s2", title: "Daygon Ta", by: "R. Villanueva", aliases: [], moments: ["Entrance"], notes: "", arrs: [
      arr("a3", { langs: ["Cebuano"], choirs: ["Koro San Roque"], occ: ["Ordinary Time"], updated: "1 week ago", sections: [
        sec("d1", "Bersikulo 1", "Daygon ta ang Ginoo\nsa tanang kasingkasing\n\nUg awiton ta ang iyang\nmaayong kaluoy"),
        sec("d2", "Koro", "Daygon, daygon ta\nang ngalan sa Ginoo"),
        sec("d3", "Bersikulo 2", "Ang iyang gugma\nwala nay katapusan\n\nAng iyang pagkamatinud-anon\nalang sa tanang kaliwatan"),
        sec("d4", "Katapusan", "Amen, amen,\ndaygon ta ang Ginoo")], seq: ["d1", "d2", "d3", "d2"] }),
      arr("a4", { langs: ["Cebuano"], choirs: ["Youth Choir"], updated: "5 months ago", sections: [
        sec("y1", "Bersikulo 1", "Daygon ta ang Ginoo\nsa tanang kasingkasing"), sec("y2", "Koro", "Daygon, daygon ta\nang ngalan sa Ginoo")], seq: ["y1", "y2"] })] },
    { id: "s3", title: "Ihalad Namo", by: "J. Bacalso", aliases: ["Offertory · Bacalso"], moments: ["Preparation of the Gifts"], notes: "", arrs: [
      arr("a5", { langs: ["Cebuano"], choirs: ["Koro San Roque"], updated: "1 month ago", sections: [
        sec("i1", "Bersikulo 1", "Ihalad namo kining pan ug bino\nsa imong halaran"), sec("i2", "Koro", "Dawata, O Ginoo,\nang among halad"),
        sec("i3", "Bersikulo 2", "Ihalad usab namo\nang among kinabuhi")], seq: ["i1", "i2", "i3", "i2"] }),
      arr("a6", { title: "Scanned score (2019)", langs: ["Cebuano"], choirs: ["Koro San Roque"], archived: true, updated: "2 years ago", sections: [sec("x1", "Koro", "Dawata, O Ginoo")], seq: ["x1"] })] },
    { id: "s4", title: "Pan sa Kinabuhi", by: "Koro San Roque", aliases: ["Bread of Life"], moments: ["Communion"], notes: "", arrs: [
      arr("a7", { langs: ["Cebuano"], choirs: ["Koro San Roque"], occ: ["Ordinary Time", "Easter"], updated: "yesterday", sections: [
        sec("p1", "Bersikulo 1", "Ako ang pan sa kinabuhi\nang moduol kanako dili gutomon\n\nAng mosalig kanako\ndili na gayud uhawon"),
        sec("p2", "Koro", "Pan sa kinabuhi,\nihatag kanamo karon", { approved: false }),
        sec("p3", "Bersikulo 2", "Kadtong mokaon niining pan\nmagkinabuhi sa kahangturan\nug banhawon ko siya\nsa katapusang adlaw\nmao kini ang akong saad", { approved: false })], seq: ["p1", "p2", "p3", "p2"] })] },
    { id: "s5", title: "Halad sa Kasingkasing", by: "M. Dela Cruz", aliases: [], moments: ["Preparation of the Gifts"], notes: "", arrs: [
      arr("a8", { langs: ["Cebuano"], choirs: ["Koro Sta. Cecilia"], sections: [sec("h1", "Bersikulo 1", "Kining kasingkasing\namo nang ihalad"), sec("h2", "Koro", "Dawata kami, Ginoo,\nsa imong gugma")], seq: ["h1", "h2"] })] },
    { id: "s6", title: "Santos", by: "Misa Bisaya · R. Villanueva", aliases: ["Sanctus · Misa Bisaya"], moments: ["Eucharistic Prayer"], notes: "", arrs: [
      arr("a9", { langs: ["Cebuano"], choirs: ["Koro San Roque", "Youth Choir"], sections: [sec("t1", "Santos", "Santos, santos, santos\nGinoong Dios sa kalangitan")], seq: ["t1"] })] },
    { id: "s7", title: "Ang Ginoo Akong Magbalantay", by: "Salmo 23 · traditional setting", aliases: ["Salmo 23", "The Lord is my shepherd"], moments: ["Communion"], notes: "", arrs: [
      arr("a10", { langs: ["Cebuano"], choirs: [], sections: [sec("m1", "Koro", "Ang Ginoo akong magbalantay,\ndili ako kulangon")], seq: [] })] },
    { id: "s8", title: "Amahan Namo", by: "Traditional", aliases: ["Our Father", "Pater Noster"], moments: ["Communion Rite"], notes: "", arrs: [
      arr("a11", { langs: ["Cebuano"], choirs: ["Koro San Roque"], sections: [sec("o1", "Amahan Namo", "Amahan namo, nga anaa sa mga langit,\npagdaygon ang imong ngalan")], seq: ["o1"] })] },
    { id: "s9", title: "Kordero sa Dios · Daan", by: "Unknown", aliases: [], moments: ["Fraction Rite"], notes: "Replaced by the Misa Bisaya setting.", archived: true, arrs: [
      arr("a12", { langs: ["Cebuano"], choirs: [], sections: [sec("q1", "Kordero", "Kordero sa Dios")], seq: ["q1"] })] }
  ];

  // ── derived Arrangement facts ───────────────────────────────────────────────────────────────
  function groups(text) { return text.split(/\n\s*\n/).map(function (g) { return g.split("\n").filter(function (l) { return l.trim(); }); }).filter(function (g) { return g.length; }); }
  function slides(s) { return groups(s.text).map(function (lines) { return { k: "lyrics", h: s.name, b: lines.join("\n"), fits: lines.length <= 4, lines: lines.length }; }); }
  function secOf(a, id) { for (var i = 0; i < a.sections.length; i++) if (a.sections[i].id === id) return a.sections[i]; return null; }
  function uses(a, id) { return a.seq.filter(function (x) { return x === id; }).length; }
  function slideCount(a) { return a.seq.reduce(function (n, id) { return n + slides(secOf(a, id)).length; }, 0); }
  function arrLabel(a) { return a.title || a.langs.join(" + ") + (a.choirs.length ? " · " + a.choirs[0] + (a.choirs.length > 1 ? " +" + (a.choirs.length - 1) : "") : ""); }
  function readiness(a) {
    if (!a.sections.length) return { state: "draft", text: "No sections yet" };
    if (!a.seq.length) return { state: "warning", text: "Incomplete", why: "No performance sequence yet" };
    var used = a.sections.filter(function (s) { return uses(a, s.id); });
    if (used.some(function (s) { return !slides(s).length; })) return { state: "warning", text: "Incomplete", why: "A section in the Sequence has no lyrics" };
    var review = used.filter(function (s) { return !s.approved || slides(s).some(function (x) { return !x.fits; }); });
    if (review.length) return { state: "info", text: "Needs review", why: review.map(function (s) { return s.name; }).join(", ") + (review.length === 1 ? " needs" : " need") + " pagination review" };
    return { state: "live", text: "Ready" };
  }
  function status(r) { return r.state === "draft" ? '<span class="o-meta">' + esc(r.text) + "</span>" : statusLabel(r.state, r.text, r.state === "warning" ? "diamond" : null); }

  // ── History fixture (Decisions 140–143) ─────────────────────────────────────────────────────
  function arrHistory(a) {
    var before = copy(a), k = before.sections[1] || before.sections[0];
    if (k) k.text = k.text.replace(/kaloy-i kami\.$/, "malooy ka kanamo.").replace(/\n\n/, "\n");
    before.seq = before.seq.slice(0, Math.max(1, before.seq.length - 1));
    var older = copy(before); older.sections.forEach(function (s) { s.approved = false; });
    return [
      { when: "Today · 10:37–10:44", what: "Edited lyrics · 8 changes", how: "Manual edit", revs: [
        { at: "10:44", what: "Repeated " + (a.sections[0] ? a.sections[0].name : "a section") + " at the end", snap: copy(a), current: true },
        { at: "10:42", what: "Corrected " + (k ? k.name : "refrain") + " wording", snap: before },
        { at: "10:39", what: "Adjusted a slide boundary", snap: before },
        { at: "10:37", what: "Approved pagination", snap: before }] },
      { when: "Yesterday · 16:12", what: "Updated from import", how: "Import · whole-Arrangement replacement", revs: [{ at: "16:12", what: "Replaced from koro-san-roque.ordo", snap: older }] },
      { when: "14 Aug 2026 · 09:08", what: "Created arrangement", how: "Creation", revs: [{ at: "09:08", what: "Created with " + a.langs.join(", "), snap: older }] }
    ];
  }
  function songHistory(s) {
    var before = copy(s); before.aliases = before.aliases.slice(0, Math.max(0, before.aliases.length - 1));
    return [
      { when: "Today · 09:15", what: "Edited details · 2 changes", how: "Manual edit", revs: [
        { at: "09:15", what: "Added an alias", snap: copy(s), current: true }, { at: "09:14", what: "Set usual moment", snap: before }] },
      { when: "14 Aug 2026 · 09:05", what: "Created song", how: "Creation", revs: [{ at: "09:05", what: "Created " + s.title, snap: before }] }
    ];
  }

  // ── Song import fixture (Decisions 135–137) ─────────────────────────────────────────────────
  var DOCS = [
    { file: "daygon-ta-sta-cecilia.ordo", title: "Daygon Ta", by: "R. Villanueva", match: "s2", why: "Title, composer and opening line match Daygon Ta.", arrs: [
      { label: "Cebuano · Koro Sta. Cecilia", kind: "attach", evidence: "Choir Koro Sta. Cecilia has no Arrangement of this Song; lyrics and Sequence differ from both existing Arrangements.",
        a: arr("n1", { langs: ["Cebuano"], choirs: ["Koro Sta. Cecilia"], sections: [sec("c1", "Bersikulo 1", "Daygon ta ang Ginoo\nsa tanang kasingkasing"), sec("c2", "Koro", "Daygon ta, daygon ta\nang Ginoo karon")], seq: ["c1", "c2", "c2"] }) }] },
    { file: "pan-sa-kinabuhi-koro.ordo", title: "Pan sa Kinabuhi", by: "Koro San Roque", match: "s4", why: "Title and composer match; the alias Bread of Life also matches.", arrs: [
      { label: "Cebuano · Koro San Roque", kind: "decide", near: "a7", evidence: "Resembles the existing Koro San Roque Arrangement: Bersikulo 2 is reworded, a Bersikulo 3 is added, and the Sequence ends differently.",
        a: arr("n2", { langs: ["Cebuano"], choirs: ["Koro San Roque"], sections: [
          sec("p1", "Bersikulo 1", "Ako ang pan sa kinabuhi\nang moduol kanako dili gutomon\n\nAng mosalig kanako\ndili na gayud uhawon"),
          sec("p2", "Koro", "Pan sa kinabuhi,\nihatag kanamo karon"),
          sec("p3", "Bersikulo 2", "Kadtong mokaon niining pan\nmagkinabuhi sa kahangturan\n\nUg banhawon ko siya\nsa katapusang adlaw"),
          sec("p4", "Bersikulo 3", "Ang akong unod tinuod nga pagkaon\nang akong dugo tinuod nga ilimnon")], seq: ["p1", "p2", "p3", "p2", "p4", "p2"] }) }] },
    { file: "ginoo-kaloy-i-kami.ordo", title: "Ginoo, Kaloy-i Kami", by: "Misa Bisaya · R. Villanueva", match: "s1", why: "Title, composer and alias match.", arrs: [
      { label: "Cebuano · Koro San Roque +1", kind: "skip", evidence: "Identical to the existing Cebuano Arrangement: lyrics, Sequence and pagination all match.", a: arr("n3", { langs: ["Cebuano"], choirs: ["Koro San Roque", "Youth Choir"], sections: KYRIE_CEB, seq: ["k1", "k2", "k1"] }) }] },
    { file: "maghimaya-kita.ordo", title: "Maghimaya Kita", by: "A. Tudtud", match: null, why: "No Song in the Library matches the title, composer, aliases or opening line.", arrs: [
      { label: "Cebuano · Youth Choir", kind: "new", evidence: "Creates the Song work and its first Arrangement.", a: arr("n4", { langs: ["Cebuano"], choirs: ["Youth Choir"], sections: [sec("g1", "Koro", "Maghimaya kita\nsa Ginoong Dios")], seq: ["g1"] }) }] },
    { file: "amahan-namo-latin.ordo", title: "Amahan Namo", by: "Traditional", match: "s8", why: "The alias Pater Noster matches this Song.", arrs: [
      { label: "Latin · Koro San Roque", kind: "attach", evidence: "A new language rendition of the same composition (Decision 88).", a: arr("n5", { langs: ["Latin"], choirs: ["Koro San Roque"], sections: [sec("l1", "Pater Noster", "Pater noster, qui es in caelis,\nsanctificetur nomen tuum")], seq: ["l1"] }) }] },
    { file: "santos-koro-e.ordo", title: "Santos", by: "Misa Bisaya", invalid: "Line 41: the Sequence refers to section “Hosanna 2”, which the document doesn't define. The whole document is excluded.", arrs: [] }
  ];
  var KIND = { attach: ["info", "Attach Arrangement"], decide: ["warning", "Needs decision"], skip: ["muted", "Skip unchanged"], new: ["live", "Add new Song"] };
  var OUTCOMES = { attach: [["attach", "Attach Arrangement"], ["skip", "Skip document"]], new: [["new", "Add new Song"], ["skip", "Skip document"]], skip: [["skip", "Skip unchanged"], ["attach", "Attach as another Arrangement"]],
    decide: [["", "Choose an outcome…"], ["add", "Add as new Arrangement"], ["update", "Update existing Arrangement"], ["skip", "Skip document"]] };

  // opts: w, h, view ("songs"|"editor"|"import"), song, arr, tab, setup(api), afterRender(api)
  function library(wrapper, opts) {
    opts = opts || {};
    var W = opts.w || 1366, H = opts.h || 768;
    var app = document.createElement("div");
    app.className = "o-app sg-app"; app.tabIndex = -1;
    wrapper.classList.add("o-fit"); wrapper.appendChild(app);

    var S = { songs: copy(SONGS), view: opts.view || "songs", q: "", f: { choir: "", lang: "", occ: "" }, archived: false,
      song: opts.song || "s1", arr: opts.arr || null, tab: opts.tab || "lyrics", sec: null, slide: 0, occ: 0, open: {},
      overlay: null, child: null, form: null, dupOpen: false, dup: null, popover: null, hist: null, rev: null, cmpTab: "lyrics", confirm: false,
      toast: null, undo: null, focus: null, from: null,
      docs: copy(DOCS), doc: 1, staged: {}, cmpDoc: null, scale: 1 };
    function song(id) { id = id || S.song; for (var i = 0; i < S.songs.length; i++) if (S.songs[i].id === id) return S.songs[i]; return null; }
    function arrangement() { var s = song(); if (!s) return null; for (var i = 0; i < s.arrs.length; i++) if (s.arrs[i].id === S.arr) return s.arrs[i]; return null; }
    function section() { var a = arrangement(); return a && (secOf(a, S.sec) || a.sections[0]) || null; }
    function snapshot() { return { songs: copy(S.songs), song: S.song, arr: S.arr, sec: S.sec, occ: S.occ, slide: S.slide }; }
    function done(text, snap) { S.toast = text; S.undo = snap || null; }
    function undo() { var u = S.undo; if (!u) return; S.songs = u.songs; S.song = u.song; S.arr = u.arr; S.sec = u.sec; S.occ = u.occ; S.slide = u.slide; S.undo = null; S.toast = null; }

    // ── search and filters (Decisions 133, 134) ───────────────────────────────────────────────
    function filtersOn() { return !!(S.f.choir || S.f.lang || S.f.occ); }
    function arrPasses(a) {
      function has(list, v) { return !v || (v === "Unspecified" ? !list.length : list.indexOf(v) >= 0); }
      return !a.archived && has(a.choirs, S.f.choir) && has(a.langs, S.f.lang) && has(a.occ, S.f.occ);
    }
    function arrText(a) { return norm([a.title, a.translator, a.choirs.join(" "), a.langs.join(" "), a.sections.map(function (s) { return s.text; }).join(" ")].join(" ")); }
    function result(s) {
      if (s.archived && !S.archived) return null;
      var q = norm(S.q), songHit = !q || norm([s.title, s.by].join(" ")).indexOf(q) >= 0, alias = null, lyric = null;
      if (q && !songHit) s.aliases.forEach(function (al) { if (!alias && norm(al).indexOf(q) >= 0) alias = al; });
      var matching = s.arrs.filter(function (a) {
        if (!arrPasses(a)) return false;
        if (!q || songHit || alias) return true;
        if (arrText(a).indexOf(q) < 0) return false;
        a.sections.forEach(function (x) { x.text.split("\n").forEach(function (l) { if (!lyric && norm(l).indexOf(q) >= 0) lyric = l; }); });
        if (!lyric && norm(a.choirs.join(" ")).indexOf(q) >= 0) lyric = "Choir · " + a.choirs.join(", ");
        return true;
      });
      if (!matching.length && (filtersOn() || (q && !songHit && !alias))) return null;
      if (q && !songHit && !alias && !lyric) return null;
      return { s: s, matching: matching, reason: alias ? "Alias · " + alias : lyric ? (lyric.indexOf("Choir") === 0 ? lyric : "“" + lyric.trim() + "”") : null };
    }
    function results() { return S.songs.map(result).filter(Boolean).sort(function (a, b) { return a.s.title.localeCompare(b.s.title); }); }

    // ── commands ──────────────────────────────────────────────────────────────────────────────
    function openArr(id, tab) { S.arr = id; S.view = "editor"; S.tab = tab || "lyrics"; var a = arrangement(); S.sec = a.sections[0] ? a.sections[0].id : null; S.slide = 0; S.occ = 0; S.open = {}; S.overlay = null; S.from = null; }
    function backToSongs() { S.view = "songs"; S.overlay = null; S.child = null; S.popover = null; S.from = null; }
    function newSongForm() { S.overlay = "newsong"; S.child = null; S.form = { title: "", by: "", aliases: "", moments: "" }; S.dupOpen = false; S.dup = null; S.focus = "ns.title"; }
    function duplicates() {
      var t = norm(S.form.title); if (t.length < 3) return [];
      return S.songs.filter(function (s) { return !s.archived && (norm(s.title).indexOf(t) >= 0 || t.indexOf(norm(s.title)) >= 0 || s.aliases.some(function (a) { return norm(a).indexOf(t) >= 0; }) ||
        t.split(" ").filter(function (w) { return w.length > 3 && norm(s.title + " " + s.aliases.join(" ")).indexOf(w) >= 0; }).length > 0); });
    }
    function createSong() {
      var snap = snapshot(), f = S.form, id = "s" + (S.songs.length + 10);
      S.songs.push({ id: id, title: f.title.trim(), by: f.by.trim(), aliases: f.aliases ? f.aliases.split(",").map(function (x) { return x.trim(); }).filter(Boolean) : [], moments: f.moments ? [f.moments] : [], notes: "", arrs: [] });
      S.song = id; S.overlay = null; S.q = ""; S.f = { choir: "", lang: "", occ: "" };
      done("Created " + f.title.trim() + ".", snap);
    }
    function detailsForm(create) {
      var a = create ? { title: "", langs: [], choirs: [], occ: [], translator: "", notes: "" } : arrangement();
      S.form = { create: create, title: a.title, langs: a.langs.slice(), choirs: a.choirs.slice(), occ: a.occ.slice(), translator: a.translator, notes: a.notes, tried: false };
      S.overlay = "details"; S.child = null;
    }
    function saveDetails() {
      var f = S.form; if (!f.langs.length) { f.tried = true; return; }
      var snap = snapshot();
      if (f.create) {
        var s = song(), id = "a" + Date.now() % 100000;
        s.arrs.push(arr(id, { title: f.title, langs: f.langs, choirs: f.choirs, occ: f.occ, translator: f.translator, notes: f.notes, sections: [], seq: [], updated: "just now" }));
        S.overlay = null; openArr(id, "lyrics"); S.focus = "addsec";
        done("Created the " + arrLabel(arrangement()) + " Arrangement.", snap);
      } else {
        var a = arrangement(); ["title", "langs", "choirs", "occ", "translator", "notes"].forEach(function (k) { a[k] = f[k]; });
        S.overlay = null; done("Saved Arrangement details.", snap);
      }
    }
    function openPopover(kind) { var s = section(), a = arrangement(); S.popover = { kind: kind, name: kind === "rename" ? s.name : nextName(a) }; S.focus = "pop.name"; }
    function nextName(a) {
      var n = 0; a.sections.forEach(function (s) { var m = /^(Verse|Bersikulo) (\d+)$/.exec(s.name); if (m) n = Math.max(n, +m[2]); });
      var word = a.langs[0] === "English" ? "Verse" : "Bersikulo";
      return a.sections.length ? word + " " + (n + 1) : "";
    }
    function commitPopover() {
      var p = S.popover, a = arrangement(), snap = snapshot(), name = p.name.trim(); if (!name) return;
      if (p.kind === "rename") { var s = section(), old = s.name; s.name = name; done("Renamed " + old + " to " + name + ".", snap); }
      else { var id = "n" + Date.now() % 100000; a.sections.push(sec(id, name, "", { approved: false })); S.sec = id; S.slide = 0; S.focus = "lyrics"; done("Added section " + name + ".", snap); }
      S.popover = null;
    }
    function addOcc(id) {
      var a = arrangement(), snap = snapshot(), at = a.seq.length ? Math.min(S.occ + 1, a.seq.length) : 0;
      a.seq.splice(at, 0, id); S.occ = at; S.open = {};
      done("Added " + secOf(a, id).name + " at " + pos(at) + " in the Sequence.", snap);
    }
    function pos(i) { return (i < 9 ? "0" : "") + (i + 1); }
    function moveOcc(i, d) { var a = arrangement(), snap = snapshot(), x = a.seq[i]; a.seq[i] = a.seq[i + d]; a.seq[i + d] = x; S.occ = i + d; done("Moved " + secOf(a, x).name + " to " + pos(i + d) + ".", snap); }
    function removeOcc(i) {
      var a = arrangement(), snap = snapshot(), name = secOf(a, a.seq[i]).name;
      a.seq.splice(i, 1); S.occ = Math.min(i, a.seq.length - 1); if (S.occ < 0) S.occ = 0;
      done("Removed " + pos(i) + " " + name + " from the Sequence. The section stays in Lyrics.", snap);
    }
    function moveSec(i, d) { var a = arrangement(), snap = snapshot(), x = a.sections[i]; a.sections[i] = a.sections[i + d]; a.sections[i + d] = x; done("Moved " + x.name + " in Lyrics. The Sequence is unchanged.", snap); }
    function boundary(kind) {
      var s = section(), g = groups(s.text), snap = snapshot(), i = Math.min(S.slide, g.length - 1);
      if (kind === "join" && i > 0) { g.splice(i - 1, 2, g[i - 1].concat(g[i])); S.slide = i - 1; done("Joined slide " + (i + 1) + " with slide " + i + ".", snap); }
      else if (kind === "split" && g[i] && g[i].length > 1) { var m = Math.ceil(g[i].length / 2); g.splice(i, 1, g[i].slice(0, m), g[i].slice(m)); S.slide = i + 1; done("Split slide " + (i + 1) + " after line " + m + ".", snap); }
      s.text = g.map(function (x) { return x.join("\n"); }).join("\n\n");
    }
    function openHistory(kind) { S.overlay = "history"; S.child = null; S.hist = { kind: kind, sessions: kind === "song" ? songHistory(song()) : arrHistory(arrangement()), open: { 0: true } }; S.rev = null; S.confirm = false; }
    function openRev(si, ri) { S.rev = { si: si, ri: ri }; S.child = "compare"; S.confirm = false; S.cmpTab = S.hist.kind === "song" ? "overview" : "lyrics"; }
    function restore() {
      var snap = snapshot(), r = S.hist.sessions[S.rev.si].revs[S.rev.ri].snap;
      if (S.hist.kind === "song") { var s = song(); ["title", "by", "aliases", "moments", "notes"].forEach(function (k) { s[k] = copy(r[k]); }); }
      else { var a = arrangement(); ["sections", "seq", "langs", "choirs", "occ", "title", "translator"].forEach(function (k) { a[k] = copy(r[k]); }); S.sec = a.sections[0] && a.sections[0].id; S.slide = 0; S.occ = 0; }
      S.overlay = null; S.child = null; S.confirm = false;
      done("Restored the revision from " + S.hist.sessions[S.rev.si].revs[S.rev.ri].at + ". Every earlier revision is still in History.", snap);
    }

    // ── shared views ──────────────────────────────────────────────────────────────────────────
    function closeTab(label) { return '<button class="pl-close-tab" data-act="close" aria-label="' + label + '" title="' + label + '">' + icon("chevron-right") + "</button>"; }
    function frame(c) { return K.slide(c || null); }
    function clip(c) { return { k: c.k, h: c.h, b: c.b.split("\n").slice(0, 4).join("\n") }; }   // never preview a squeezed frame
    function thumbs(list, cur, attr) {
      return '<div class="pl-thumbs sg-thumbs">' + list.map(function (c, i) {
        var on = i === cur ? ' aria-current="true"' : "";
        return '<button class="pr-tile pl-thumb" ' + attr + '="' + i + '"' + on + '><div class="o-slide-wrap"' + on + ">" + frame(c.fits === false ? clip(c) : c) + "</div>" +
          '<div class="o-caption"><span class="o-n">' + (i + 1) + '</span><span class="o-t">' + (c.fits === false ? icon("triangle-alert") + " " : "") + esc(c.b.split("\n")[0]) + "</span></div></button>";
      }).join("") + "</div>";
    }
    function chips(list) { return list.map(function (x) { return '<span class="pl-chip">' + esc(x) + "</span>"; }).join(" "); }
    function saved() { return '<span class="o-meta pl-saved">' + icon("check") + "Saved</span>"; }

    // ── Songs view: master-detail (Decisions 132, 219) ────────────────────────────────────────
    function libHead() {
      return '<header class="sg-libhead"><h1 class="o-title">Library</h1><div class="o-tabs sg-libtabs" role="tablist">' +
        '<button role="tab" aria-selected="true">Songs</button><button role="tab" aria-selected="false">Responses &amp; Prayers</button><button role="tab" aria-selected="false">General Presentations</button></div>' +
        '<span class="o-grow"></span><button class="o-btn" data-act="import">' + icon("file-down") + "Import .ordo</button></header>";
    }
    function select(key, label, values) {
      return '<select class="pl-input pl-input--sel sg-filter" data-filter="' + key + '" aria-label="' + label + '"><option value="">' + "All " + label.toLowerCase() + "s</option>" +
        values.concat(["Unspecified"]).map(function (v) { return '<option' + (S.f[key] === v ? " selected" : "") + ">" + esc(v) + "</option>"; }).join("") + "</select>";
    }
    function browser() {
      var rs = results(), active = S.songs.filter(function (s) { return !s.archived; }), list;
      if (!S.songs.length) list = '<div class="sg-empty-list"><b>No songs yet</b><div class="o-meta">Songs you create or import appear here, ready to use in any Mass.</div>' +
        '<button class="o-btn o-btn--primary" data-act="newsong">' + icon("plus") + 'Create song</button><button class="o-btn" data-act="import">' + icon("file-down") + "Import .ordo</button></div>";
      else if (!active.length && !S.archived) list = '<div class="sg-empty-list"><b>No active songs</b><div class="o-meta">Every Song is archived.</div><button class="o-btn" data-act="show-archived">Show archived</button></div>';
      else if (!rs.length) list = '<div class="sg-empty-list"><b>No songs match</b><div class="o-meta">Nothing matches ' + (S.q ? "“" + esc(S.q) + "”" : "") + (S.q && filtersOn() ? " with these filters" : filtersOn() ? "these filters" : "") + ".</div>" +
        '<button class="o-btn" data-act="clear">Clear search and filters</button></div>';
      else list = rs.map(function (r) {
        var s = r.s, n = s.arrs.filter(function (a) { return !a.archived; }).length;
        return '<button class="o-row sg-result" data-song="' + s.id + '"' + (s.id === S.song ? ' aria-selected="true"' : "") + '><span class="o-grow"><b class="o-ellipsis">' + esc(s.title) + "</b>" +
          '<span class="o-meta o-ellipsis">' + esc(r.reason || s.by) + "</span></span>" +
          '<span class="o-meta sg-count">' + (filtersOn() ? r.matching.length + " matching · " + n + " total" : n + (n === 1 ? " Arrangement" : " Arrangements")) + "</span>" +
          (s.archived ? '<span class="pl-chip">Archived</span>' : "") + "</button>";
      }).join("");
      return '<section class="o-pane sg-browser" aria-label="Songs"><div class="sg-browser-head"><div class="sg-row"><h2 class="o-h2">Songs</h2><span class="o-grow"></span>' +
        '<button class="o-btn o-btn--primary" data-act="newsong">' + icon("plus") + "New song</button></div>" +
        '<div class="o-field">' + icon("search") + '<input data-input="q" data-focus-key="q" placeholder="Search titles, aliases, composers, lyrics" value="' + esc(S.q) + '" aria-label="Search Songs"></div>' +
        '<div class="sg-filters">' + select("choir", "Choir", CHOIRS) + select("lang", "Language", LANGS) + select("occ", "Occasion", OCCASIONS) + "</div>" +
        '<div class="sg-row o-meta"><span>' + (S.songs.length ? rs.length + (rs.length === 1 ? " song" : " songs") : "") + "</span><span class=\"o-grow\"></span>" +
        (S.archived ? '<button class="pl-link" data-act="hide-archived">Hide archived</button>' : '<button class="pl-link" data-act="show-archived">Show archived</button>') + "<span>Title A–Z</span></div></div>" +
        '<div class="o-pane-body sg-results">' + list + "</div></section>";
    }
    function arrRow(a, s, match) {
      var r = readiness(a), slidesN = slideCount(a);
      return '<div class="sg-arr' + (match ? "" : " is-subdued") + '"><div class="o-grow"><div class="sg-row"><b>' + esc(arrLabel(a)) + "</b>" + (a.archived ? '<span class="pl-chip">Archived</span>' : s.archived ? '<span class="pl-chip">Available when Song is restored</span>' : status(r)) + "</div>" +
        '<div class="o-meta">' + a.sections.length + (a.sections.length === 1 ? " section · " : " sections · ") + slidesN + (slidesN === 1 ? " slide" : " slides") + " · " + esc(a.langs.join(", ")) +
        (a.translator ? " · " + esc(a.translator) : "") + " · updated " + esc(a.updated) + "</div>" +
        '<div class="sg-chips">' + (a.choirs.length ? chips(a.choirs) : '<span class="pl-chip">Choir unspecified</span>') + " " + chips(a.occ) + "</div></div>" +
        '<button class="o-btn" data-open="' + a.id + '">' + (s.archived || a.archived ? "Inspect" : "Open") + "</button></div>";
    }
    function detail() {
      var s = song();
      if (!S.songs.length) return '<section class="o-pane sg-detail"><div class="o-pane-body sg-guide"><h2 class="o-h2">Build your parish Song Library</h2><ol>' +
        "<li><b>Create or import a Song.</b> Its title, composer and aliases identify the composition.</li><li><b>Add an Arrangement.</b> Write its lyrics in sections, then set the Sequence they're sung in.</li>" +
        "<li><b>Say who sings it.</b> Choirs, languages and occasions help you find the right one when planning a Mass.</li></ol></div></section>";
      if (!s || !result(s)) return '<section class="o-pane sg-detail"><div class="o-pane-body sg-guide"><div class="o-meta">Select a Song to see its Arrangements.</div></div></section>';
      var r = result(s), match = r.matching.map(function (a) { return a.id; });
      var ordered = s.arrs.filter(function (a) { return match.indexOf(a.id) >= 0; }).concat(s.arrs.filter(function (a) { return match.indexOf(a.id) < 0; }));
      var meta = [["Composer / original author", s.by || "—"], ["Aliases", s.aliases.join(", ") || "—"], ["Usual moments", s.moments.join(", ") || "—"]];
      return '<section class="o-pane sg-detail" aria-label="Selected Song"><div class="sg-detail-head"><div class="o-grow"><div class="o-meta">Song</div><h2 class="o-title">' + esc(s.title) + "</h2></div>" + saved() +
        '<button class="o-btn" data-act="song-history">' + icon("history") + "History</button>" +
        (s.archived ? "" : '<button class="o-btn o-btn--primary" data-act="new-arr">' + icon("plus") + "New arrangement</button>") +
        '<button class="o-btn o-btn--icon" data-act="song-menu" aria-label="More Song actions" title="More">' + icon("ellipsis") + "</button></div>" +
        '<div class="o-pane-body sg-detail-body">' +
        (s.archived ? '<div class="o-notice" data-state="info">' + icon("archive") + '<div class="o-grow"><b>Archived.</b> Inspect only; restore to edit or use it in a Mass. ' + esc(s.notes) + '</div><button class="o-btn o-btn--primary">Restore</button></div>' : "") +
        '<dl class="sg-meta">' + meta.map(function (m) { return "<div><dt>" + m[0] + "</dt><dd>" + esc(m[1]) + "</dd></div>"; }).join("") + "</dl>" +
        '<div class="pl-block-head"><b>Arrangements</b><span class="o-meta">' + (filtersOn() || S.q ? r.matching.length + " matching · " + s.arrs.length + " total" : s.arrs.length + " total") + "</span></div>" +
        (s.arrs.length ? '<div class="sg-arrs">' + ordered.map(function (a) { return arrRow(a, s, !(filtersOn() || S.q) || match.indexOf(a.id) >= 0); }).join("") + "</div>" :
          '<div class="pl-empty">' + icon("list-music", "o-icon-20") + '<b>No Arrangements yet</b><div class="o-meta">An Arrangement holds one rendition’s lyrics, sections and Sequence for a language and choir.</div>' +
          '<button class="o-btn o-btn--primary" data-act="new-arr" data-focus-key="first-arr">' + icon("plus") + "Add first Arrangement</button></div>") +
        "</div>" + (S.popover && S.popover.kind === "song-menu" ? '<div class="o-menu sg-overflow" role="menu"><button class="o-row" role="menuitem" disabled>' + icon("merge") + "Merge with another song<span class=\"o-meta\">After MVP</span></button>" +
          '<div class="pl-menu-sep"></div><button class="o-row" role="menuitem">' + icon("archive") + "Archive song</button></div>" : "") + "</section>";
    }

    // ── Arrangement editor (Decisions 115–131, 138, 145) ─────────────────────────────────────
    function edHead(a, s) {
      var r = readiness(a), ro = a.archived || s.archived;
      return '<header class="sg-edhead"><button class="o-btn o-btn--quiet o-btn--icon" data-act="back" aria-label="Back to Songs" title="Back to Songs">' + icon("arrow-left") + "</button>" +
        '<div class="o-grow"><div class="o-meta">Library / Songs / ' + esc(s.title) + '</div><h1 class="o-title pl-title">' + esc(arrLabel(a)) + "</h1></div>" +
        status(r) + saved() +
        '<button class="o-btn" data-act="details">' + icon("sliders-horizontal") + "Arrangement details</button>" +
        '<button class="o-btn" data-act="arr-history">' + icon("history") + "History</button>" +
        '<button class="o-btn o-btn--icon" aria-label="More Arrangement actions" title="More">' + icon("ellipsis") + "</button></header>" +
        (ro ? '<div class="o-notice sg-banner" data-state="info">' + icon("archive") + "<div><b>Archived.</b> Inspect only until restored.</div></div>" : "") +
        (S.from ? '<div class="o-notice sg-banner" data-state="info">' + icon("corner-down-right") + "<div><b>Opened from " + esc(S.from) + '.</b> Fix the section here; Readiness updates when it’s resolved. <button class="pl-link">Back to the Mass</button></div></div>' : "") +
        '<div class="o-tabs sg-edtabs" role="tablist"><button role="tab" data-tab="lyrics" aria-selected="' + (S.tab === "lyrics") + '">Lyrics</button>' +
        '<button role="tab" data-tab="sequence" aria-selected="' + (S.tab === "sequence") + '">Sequence' + (a.sections.length && !a.seq.length ? ' <span class="o-status" data-state="warning"><span class="o-marker o-marker--diamond"></span></span>' : "") + "</button></div>";
    }
    function secStatus(s) {
      var sl = slides(s);
      if (!sl.length) return '<span class="o-meta">Empty</span>';
      if (sl.some(function (x) { return !x.fits; })) return statusLabel("warning", "Doesn’t fit", "diamond");
      if (!s.approved) return statusLabel("info", "Needs review");
      return "";
    }
    function lyricsTask(a) {
      var s = section(), ro = a.archived;
      var list = a.sections.map(function (x, i) {
        var n = slides(x).length, u = uses(a, x.id);
        return '<div class="o-row sg-sec"' + (s && x.id === s.id ? ' aria-selected="true"' : "") + ">" + icon("grip-vertical") +
          '<button class="sg-sec-name" data-sec="' + x.id + '" data-focus-key="sec-' + x.id + '"><b>' + esc(x.name) + '</b><span class="o-meta">' + n + (n === 1 ? " slide" : " slides") + " · " + (u ? "used " + u + "×" : "not in Sequence") + "</span>" + secStatus(x) + "</button>" +
          (s && x.id === s.id && !ro ? '<span class="pl-acts"><button class="o-btn o-btn--quiet o-btn--icon" data-secmv="' + i + ':-1" aria-label="Move up" title="Move up (Alt+Up)"' + (i ? "" : " disabled") + ">" + icon("arrow-up") + "</button>" +
            '<button class="o-btn o-btn--quiet o-btn--icon" data-secmv="' + i + ':1" aria-label="Move down" title="Move down (Alt+Down)"' + (i < a.sections.length - 1 ? "" : " disabled") + ">" + icon("arrow-down") + "</button>" +
            '<button class="o-btn o-btn--quiet o-btn--icon" data-act="rename" data-anchor="rename" aria-label="Section actions" title="Rename section">' + icon("ellipsis") + "</button></span>" : "") + "</div>";
      }).join("");
      var left = '<section class="o-pane sg-left" aria-label="Sections"><div class="o-pane-head"><b>Sections</b><span class="o-grow"></span>' +
        (ro ? "" : '<button class="o-btn" data-act="add-sec" data-anchor="addsec" data-focus-key="addsec">' + icon("plus") + "Add section</button>") + "</div>" +
        '<div class="o-pane-body o-rows--open">' + (a.sections.length ? list : "") + "</div>" +
        '<div class="sg-pane-foot o-meta">Lyrics order is for writing only; the Sequence sets what’s sung.</div></section>';
      var center;
      if (!s) center = '<section class="o-pane sg-center"><div class="o-pane-body"><div class="pl-empty">' + icon("text", "o-icon-20") + "<b>No sections yet</b>" +
        '<div class="o-meta">Add each part once, such as a verse, the refrain or an ending. A section used several times is still written once.</div>' +
        '<button class="o-btn o-btn--primary" data-act="add-sec" data-anchor="addsec" data-focus-key="addsec">' + icon("plus") + "Add first section</button></div></div></section>";
      else {
        var sl = slides(s), cur = Math.min(S.slide, Math.max(0, sl.length - 1)), g = sl[cur], u = uses(a, s.id), bad = sl.filter(function (x) { return !x.fits; }).length;
        var split = g && g.lines > 1 ? "Split after line " + Math.ceil(g.lines / 2) : "Split slide";
        center = '<section class="o-pane sg-center" aria-label="Section lyrics"><div class="o-pane-head"><div class="o-grow"><b>' + esc(s.name) + '</b> <span class="o-meta">' + (u ? "Sung " + u + (u === 1 ? " time" : " times") + " in the Sequence" : "Not in the Sequence yet") + "</span></div>" +
          (!u && !ro ? '<button class="o-btn o-btn--quiet" data-act="add-to-seq">' + icon("list-plus") + "Add to Sequence</button>" : "") + "</div>" +
          '<div class="o-pane-body sg-center-body">' +
          (bad ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div><b>" + bad + (bad === 1 ? " slide doesn’t fit." : " slides don’t fit.") + "</b> A slide holds at most four lines. Split it at a phrase before Ordo would reduce the size.</div></div>" :
            !s.approved && sl.length ? '<div class="o-notice" data-state="info">' + icon("info") + '<div class="o-grow"><b>Pagination needs review.</b> The lyrics changed since the slides were approved. Check each slide in the Inspector.</div>' +
            '<button class="o-btn" data-act="approve">' + icon("check") + "Approve pagination</button></div>" : "") +
          '<label class="o-label" for="sg-lyrics">Lyrics</label>' +
          '<textarea id="sg-lyrics" class="pl-input sg-lyrics" rows="9" data-input="lyrics" data-focus-key="lyrics"' + (ro ? " readonly" : "") + ' placeholder="Type or paste the words. Leave a blank line where the next slide should start.">' + esc(s.text) + "</textarea>" +
          '<div class="sg-bounds"><span class="o-meta">' + icon("corner-down-left") + " A blank line starts a new slide.</span><span class=\"o-grow\"></span>" +
          (ro ? "" : '<button class="o-btn" data-act="split"' + (g && g.lines > 1 ? "" : " disabled") + ">" + icon("scissors") + esc(split) + "</button>" +
            '<button class="o-btn" data-act="join"' + (cur > 0 ? "" : " disabled") + ">" + icon("merge") + (cur > 0 ? "Join slide " + (cur + 1) + " with slide " + cur : "Join with previous slide") + "</button>") + "</div>" +
          '<div class="pl-block-head"><b>Generated slides</b><span class="o-meta">' + sl.length + (sl.length === 1 ? " slide" : " slides") + " · select one to inspect</span></div>" +
          (sl.length ? thumbs(sl, cur, "data-slide") : '<div class="o-meta">No slides yet: this section has no lyrics.</div>') + "</div></section>";
      }
      return left + center + inspector(a, s ? slides(s)[Math.min(S.slide, Math.max(0, slides(s).length - 1))] : null, s, s ? "Slide " + (Math.min(S.slide, slides(s).length - 1) + 1) + " of " + slides(s).length : "");
    }
    function sequenceTask(a) {
      var ro = a.archived;
      var left = '<section class="o-pane sg-left" aria-label="Available sections"><div class="o-pane-head"><b>Available sections</b></div><div class="o-pane-body o-rows--open">' +
        (a.sections.length ? a.sections.map(function (x) {
          var n = slides(x).length, u = uses(a, x.id);
          return '<div class="o-row sg-sec"><span class="o-grow sg-sec-name"><b>' + esc(x.name) + '</b><span class="o-meta">' + n + (n === 1 ? " slide" : " slides") + " · " + (u ? "used " + u + "×" : "unused") + "</span></span>" +
            (ro ? "" : '<button class="o-btn" data-addocc="' + x.id + '">' + icon("plus") + "Add</button>") + "</div>";
        }).join("") : '<div class="o-meta sg-pad">Write sections in Lyrics first.</div>') + "</div></section>";
      var allOpen = a.seq.length && a.seq.every(function (x, i) { return S.open[i]; });
      var rows = a.seq.map(function (id, i) {
        var x = secOf(a, id), sl = slides(x), sel = i === S.occ, open = !!S.open[i];
        return '<div class="sg-occ' + (sel ? " is-selected" : "") + '"><div class="o-row sg-occ-row"' + (sel ? ' aria-selected="true"' : "") + ">" + icon("grip-vertical") +
          '<button class="sg-expander" data-expand="' + i + '" aria-expanded="' + open + '" aria-label="' + (open ? "Collapse" : "Expand") + '">' + icon(open ? "chevron-down" : "chevron-right") + "</button>" +
          '<button class="sg-occ-name" data-occ="' + i + '" data-focus-key="occ-' + i + '"><span class="sg-pos">' + pos(i) + "</span><b>" + esc(x.name) + '</b><span class="o-meta">' + sl.length + (sl.length === 1 ? " slide" : " slides") + "</span></button>" +
          (uses(a, id) > 1 ? '<span class="pl-chip">' + icon("link") + "Same section</span>" : "") + '<span class="o-grow"></span>' +
          (sel && !ro ? '<span class="pl-acts"><button class="o-btn o-btn--quiet" data-act="repeat">' + icon("repeat") + "Repeat</button>" +
            '<button class="o-btn o-btn--quiet o-btn--icon" data-occmv="' + i + ':-1" aria-label="Move up" title="Move up (Alt+Up)"' + (i ? "" : " disabled") + ">" + icon("arrow-up") + "</button>" +
            '<button class="o-btn o-btn--quiet o-btn--icon" data-occmv="' + i + ':1" aria-label="Move down" title="Move down (Alt+Down)"' + (i < a.seq.length - 1 ? "" : " disabled") + ">" + icon("arrow-down") + "</button>" +
            '<button class="o-btn o-btn--quiet o-btn--icon" data-occdel="' + i + '" aria-label="Remove from Sequence" title="Remove (Delete)">' + icon("x") + "</button></span>" : "") + "</div>" +
          (open ? '<div class="sg-occ-kids">' + thumbs(sl, sel ? S.slide : -1, 'data-occ="' + i + '" data-kid') + "</div>" : "") + "</div>";
      }).join("");
      var center = '<section class="o-pane sg-center" aria-label="Performance sequence"><div class="o-pane-head"><div class="o-grow"><b>Sequence</b> <span class="o-meta">' +
        (a.seq.length ? a.seq.length + " occurrences · " + slideCount(a) + " slides in sung order" : "Incomplete") + "</span></div>" +
        (a.seq.length ? '<button class="o-btn o-btn--quiet" data-act="expand-all">' + (allOpen ? "Collapse all" : "Expand all") + "</button>" : "") + "</div>" +
        '<div class="o-pane-body sg-center-body">' + (a.seq.length ? rows :
          '<div class="pl-empty">' + icon("list-ordered", "o-icon-20") + "<b>No performance sequence yet</b>" +
          '<div class="o-meta">Add sections in the order they’re sung. Until then this Arrangement is incomplete and gives a Mass no slides.</div>' +
          (a.sections.length && !ro ? '<button class="o-btn o-btn--primary" data-addocc="' + a.sections[0].id + '" data-focus-key="first-occ">' + icon("plus") + "Add first occurrence · " + esc(a.sections[0].name) + "</button>" : "") + "</div>") + "</div></section>";
      var x = a.seq.length ? secOf(a, a.seq[S.occ]) : null, sl = x ? slides(x) : [], k = Math.min(S.slide, Math.max(0, sl.length - 1));
      return left + center + inspector(a, sl[k] || null, x, x ? pos(S.occ) + " " + x.name + " · slide " + (k + 1) + " of " + sl.length : "");
    }
    function inspector(a, c, s, where) {
      var meta = c ? [["Section", s.name], ["Position", where], ["Pagination", !c.fits ? "Doesn’t fit · " + c.lines + " lines" : s.approved ? "Approved" : "Needs review"], ["Lyric size", c.fits ? "72 · fits safely" : "Would need below 64"], ["Theme", "Ordo Black · preview"]] : [];
      return '<aside class="o-pane pl-inspector sg-insp" aria-label="Inspector"><div class="o-pane-head"><b>Inspector</b><span class="o-grow"></span><span class="o-meta">' + esc(where) + "</span></div>" +
        '<div class="o-pane-body pl-insp-body">' + frame(c && !c.fits ? clip(c) : c) +
        (c && !c.fits ? '<div class="pr-private" data-state="unavailable">' + icon("triangle-alert") + "<div><b>Doesn’t fit</b><div>" + (c.lines - 4) + " line" + (c.lines - 4 === 1 ? "" : "s") + " would be cut. Split the slide at a phrase.</div></div></div>" : "") +
        (c ? '<dl class="pl-meta">' + meta.map(function (m) { return "<dt>" + m[0] + "</dt><dd>" + esc(m[1]) + "</dd>"; }).join("") + "</dl>" : '<div class="o-meta">Nothing to inspect yet.</div>') + "</div></aside>";
    }

    // ── overlays ──────────────────────────────────────────────────────────────────────────────
    function newSong() {
      var f = S.form, d = duplicates();
      var dupHtml = d.length ? '<div class="sg-dups"><button class="pl-link" data-act="dups" aria-expanded="' + S.dupOpen + '">' + icon(S.dupOpen ? "chevron-down" : "chevron-right") + d.length + (d.length === 1 ? " possible existing Song" : " possible existing Songs") + "</button>" +
        (S.dupOpen ? '<div class="sg-dup-list">' + d.map(function (s) { return '<button class="o-row" data-dup="' + s.id + '"><span class="o-grow"><b>' + esc(s.title) + '</b><span class="o-meta">' + esc(s.by) + " · " + s.arrs.length + " Arrangements</span></span>" + icon("chevron-right") + "</button>"; }).join("") + "</div>" : "") + "</div>" : "";
      return '<div class="o-scrim"></div><div class="o-slideover sg-form' + (S.child ? " is-receded" : "") + '" role="dialog" aria-modal="true" aria-labelledby="sg-ns-t">' + closeTab("Cancel new song") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="sg-ns-t">New song</h2><div class="o-meta">The composition itself. Lyrics, language and choirs belong to its Arrangements.</div></div></div>' +
        '<div class="o-overlay-body"><div class="pl-form">' +
        '<div class="pl-frow"><span class="o-label">Primary title <span class="o-meta">required</span></span><div>' + nsField("title", f.title, "Ginoo, Kaloy-i Kami") + dupHtml + "</div></div>" +
        '<div class="pl-frow"><span class="o-label">Composer or original author</span><div>' + nsField("by", f.by, "Optional") + "</div></div>" +
        '<div class="pl-frow"><span class="o-label">Aliases</span><div>' + nsField("aliases", f.aliases, "Optional, separated by commas") + '<div class="o-meta">Other names people search for, such as Kyrie.</div></div></div>' +
        '<div class="pl-frow"><span class="o-label">Usual moment</span><div>' + nsField("moments", f.moments, "Optional, such as Entrance or Communion") + "</div></div></div></div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-act="close">Cancel</button><button class="o-btn o-btn--primary" data-act="create-song"' + (f.title.trim() ? "" : " disabled") + ">Create song</button></div></div>" +
        (S.child === "dup" ? dupPreview() : "");
    }
    function nsField(k, v, ph) { return '<input class="pl-input" data-input="ns" data-key="' + k + '" data-focus-key="ns.' + k + '" value="' + esc(v) + '" placeholder="' + esc(ph) + '">'; }
    function dupPreview() {
      var s = song(S.dup);
      return '<div class="o-slideover pl-child sg-child" role="dialog" aria-modal="true" aria-labelledby="sg-dp-t"><div class="o-overlay-head"><button class="o-btn o-btn--quiet o-btn--icon" data-act="back-child" aria-label="Back to New song" title="Back">' + icon("arrow-left") + "</button>" +
        '<div class="o-grow"><h2 class="o-h2" id="sg-dp-t">' + esc(s.title) + '</h2><div class="o-meta">Existing Song · preview only</div></div></div>' +
        '<div class="o-overlay-body"><dl class="sg-meta">' + [["Composer / original author", s.by], ["Aliases", s.aliases.join(", ") || "—"], ["Usual moments", s.moments.join(", ") || "—"]].map(function (m) { return "<div><dt>" + m[0] + "</dt><dd>" + esc(m[1]) + "</dd></div>"; }).join("") + "</dl>" +
        '<div class="pl-block-head"><b>Arrangements</b></div>' + s.arrs.map(function (a) { return '<div class="sg-arr is-compact"><div class="o-grow"><b>' + esc(arrLabel(a)) + '</b><div class="o-meta">' + esc(a.langs.join(", ")) + " · " + (a.choirs.join(", ") || "Choir unspecified") + "</div></div></div>"; }).join("") + "</div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-act="back-child">Back</button><button class="o-btn o-btn--primary" data-act="use-existing">Use this existing Song</button></div></div>';
    }
    function details() {
      var f = S.form, s = song(), a = f.create ? null : arrangement();
      function chipset(key, values, extra) {
        return '<div class="sg-chipset">' + values.map(function (v) { var on = f[key].indexOf(v) >= 0; return '<button class="sg-toggle" data-chip="' + key + ":" + esc(v) + '" aria-pressed="' + on + '">' + (on ? icon("check") : "") + esc(v) + "</button>"; }).join("") +
          '<button class="sg-toggle sg-toggle--add">' + icon("plus") + esc(extra) + "</button></div>";
      }
      var shared = !f.create && a.choirs.length > 1;
      return '<div class="o-scrim"></div><div class="o-slideover sg-form" role="dialog" aria-modal="true" aria-labelledby="sg-ad-t">' + closeTab("Cancel") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="sg-ad-t">' + (f.create ? "New Arrangement" : "Arrangement details") + '</h2><div class="o-meta">' + esc(s.title) + (f.create ? "" : " · " + esc(arrLabel(a))) + "</div></div></div>" +
        '<div class="o-overlay-body">' +
        (shared ? '<div class="o-notice" data-state="info">' + icon("users") + "<div><b>Shared by " + esc(a.choirs.join(" and ")) + ".</b> Saving updates this one Arrangement for every choir that uses it. Mass plans keep their references; past Presentation snapshots don’t change.</div></div>" : "") +
        '<div class="pl-form">' +
        '<div class="pl-frow"><span class="o-label">Display title</span><div><input class="pl-input" data-input="ad" data-key="title" data-focus-key="ad.title" value="' + esc(f.title) + '" placeholder="Leave blank to show ' + esc(arrLabel({ title: "", langs: f.langs.length ? f.langs : ["Language"], choirs: f.choirs })) + '"></div></div>' +
        '<div class="pl-frow"><span class="o-label">Languages <span class="o-meta">at least one</span></span><div>' + chipset("langs", LANGS, "Language") +
          (f.tried && !f.langs.length ? '<div class="sg-error">' + icon("circle-alert") + "Choose the language this rendition is sung in.</div>" : '<div class="o-meta">Several only when this exact rendition mixes them.</div>') + "</div></div>" +
        '<div class="pl-frow"><span class="o-label">Choirs</span><div>' + chipset("choirs", CHOIRS, "Choir") + '<div class="o-meta">Choirs that sing this exact rendition. Different wording or order needs its own Arrangement.</div></div></div>' +
        '<div class="pl-frow"><span class="o-label">Translator or adapter</span><div><input class="pl-input" data-input="ad" data-key="translator" value="' + esc(f.translator) + '" placeholder="Optional"><div class="o-meta">The composer stays on the Song.</div></div></div>' +
        '<div class="pl-frow"><span class="o-label">Occasions</span><div>' + chipset("occ", OCCASIONS.slice(0, 5), "Occasion") + '<div class="o-meta">Advisory, for finding it later. Leaving it empty means unspecified, not “always”.</div></div></div>' +
        '<div class="pl-frow"><span class="o-label">Source and rights notes</span><div><textarea class="pl-input" rows="2" data-input="ad" data-key="notes" placeholder="Optional">' + esc(f.notes) + "</textarea></div></div></div></div>" +
        '<div class="o-overlay-foot">' + (f.create ? "" : '<button class="o-btn">' + icon("copy") + "Duplicate Arrangement</button>") + '<span class="o-grow"></span>' +
        '<button class="o-btn" data-act="close">Cancel</button><button class="o-btn o-btn--primary" data-act="save-details">' + (f.create ? "Create Arrangement" : "Save details") + "</button></div></div>";
    }
    function history() {
      var h = S.hist, ent = h.kind === "song" ? song().title : arrLabel(arrangement());
      var list = h.sessions.map(function (g, si) {
        var open = !!h.open[si];
        return '<div class="sg-sess' + (open ? " is-open" : "") + '"><button class="o-row sg-sess-row" data-sess="' + si + '" aria-expanded="' + open + '">' + icon(open ? "chevron-down" : "chevron-right") +
          '<span class="o-grow"><b>' + esc(g.what) + '</b><span class="o-meta">' + esc(g.when) + " · " + esc(g.how) + "</span></span><span class=\"o-meta\">" + g.revs.length + (g.revs.length === 1 ? " revision" : " revisions") + "</span></button>" +
          (open ? '<div class="sg-revs">' + g.revs.map(function (r, ri) {
            var sel = S.rev && S.rev.si === si && S.rev.ri === ri;
            return '<button class="o-row sg-rev" data-rev="' + si + ":" + ri + '"' + (sel ? ' aria-selected="true"' : "") + (r.current ? " disabled" : "") + '><span class="o-meta">' + esc(r.at) + '</span><span class="o-grow">' + esc(r.what) + "</span>" +
              (r.current ? '<span class="pl-chip">Current</span>' : icon("chevron-right")) + "</button>";
          }).join("") + "</div>" : "") + "</div>";
      }).join("");
      return '<div class="o-scrim"></div><div class="o-slideover sg-history' + (S.child ? " is-receded" : "") + '" role="dialog" aria-modal="true" aria-labelledby="sg-h-t">' + closeTab("Close History") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="sg-h-t">History</h2><div class="o-meta">' + (h.kind === "song" ? "Song · " : "Arrangement · ") + esc(ent) + "</div></div></div>" +
        '<div class="o-overlay-body">' + list + '<p class="o-meta">' + (h.kind === "song" ? "Song History covers its title, attribution and metadata. Each Arrangement keeps its own History." : "Every saved change is kept. Select a revision to compare it with the current Arrangement.") + "</p></div></div>" +
        (S.child === "compare" ? compare() : "");
    }
    function compare() {
      var h = S.hist, g = h.sessions[S.rev.si], r = g.revs[S.rev.ri], cur = h.kind === "song" ? song() : arrangement(), old = r.snap;
      var tabs = h.kind === "song" ? ["overview"] : ["overview", "lyrics", "sequence", "slides"];
      function side(x, other, label, when, mark) {
        var body;
        if (S.cmpTab === "overview") body = h.kind === "song" ? [["Title", x.title], ["Composer", x.by], ["Aliases", x.aliases.join(", ") || "—"], ["Usual moments", x.moments.join(", ") || "—"]].map(function (m, i) {
          var o = [other.title, other.by, other.aliases.join(", ") || "—", other.moments.join(", ") || "—"][i];
          return '<div class="sg-cmp-line' + (m[1] !== o ? " is-" + mark : "") + '"><span class="o-meta">' + m[0] + "</span><div>" + esc(m[1]) + "</div></div>"; }).join("") :
          [["Languages", x.langs.join(", ")], ["Choirs", x.choirs.join(", ") || "—"], ["Sections", x.sections.length], ["Sequence", x.seq.length + " occurrences · " + slideCount(x) + " slides"]].map(function (m, i) {
            var o = [other.langs.join(", "), other.choirs.join(", ") || "—", other.sections.length, other.seq.length + " occurrences · " + slideCount(other) + " slides"][i];
            return '<div class="sg-cmp-line' + (String(m[1]) !== String(o) ? " is-" + mark : "") + '"><span class="o-meta">' + m[0] + "</span><div>" + esc(m[1]) + "</div></div>"; }).join("");
        else if (S.cmpTab === "lyrics") body = x.sections.map(function (s) {
          var os = secOf(other, s.id), ol = os ? os.text.split("\n").filter(function (x) { return x.trim(); }) : [], li = -1;   // compare lines by position
          return '<div class="sg-cmp-sec"><span class="o-meta">' + esc(s.name) + "</span>" + s.text.split("\n").map(function (l) { return l.trim() ? '<div class="sg-cmp-line' + (ol[++li] !== l ? " is-" + mark : "") + '">' + esc(l) + "</div>" : '<div class="sg-cmp-gap">slide break</div>'; }).join("") + "</div>";
        }).join("");
        else if (S.cmpTab === "sequence") body = x.seq.map(function (id, i) { return '<div class="sg-cmp-line' + (other.seq[i] !== id ? " is-" + mark : "") + '"><span class="sg-pos">' + pos(i) + "</span> " + esc(secOf(x, id).name) + "</div>"; }).join("");
        else body = '<div class="sg-cmp-slides">' + [].concat.apply([], x.seq.map(function (id) { return slides(secOf(x, id)); })).map(function (c) { return frame(c); }).join("") + "</div>";
        return '<div class="sg-cmp-col"><div class="sg-cmp-head"><b>' + label + '</b><span class="o-meta">' + esc(when) + "</span></div>" + body + "</div>";
      }
      var foot = S.confirm ?
        '<div class="o-overlay-foot sg-confirm"><div class="sg-confirm-text">' + icon("triangle-alert") + "<div><b>Restore the whole " + (h.kind === "song" ? "Song" : "Arrangement") + " to " + esc(r.at) + "?</b> " +
          (h.kind === "song" ? "Only the Song’s identity and metadata change; its Arrangements stay as they are." : "Used by " + esc(cur.choirs.join(" and ") || "no choir") + ". 2 editable Mass plans will show the restored words; Mass-local copies and past Presentation snapshots don’t change.") +
          " It becomes a new revision, so nothing in History is lost.</div></div>" +
          '<div class="sg-confirm-acts"><button class="o-btn" data-act="keep">Keep current</button><button class="o-btn o-btn--warning" data-act="do-restore">Restore entire ' + (h.kind === "song" ? "Song" : "Arrangement") + "</button></div></div>" :
        '<div class="o-overlay-foot"><span class="o-meta o-grow">Read-only comparison · restores the whole ' + (h.kind === "song" ? "Song" : "Arrangement") + '</span><button class="o-btn" data-act="back-child">Back</button><button class="o-btn o-btn--primary" data-act="restore">Restore this revision</button></div>';
      return '<div class="o-slideover pl-child sg-compare" role="dialog" aria-modal="true" aria-labelledby="sg-c-t"><div class="o-overlay-head"><button class="o-btn o-btn--quiet o-btn--icon" data-act="back-child" aria-label="Back to History" title="Back">' + icon("arrow-left") + "</button>" +
        '<div class="o-grow"><h2 class="o-h2" id="sg-c-t">Compare revision</h2><div class="o-meta">' + esc(g.when.split(" · ")[0]) + " " + esc(r.at) + " · " + esc(r.what) + "</div></div></div>" +
        (tabs.length > 1 ? '<div class="o-tabs" role="tablist">' + tabs.map(function (t) { return '<button role="tab" data-cmptab="' + t + '" aria-selected="' + (S.cmpTab === t) + '">' + t.charAt(0).toUpperCase() + t.slice(1) + "</button>"; }).join("") + "</div>" : "") +
        '<div class="o-overlay-body"><div class="sg-cmp">' + side(cur, old, "Current", "Saved just now", "cur") + side(old, cur, "Revision to restore", g.when.split(" · ")[0] + " " + r.at, "old") + "</div></div>" + foot + "</div>";
    }

    // ── Song import review (Decisions 135–137) ────────────────────────────────────────────────
    function choice(di, ai) { var d = S.docs[di], k = d.arrs[ai].kind, v = S.staged[di + ":" + ai]; return v != null ? v : k === "decide" ? "" : k; }
    function importView() {
      var tally = { new: 0, attach: 0, update: 0, add: 0, skip: 0, open: 0, invalid: 0 };
      S.docs.forEach(function (d, di) { if (d.invalid) { tally.invalid++; return; } d.arrs.forEach(function (x, ai) { var c = choice(di, ai); if (!c) tally.open++; else tally[c]++; }); });
      var list = S.docs.map(function (d, di) {
        var k = d.invalid ? ["danger", "Invalid"] : (function () { var c = choice(di, 0), base = d.arrs[0].kind; return !c ? KIND.decide : c === base ? KIND[base] : c === "skip" ? KIND.skip : c === "update" ? ["warning", "Update existing"] : c === "add" ? ["info", "Add as new Arrangement"] : KIND[c] || KIND[base]; })();
        return '<button class="o-row sg-doc" data-doc="' + di + '"' + (di === S.doc ? ' aria-selected="true"' : "") + '><span class="o-grow"><b>' + esc(d.title) + '</b><span class="o-meta">' + esc(d.file) + "</span></span>" +
          '<span class="sg-kind" data-state="' + k[0] + '">' + esc(k[1]) + "</span></button>";
      }).join("");
      var d = S.docs[S.doc], m = d.match ? song(d.match) : null, body;
      if (d.invalid) body = '<div class="o-notice" data-state="danger">' + icon("circle-x") + "<div><b>This document can’t be imported.</b> " + esc(d.invalid) + "</div></div>" +
        '<p class="o-meta">Invalid documents stay here so you can see why, but they never change the Library. Fix the file and import it again.</p>';
      else body = '<div class="sg-imp-cards"><div class="sg-imp-card"><div class="o-section-label">Imported document</div><b>' + esc(d.title) + '</b><div class="o-meta">' + esc(d.by) + " · " + d.arrs.length + (d.arrs.length === 1 ? " Arrangement" : " Arrangements") + "</div></div>" +
        '<div class="sg-imp-card"><div class="o-section-label">Song in the Library</div>' + (m ? "<b>" + esc(m.title) + '</b><div class="o-meta">' + esc(m.by) + " · " + m.arrs.length + " Arrangements</div>" : '<b>New Song</b><div class="o-meta">No credible match</div>') + "</div></div>" +
        '<div class="sg-evidence">' + icon("search-check") + "<div><b>" + (m ? "Same Song work." : "No existing Song.") + "</b> " + esc(d.why) + "</div></div>" +
        d.arrs.map(function (x, ai) {
          var c = choice(S.doc, ai), opts = OUTCOMES[x.kind];
          return '<div class="sg-imp-arr"><div class="pl-block-head"><b>' + esc(x.label) + '</b><span class="o-meta">' + x.a.sections.length + " sections · " + slideCount(x.a) + " slides</span></div>" +
            '<div class="o-meta">' + esc(x.evidence) + "</div>" +
            (x.kind === "decide" ? '<div class="sg-outcomes" role="radiogroup" aria-label="Outcome">' + opts.slice(1).map(function (o) {
              return '<label class="sg-outcome"><input type="radio" name="o' + S.doc + ai + '" data-stage="' + S.doc + ":" + ai + '" value="' + o[0] + '"' + (c === o[0] ? " checked" : "") + "><span><b>" + o[1] + "</b>" + (o[0] === "add" ? ' <span class="pl-chip">Recommended</span>' : "") +
                '<span class="o-meta">' + (o[0] === "add" ? "Keeps both renditions side by side." : o[0] === "update" ? "Replaces the existing Arrangement as a new revision. Koro San Roque and 1 editable Mass plan will see the new words; Mass-local copies don’t change." : "Leaves the Library as it is.") + "</span></span></label>";
            }).join("") + "</div>" :
              '<div class="sg-row"><span class="o-label">Staged action</span><select class="pl-input pl-input--sel" data-stage="' + S.doc + ":" + ai + '">' + opts.map(function (o) { return '<option value="' + o[0] + '"' + (c === o[0] ? " selected" : "") + ">" + o[1] + "</option>"; }).join("") + "</select></div>") +
            '<div class="sg-row"><button class="o-btn" data-act="compare-doc">' + icon("columns-2") + (m ? "Compare contents" : "Preview document") + "</button></div></div>";
        }).join("");
      var parts = [];
      if (tally.new) parts.push(tally.new + " new Song"); if (tally.attach + tally.add) parts.push((tally.attach + tally.add) + " new Arrangement" + (tally.attach + tally.add > 1 ? "s" : ""));
      if (tally.update) parts.push(tally.update + " update"); if (tally.skip) parts.push(tally.skip + " skipped"); if (tally.invalid) parts.push(tally.invalid + " invalid");
      return '<div class="sg-import"><header class="sg-edhead"><button class="o-btn o-btn--quiet o-btn--icon" data-act="back" aria-label="Cancel import" title="Cancel import">' + icon("x") + "</button>" +
        '<div class="o-grow"><div class="o-meta">Library / Songs</div><h1 class="o-title pl-title">Review Song import</h1></div><span class="o-meta">Nothing has changed in the Library yet.</span></header>' +
        '<div class="sg-panes sg-imp-panes"><section class="o-pane sg-browser"><div class="o-pane-head"><b>' + S.docs.length + " documents</b></div><div class=\"o-pane-body o-rows--open\">" + list + "</div></section>" +
        '<section class="o-pane sg-detail"><div class="sg-detail-head"><div class="o-grow"><div class="o-meta">' + esc(d.file) + '</div><h2 class="o-title">' + esc(d.title) + "</h2></div></div>" +
        '<div class="o-pane-body sg-detail-body">' + body + "</div></section></div>" +
        '<footer class="sg-imp-foot"><div class="o-grow"><b>One Library update</b><div class="o-meta">If any approved change fails, none of them is saved.</div></div>' +
        '<span class="o-meta">' + parts.join(" · ") + (tally.open ? " · " + tally.open + " decision" + (tally.open > 1 ? "s" : "") + " to make" : "") + "</span>" +
        '<button class="o-btn o-btn--primary" data-act="commit-import"' + (tally.open ? " disabled" : "") + ">" + icon("check") + "Import approved</button></footer></div>";
    }
    function compareDoc() {
      var d = S.docs[S.doc], x = d.arrs[0], m = d.match ? song(d.match) : null, near = m ? (x.near ? m.arrs.filter(function (a) { return a.id === x.near; })[0] : m.arrs[0]) : null;
      var tabs = ["overview", "lyrics", "sequence", "slides"];
      function col(a, label, other, mark) {
        var body;
        if (!a) return '<div class="sg-cmp-col"><div class="sg-cmp-head"><b>' + label + "</b></div><div class=\"o-meta\">No match.</div></div>";
        if (S.cmpTab === "overview") body = [["Languages", a.langs.join(", ")], ["Choirs", a.choirs.join(", ") || "—"], ["Sections", a.sections.map(function (s) { return s.name; }).join(", ")], ["Sequence", a.seq.length + " occurrences · " + slideCount(a) + " slides"]]
          .map(function (r) { return '<div class="sg-cmp-line"><span class="o-meta">' + r[0] + "</span><div>" + esc(r[1]) + "</div></div>"; }).join("");
        else if (S.cmpTab === "lyrics") body = a.sections.map(function (s) {
          var os = other && secOf(other, s.id), ol = os ? os.text.split("\n").filter(function (x) { return x.trim(); }) : [], li = -1;   // compare lines by position
          return '<div class="sg-cmp-sec"><span class="o-meta">' + esc(s.name) + "</span>" + s.text.split("\n").map(function (l) { return l.trim() ? '<div class="sg-cmp-line' + (ol[++li] !== l && other ? " is-" + mark : "") + '">' + esc(l) + "</div>" : '<div class="sg-cmp-gap">slide break</div>'; }).join("") + "</div>";
        }).join("");
        else if (S.cmpTab === "sequence") body = a.seq.map(function (id, i) { return '<div class="sg-cmp-line' + (other && other.seq[i] !== id ? " is-" + mark : "") + '"><span class="sg-pos">' + pos(i) + "</span> " + esc(secOf(a, id).name) + "</div>"; }).join("");
        else body = '<div class="sg-cmp-slides">' + [].concat.apply([], a.seq.map(function (id) { return slides(secOf(a, id)); })).map(function (c) { return frame(c); }).join("") + "</div>";
        return '<div class="sg-cmp-col"><div class="sg-cmp-head"><b>' + label + "</b></div>" + body + "</div>";
      }
      return '<div class="o-scrim"></div><div class="o-slideover sg-compare sg-compare--wide" role="dialog" aria-modal="true" aria-labelledby="sg-cd-t">' + closeTab("Close comparison") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="sg-cd-t">Compare contents</h2><div class="o-meta">Read-only · the staged action doesn’t change here</div></div>' +
        (m ? '<label class="o-meta sg-row">Nearest match <select class="pl-input pl-input--sel">' + m.arrs.map(function (a) { return "<option" + (a === near ? " selected" : "") + ">" + esc(arrLabel(a)) + "</option>"; }).join("") + "</select></label>" : "") + "</div>" +
        '<div class="o-tabs" role="tablist">' + tabs.map(function (t) { return '<button role="tab" data-cmptab="' + t + '" aria-selected="' + (S.cmpTab === t) + '">' + t.charAt(0).toUpperCase() + t.slice(1) + "</button>"; }).join("") + "</div>" +
        '<div class="o-overlay-body"><div class="sg-cmp">' + col(x.a, "Imported · " + esc(d.file), near, "cur") + col(near, "Library · " + (near ? esc(arrLabel(near)) : ""), x.a, "old") + "</div></div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-act="close">Back to review</button></div></div>';
    }

    // ── render ────────────────────────────────────────────────────────────────────────────────
    function popoverHtml() {
      var p = S.popover; if (!p || p.kind === "song-menu") return "";
      var sugg = ["Verse 1", "Bersikulo 1", "Koro", "Refrain", "Bridge", "Katapusan"];
      return '<div class="o-menu sg-pop" role="dialog" aria-label="' + (p.kind === "rename" ? "Rename section" : "Add section") + '"><div class="sg-pop-body"><label class="o-label" for="sg-pop-name">Section name</label>' +
        '<input id="sg-pop-name" class="pl-input" data-input="pop" data-focus-key="pop.name" value="' + esc(p.name) + '" placeholder="Name this part">' +
        (p.kind === "add" ? '<div class="sg-chipset">' + sugg.map(function (x) { return '<button class="sg-toggle" data-sugg="' + esc(x) + '">' + esc(x) + "</button>"; }).join("") + "</div>" : '<div class="o-meta">Every occurrence in the Sequence shows the new name.</div>') +
        '<div class="sg-row"><span class="o-grow"></span><button class="o-btn" data-act="pop-cancel">Cancel</button><button class="o-btn o-btn--primary" data-act="pop-ok"' + (p.name.trim() ? "" : " disabled") + ">" + (p.kind === "rename" ? "Rename" : "Add section") + "</button></div></div></div>";
    }
    function render() {
      app.style.setProperty("--o-scale", S.scale);
      app.dataset.layout = W >= 1700 ? "wide" : "compact";
      var keep = {}, ae = document.activeElement, fk = ae && app.contains(ae) && ae.dataset ? ae.dataset.focusKey : null, caret = fk && ae.selectionStart;
      [".sg-results", ".sg-detail-body", ".sg-left .o-pane-body", ".sg-center-body", ".sg-history .o-overlay-body"].forEach(function (q) { var el = app.querySelector(q); if (el) keep[q] = el.scrollTop; });
      var body, ov = "";
      if (S.view === "import") { body = importView(); if (S.overlay === "compare-doc") ov = compareDoc(); }
      else if (S.view === "editor") {
        var a = arrangement(), s = song();
        body = '<div class="sg-editor">' + edHead(a, s) + '<div class="sg-panes sg-ed-panes">' + (S.tab === "lyrics" ? lyricsTask(a) : sequenceTask(a)) + "</div></div>";
      } else body = '<div class="sg-lib">' + libHead() + '<div class="sg-panes">' + browser() + detail() + "</div></div>";
      if (S.overlay === "newsong") ov = newSong();
      else if (S.overlay === "details") ov = details();
      else if (S.overlay === "history") ov = history();
      var toast = S.toast ? '<div class="o-toast" role="status">' + esc(S.toast) + (S.undo ? '<button class="o-btn" data-act="undo">Undo</button>' : "") + "</div>" : "";
      app.innerHTML = '<div class="o-body">' + K.navrail("Library") + '<div class="o-grow sg-ws">' + body + "</div></div>" + popoverHtml() + ov + toast;
      K.icons();
      Object.keys(keep).forEach(function (q) { var el = app.querySelector(q); if (el) el.scrollTop = keep[q]; });
      var pop = app.querySelector(".sg-pop, .sg-overflow");
      if (pop) {
        var anchor = app.querySelector(S.popover.kind === "rename" ? '[data-anchor="rename"]' : S.popover.kind === "song-menu" ? '[data-act="song-menu"]' : '[data-anchor="addsec"]');
        if (anchor) { var r0 = app.getBoundingClientRect(), k = r0.width / app.offsetWidth || 1, br = anchor.getBoundingClientRect();
          pop.style.left = Math.min((br.left - r0.left) / k, W - 300) + "px"; pop.style.top = (br.bottom - r0.top) / k + 4 + "px"; }
      }
      var want = S.focus || fk;
      if (want) {
        var t = app.querySelector('[data-focus-key="' + want + '"]');
        if (t) { t.focus({ preventScroll: true }); if (S.focus && t.tagName === "BUTTON") t.classList.add("o-focus");
          if (caret != null && t.setSelectionRange && !S.focus) try { t.setSelectionRange(caret, caret); } catch (err) { /* not a text field */ } }
      }
      S.focus = null;
      if (opts.afterRender) opts.afterRender(api);
    }

    app.addEventListener("click", function (e) {
      var t = e.target.closest("[data-act],[data-song],[data-open],[data-sec],[data-slide],[data-tab],[data-occ],[data-expand],[data-addocc],[data-occmv],[data-occdel],[data-secmv],[data-chip],[data-sugg],[data-dup],[data-sess],[data-rev],[data-cmptab],[data-doc]");
      if (!t || t.disabled) return;
      e.preventDefault();
      var d = t.dataset, a = d.act;
      if (a !== "undo") { S.toast = null; S.undo = null; }
      if (S.popover && S.popover.kind === "song-menu" && a !== "song-menu") S.popover = null;
      if (a === "newsong") newSongForm();
      else if (a === "import") { S.view = "import"; S.overlay = null; }
      else if (a === "clear") { S.q = ""; S.f = { choir: "", lang: "", occ: "" }; }
      else if (a === "show-archived") S.archived = true;
      else if (a === "hide-archived") S.archived = false;
      else if (d.song) { S.song = d.song; S.popover = null; }
      else if (d.open) openArr(d.open);
      else if (a === "back") backToSongs();
      else if (a === "close") { if (S.child) S.child = null; else S.overlay = null; }
      else if (a === "back-child") { S.child = null; S.confirm = false; }
      else if (a === "dups") S.dupOpen = !S.dupOpen;
      else if (d.dup) { S.dup = d.dup; S.child = "dup"; }
      else if (a === "use-existing") { S.song = S.dup; S.overlay = null; S.child = null; S.q = ""; }
      else if (a === "create-song") createSong();
      else if (a === "new-arr") detailsForm(true);
      else if (a === "details") detailsForm(false);
      else if (d.chip) { var p = d.chip.split(":"), l = S.form[p[0]], at = l.indexOf(p[1]); if (at >= 0) l.splice(at, 1); else l.push(p[1]); }
      else if (a === "save-details") saveDetails();
      else if (a === "song-menu") S.popover = S.popover && S.popover.kind === "song-menu" ? null : { kind: "song-menu" };
      else if (d.tab) { S.tab = d.tab; S.slide = 0; S.popover = null; }
      else if (d.sec) { S.sec = d.sec; S.slide = 0; S.popover = null; }
      else if (d.slide != null) S.slide = +d.slide;
      else if (a === "add-sec") openPopover("add");
      else if (a === "rename") openPopover("rename");
      else if (d.sugg) S.popover.name = d.sugg;
      else if (a === "pop-ok") commitPopover();
      else if (a === "pop-cancel") S.popover = null;
      else if (a === "add-to-seq") { var ar = arrangement(); S.occ = ar.seq.length - 1; addOcc(section().id); S.tab = "sequence"; }
      else if (d.secmv) { var m = d.secmv.split(":"); moveSec(+m[0], +m[1]); }
      else if (a === "split") boundary("split");
      else if (a === "join") boundary("join");
      else if (a === "approve") { var snap = snapshot(); section().approved = true; done("Approved " + section().name + " pagination.", snap); }
      else if (d.addocc) addOcc(d.addocc);
      else if (d.kid != null) { S.occ = +d.occ; S.slide = +d.kid; }
      else if (d.occ != null) { S.occ = +d.occ; S.slide = 0; }
      else if (d.expand != null) S.open[d.expand] = !S.open[d.expand];
      else if (a === "expand-all") { var all = arrangement().seq.every(function (x, i) { return S.open[i]; }); S.open = {}; if (!all) arrangement().seq.forEach(function (x, i) { S.open[i] = true; }); }
      else if (a === "repeat") { var r = arrangement(); addOcc(r.seq[S.occ]); }
      else if (d.occmv) { var q = d.occmv.split(":"); moveOcc(+q[0], +q[1]); }
      else if (d.occdel != null) removeOcc(+d.occdel);
      else if (a === "song-history") openHistory("song");
      else if (a === "arr-history") openHistory("arr");
      else if (d.sess != null) S.hist.open[d.sess] = !S.hist.open[d.sess];
      else if (d.rev) { var v = d.rev.split(":"); openRev(+v[0], +v[1]); }
      else if (d.cmptab) S.cmpTab = d.cmptab;
      else if (a === "restore") S.confirm = true;
      else if (a === "keep") S.confirm = false;
      else if (a === "do-restore") restore();
      else if (d.doc != null) S.doc = +d.doc;
      else if (a === "compare-doc") { S.overlay = "compare-doc"; S.cmpTab = "lyrics"; }
      else if (a === "commit-import") { S.view = "songs"; S.toast = "Imported the approved changes in one update."; }
      else if (a === "undo") undo();
      render();
    });
    app.addEventListener("input", function (e) {
      var el = e.target, k = el.dataset.input;
      if (k === "q") S.q = el.value;
      else if (k === "lyrics") { var s = section(); s.text = el.value; s.approved = false; var n = slides(s).length; S.slide = Math.min(S.slide, Math.max(0, n - 1)); }
      else if (k === "ns") S.form[el.dataset.key] = el.value;
      else if (k === "ad") S.form[el.dataset.key] = el.value;
      else if (k === "pop") S.popover.name = el.value;
      else return;
      render();
    });
    app.addEventListener("change", function (e) {
      var el = e.target;
      if (el.dataset.filter) { S.f[el.dataset.filter] = el.value; render(); }
      else if (el.dataset.stage) { S.staged[el.dataset.stage] = el.value; render(); }
    });
    app.addEventListener("keydown", function (e) {
      if (e.key === "Escape" && (S.popover || S.overlay)) { if (S.popover) S.popover = null; else if (S.child) S.child = null; else S.overlay = null; e.preventDefault(); render(); return; }
      var row = e.target.closest && e.target.closest("[data-occ]");
      if (row && S.tab === "sequence" && e.key === "Delete") { removeOcc(+row.dataset.occ); e.preventDefault(); render(); }
      if (row && S.tab === "sequence" && e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
        var i = +row.dataset.occ, dd = e.key === "ArrowUp" ? -1 : 1, n = arrangement().seq.length;
        if (i + dd >= 0 && i + dd < n) { moveOcc(i, dd); S.focus = "occ-" + (i + dd); e.preventDefault(); render(); }
      }
    });

    var api = {
      app: app, state: S, render: render,
      song: function (id) { S.song = id; return api; }, search: function (q) { S.q = q; return api; }, filter: function (k, v) { S.f[k] = v; return api; },
      empty: function () { S.songs = []; return api; }, archiveAll: function () { S.songs.forEach(function (s) { s.archived = true; }); return api; },
      showArchived: function () { S.archived = true; return api; },
      newSong: function (title) { newSongForm(); S.form.title = title || ""; return api; }, dups: function () { S.dupOpen = true; return api; }, previewDup: function (id) { S.dup = id; S.child = "dup"; return api; },
      createSong: function (title) { newSongForm(); S.form.title = title; createSong(); S.toast = null; S.undo = null; return api; },
      open: function (song, arr, tab) { S.song = song; openArr(arr, tab); return api; }, sec: function (id) { S.sec = id; S.slide = 0; return api; }, slide: function (i) { S.slide = i; return api; },
      occ: function (i) { S.occ = i; return api; }, expand: function (i) { S.open[i] = true; return api; },
      details: function (create) { detailsForm(!!create); return api; }, form: function (k, v) { S.form[k] = v; return api; }, tryCreate: function () { saveDetails(); return api; },
      createArr: function (o) { detailsForm(true); Object.keys(o || {}).forEach(function (k) { S.form[k] = o[k]; }); saveDetails(); S.toast = null; S.undo = null; return api; },
      addSection: function () { openPopover("add"); return api; }, rename: function () { openPopover("rename"); return api; },
      lyrics: function (text) { var s = section(); s.text = text; s.approved = false; return api; },
      removeOcc: function (i) { removeOcc(i); return api; }, clearSeq: function () { arrangement().seq = []; return api; },
      history: function (kind) { openHistory(kind || "arr"); return api; }, rev: function (si, ri) { openRev(si, ri); return api; }, cmp: function (t) { S.cmpTab = t; return api; },
      confirm: function () { S.confirm = true; return api; }, restore: function () { restore(); return api; },
      fromMass: function (label) { S.from = label; return api; },
      importDoc: function (i) { S.view = "import"; S.doc = i; return api; }, stage: function (key, v) { S.staged[key] = v; return api; }, compareDoc: function (t) { S.overlay = "compare-doc"; S.cmpTab = t || "lyrics"; return api; },
      quiet: function () { S.toast = null; S.undo = null; return api; }
    };
    K.fit(wrapper, app, W, H);
    if (opts.setup) opts.setup(api);
    render();
    return api;
  }

  K.library = library;
  K.SONGS = SONGS;
})();
