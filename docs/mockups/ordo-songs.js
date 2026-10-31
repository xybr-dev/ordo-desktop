// Ordo mockups: the Library's Songs view, Arrangement editor, History and Song import on the
// shared kit (load after ordo-kit.js). Each instance owns a working copy of the sample Library,
// so its flows can edit, restore and Undo without touching other mockups. Mockup behaviour only.
(function () {
  "use strict";
  var K = window.OrdoKit, esc = K.esc, icon = K.icon, statusLabel = K.statusLabel;
  function copy(x) { return JSON.parse(JSON.stringify(x)); }
  function norm(s) { return String(s || "").toLowerCase().replace(/\s+/g, " ").trim(); }

  var LANGS = ["Cebuano", "English", "Tagalog", "Latin"];
  // Ordo's built-in language list: a language is picked from it, never typed in. The second entry holds
  // other names people search by.
  var LANG_CATALOG = [["Cebuano", "Bisaya, Binisaya, Sinugbuanon, Visayan"], ["English", ""], ["Tagalog", "Filipino, Pilipino"], ["Latin", ""],
    ["Hiligaynon", "Ilonggo"], ["Waray", "Winaray, Samar-Leyte"], ["Ilocano", "Iloko"], ["Kapampangan", "Pampango"], ["Bikol", "Bicolano"],
    ["Pangasinan", ""], ["Chavacano", "Chabacano, Zamboangueño"], ["Spanish", "Español"], ["Italian", ""], ["Korean", ""], ["Chinese (Mandarin)", ""]];
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

  // ── the Library's other views: Choirs, Responses & Prayers, General Presentations ───────────
  // Choir records (Decisions 87, 221). Arrangements name their Choirs, so a rename reaches them.
  var CHOIR_RECS = [
    { id: "c1", name: "Koro San Roque", label: "KSR", langs: ["Cebuano"], notes: "Sings the Sunday 7:00 and 9:00 Masses.", archived: false },
    { id: "c2", name: "Youth Choir", label: "Youth", langs: ["Cebuano", "English"], notes: "", archived: false },
    { id: "c3", name: "Koro Sta. Cecilia", label: "", langs: ["Cebuano", "Latin"], notes: "Weekday evening Masses. Ask them for the Latin settings in Lent.", archived: false },
    { id: "c4", name: "Koro Sto. Niño", label: "", langs: ["Cebuano"], notes: "Joined Koro San Roque in 2025.", archived: true }
  ];
  // Response/Prayer entries: ordered exchanges of an optional operator-only prompt and a required
  // projected response (Decisions 71, 255, 257). `plans` counts linked Mass plans; `behind` counts
  // those still bound to an earlier revision.
  var ROLES = ["Greeting", "Penitential Act", "Memorial Acclamation", "Lord's Prayer", "Lamb of God", "Dismissal", "Other"];
  var WHO = [["P", "Priest"], ["D", "Deacon"], ["L", "Lector"], ["C", "Cantor"], ["K", "Commentator"]], RWHO = [["T", "All"], ["Ch", "Choir"], ["C", "Cantor"]];
  var HDR_DEFAULT = { Cebuano: "TUBAG:" };   // a language's template default (Decision 84)
  function ex(who, trig, resp) { return { who: who, trig: trig, rwho: "T", resp: resp }; }
  var IN_NAME = "Sa ngalan sa Amahan, ug sa Anak, ug sa Espiritu Santo.";
  var RESP = [
    { id: "r1", name: "Pagtimbaya", role: "Greeting", lang: "Cebuano", hdr: "inherit", hdrText: "", plans: 6, templates: 2, ex: [ex("P", IN_NAME, "Amen."), ex("P", "Ang Ginoo maanaa kaninyo.", "Ug maanaa usab sa imong espiritu.")] },
    { id: "r2", name: "Pagtimbaya · grasya", role: "Greeting", lang: "Cebuano", hdr: "inherit", hdrText: "", plans: 2, templates: 1, ex: [ex("P", IN_NAME, "Amen."),
      ex("P", "Ang grasya sa atong Ginoong Jesukristo, ang gugma sa Dios, ug ang panag-uban sa Espiritu Santo maanaa kaninyong tanan.", "Ug maanaa usab sa imong espiritu.")] },
    { id: "r3", name: "Misteryo sa Pagtoo", role: "Memorial Acclamation", lang: "Cebuano", hdr: "inherit", hdrText: "", plans: 6, templates: 2,
      ex: [ex("P", "Ang misteryo sa pagtoo.", "Ginoo, among ginasangyaw ang imong kamatayon, ug ginasugid ang imong pagkabanhaw, hangtud sa imong pagbalik.")] },
    { id: "r4", name: "Amahan Namo", role: "Lord's Prayer", lang: "Cebuano", hdr: "hide", hdrText: "", plans: 6, templates: 2, ex: [
      ex("P", "Sumala sa sugo sa atong Manluluwas, ug tinudloan sa Dios, mangahas kita sa pag-ingon:", "Amahan namo, nga anaa sa mga langit, pagdaygon ang imong ngalan."),
      ex("P", "", "Moabot kanamo ang imong gingharian; matuman ang imong kabubut-on, dinhi sa yuta maingon sa langit.")] },
    { id: "r5", name: "Kordero sa Dios", role: "Lamb of God", lang: "Cebuano", hdr: "hide", hdrText: "", plans: 5, templates: 2, ex: [
      ex("P", "", "Kordero sa Dios, nga nagawagtang sa mga sala sa kalibutan, kaloy-i kami."), ex("P", "", "Kordero sa Dios, nga nagawagtang sa mga sala sa kalibutan, hatagi kami sa kalinaw.")] },
    { id: "r6", name: "Pagpadala", role: "Dismissal", lang: "Cebuano", hdr: "inherit", hdrText: "", plans: 6, templates: 2, behind: 2, ex: [ex("D", "Lakaw kamo sa kalinaw.", "Salamat sa Dios.")] },
    { id: "r7", name: "Greeting", role: "Greeting", lang: "English", hdr: "override", hdrText: "ALL:", plans: 1, templates: 1, ex: [
      ex("P", "In the name of the Father, and of the Son, and of the Holy Spirit.", "Amen."), ex("P", "The Lord be with you.", "And with your spirit.")] },
    { id: "r8", name: "Pagtimbaya · daan", role: "Greeting", lang: "Cebuano", hdr: "inherit", hdrText: "", plans: 0, templates: 0, archived: true, ex: [ex("P", "Ang Ginoo maanaa kaninyo.", "Ug sa imong espiritu usab.")] }
  ];
  // General Presentations: constrained Text or Image Slides with an auto-play default of eight
  // seconds that a Slide may override (Decisions 71, 257, 258).
  function tx(h, b, f) { return { kind: "text", h: h, b: b, f: f, dur: null }; }
  var GP = [
    { id: "g1", name: "Pahibalo sa Domingo", note: "Weekly announcements", dur: 8, plans: 4, slides: [
      tx("Mga Pahibalo", "Bible Study matag Huwebes, 6:00 sa gabii, sa parish hall.", ""),
      tx("Mga Pahibalo", "Ang mga ginikanan sa mga bata nga bunyagan sa Domingo motambong sa seminar sa Sabado, 2:00 sa hapon.", ""),
      tx("", "Salamat sa inyong pagtambong. Ang Ginoo magauban kanato.", "Parokya ni San Roque")] },
    { id: "g2", name: "Pasalamat", note: "Closing thanks", dur: 8, plans: 6, slides: [tx("", "Salamat sa inyong pagtambong sa Santos nga Misa.", "Parokya ni San Roque")] },
    { id: "g3", name: "Ikaduhang Kolekta", note: "Second collection appeal", dur: 10, plans: 1, slides: [
      tx("Ikaduhang kolekta", "Ang ikaduhang kolekta karong Domingo alang sa pag-ayo sa kampanaryo.", "Parokya ni San Roque"),
      { kind: "image", file: "kampanaryo-2026.jpg", alt: "The bell tower under repair", dur: 12 }] },
    { id: "g4", name: "Bible Study", note: "Recurring invitation", dur: 8, plans: 0, slides: [tx("Bible Study", "Matag Huwebes, 6:00 sa gabii, sa parish hall. Dad-a ang inyong Biblia.", "")] },
    { id: "g5", name: "Fiesta 2025", note: "Novena schedule", dur: 8, plans: 0, archived: true, slides: [tx("Fiesta ni San Roque", "Novena matag gabii, 6:00, gikan Agosto 7 hangtud 15.", "")] }
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
      toast: null, undo: null, focus: null, from: null, langs: LANGS.slice(),
      lview: opts.lview || "songs", lib: { choirs: copy(CHOIR_RECS), responses: copy(RESP), general: copy(GP) }, lsel: { choirs: "c1", responses: "r1", general: "g1" },
      lq: { choirs: "", responses: "", general: "" }, lf: { lang: "", role: "" }, larch: {}, lslide: 0, newChoir: null, lnameDraft: null, pick: null, made: null, secDraft: null,
      docs: copy(DOCS), doc: 1, staged: {}, cmpDoc: null, scale: 1 };
    function song(id) { id = id || S.song; for (var i = 0; i < S.songs.length; i++) if (S.songs[i].id === id) return S.songs[i]; return null; }
    function arrangement() { var s = song(); if (!s) return null; for (var i = 0; i < s.arrs.length; i++) if (s.arrs[i].id === S.arr) return s.arrs[i]; return null; }
    function section() { var a = arrangement(); return a && (secOf(a, S.sec) || a.sections[0]) || null; }
    function snapshot() { return { songs: copy(S.songs), lib: copy(S.lib), lsel: copy(S.lsel), song: S.song, arr: S.arr, sec: S.sec, occ: S.occ, slide: S.slide }; }
    function done(text, snap) { S.toast = text; S.undo = snap || null; }
    function undo() { var u = S.undo; if (!u) return; S.songs = u.songs; S.lib = u.lib; S.lsel = u.lsel; S.song = u.song; S.arr = u.arr; S.sec = u.sec; S.occ = u.occ; S.slide = u.slide; S.undo = null; S.toast = null; }

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
      var t = norm(S.form.title), edit = S.overlay === "songdetails"; if (t.length < 3 || (edit && t === norm(song().title))) return [];
      return S.songs.filter(function (s) { return !s.archived && !(edit && s.id === S.song) && (norm(s.title).indexOf(t) >= 0 || t.indexOf(norm(s.title)) >= 0 || s.aliases.some(function (a) { return norm(a).indexOf(t) >= 0; }) ||
        t.split(" ").filter(function (w) { return w.length > 3 && norm(s.title + " " + s.aliases.join(" ")).indexOf(w) >= 0; }).length > 0); });
    }
    function createSong() {
      var snap = snapshot(), f = S.form, id = "s" + (S.songs.length + 10);
      S.songs.push({ id: id, title: f.title.trim(), by: f.by.trim(), aliases: f.aliases ? f.aliases.split(",").map(function (x) { return x.trim(); }).filter(Boolean) : [], moments: f.moments ? [f.moments] : [], notes: "", arrs: [] });
      S.song = id; S.overlay = null; S.q = ""; S.f = { choir: "", lang: "", occ: "" };
      done("Created " + f.title.trim() + ".", snap);
    }
    function songForm() {
      var s = song();
      S.form = { title: s.title, by: s.by, aliases: s.aliases.join(", "), moments: s.moments.join(", ") };
      S.overlay = "songdetails"; S.child = null; S.dupOpen = false; S.dup = null;
    }
    function list(v) { return v ? v.split(",").map(function (x) { return x.trim(); }).filter(Boolean) : []; }
    function songChanged() { var s = song(), f = S.form; return f.title.trim() !== s.title || f.by.trim() !== s.by || list(f.aliases).join("|") !== s.aliases.join("|") || list(f.moments).join("|") !== s.moments.join("|"); }
    function saveSong() {
      var s = song(), f = S.form, snap = snapshot(); if (!f.title.trim() || !songChanged()) return;
      s.title = f.title.trim(); s.by = f.by.trim(); s.aliases = list(f.aliases); s.moments = list(f.moments);
      S.overlay = null; done("Saved Song details.", snap);
    }
    function detailsForm(create) {
      var a = create ? { title: "", langs: [], choirs: [], occ: [], translator: "", notes: "" } : arrangement();
      S.form = { create: create, title: a.title, langs: a.langs.slice(), choirs: a.choirs.slice(), occ: a.occ.slice(), translator: a.translator, notes: a.notes, tried: false };
      S.overlay = "details"; S.child = null; S.pick = null; S.made = null;
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
    function choirNames() { return S.lib.choirs.filter(function (c) { return !c.archived; }).map(function (c) { return c.name; }); }
    function pickRows() {
      var f = S.pick.target || S.form, k = S.pick.key, q = norm(S.pick.q);
      if (k === "choirs") {
        var names = choirNames(), hits = names.filter(function (c) { return f.choirs.indexOf(c) < 0 && norm(c).indexOf(q) >= 0; });
        return hits.map(function (c) { return { val: c, label: c }; }).concat(q && !S.lib.choirs.some(function (c) { return norm(c.name) === q; }) ? [{ create: S.pick.q.trim() }] : []);
      }
      return LANG_CATALOG.filter(function (l) { return f.langs.indexOf(l[0]) < 0 && norm(l[0] + ", " + l[1]).indexOf(q) >= 0; }).slice(0, 6)
        .map(function (l) { var al = q && norm(l[0]).indexOf(q) < 0 ? l[1].split(", ").filter(function (a) { return norm(a).indexOf(q) >= 0; })[0] : null; return { val: l[0], label: l[0], also: al }; });
    }
    function pickValue(v) {
      var k = S.pick.key, f = S.pick.target || S.form; if (k === "langs" && S.langs.indexOf(v) < 0) S.langs.push(v);
      if (f[k].indexOf(v) < 0) f[k].push(v); S.pick = null;
    }
    function addChoirRec(name) { var c = { id: "c" + Date.now() % 100000, name: name, label: "", langs: [], notes: "", archived: false }; S.lib.choirs.push(c); return c; }
    function createChoir(name) { addChoirRec(name); S.form.choirs.push(name); S.made = name; S.pick = null; }   // persists at once (Decision 221)
    function commitSecName(v) {
      var s = section(), name = v.trim(); S.secDraft = null;
      if (!s || name === s.name) return;
      if (!name) { done("A section needs a name. Kept " + s.name + "."); return; }
      var snap = snapshot(), old = s.name; s.name = name; done("Renamed " + old + " to " + name + ". Every Sequence occurrence shows the new name.", snap);
    }
    function openPopover(kind) { S.popover = { kind: kind, name: nextName(arrangement()) }; S.focus = "pop.name"; }
    function nextName(a) {
      var n = 0; a.sections.forEach(function (s) { var m = /^(Verse|Bersikulo) (\d+)$/.exec(s.name); if (m) n = Math.max(n, +m[2]); });
      var word = a.langs[0] === "English" ? "Verse" : "Bersikulo";
      return a.sections.length ? word + " " + (n + 1) : "";
    }
    function commitPopover() {
      var p = S.popover, a = arrangement(), snap = snapshot(), name = p.name.trim(); if (!name) return;
      var id = "n" + Date.now() % 100000; a.sections.push(sec(id, name, "", { approved: false })); S.sec = id; S.slide = 0; S.focus = "lyrics"; done("Added section " + name + ".", snap);
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
      var tabs = [["songs", "Songs"], ["responses", "Responses &amp; Prayers"], ["general", "General Presentations"], ["choirs", "Choirs"]];
      return '<header class="sg-libhead"><h1 class="o-title">Library</h1><div class="o-tabs sg-libtabs" role="tablist">' +
        tabs.map(function (t) { return '<button role="tab" data-lview="' + t[0] + '" aria-selected="' + (S.lview === t[0]) + '">' + t[1] + "</button>"; }).join("") + "</div>" +
        '<span class="o-grow"></span>' + (S.lview === "songs" ? '<button class="o-btn" data-act="import">' + icon("file-down") + "Import .ordo</button>" : "") + "</header>";
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
        '<div class="sg-filters">' + select("choir", "Choir", choirNames()) + select("lang", "Language", S.langs) + select("occ", "Occasion", OCCASIONS) + "</div>" +
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
        (s.archived ? "" : '<button class="o-btn" data-act="song-details">' + icon("pencil") + "Edit details</button>") +
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
            '<button class="o-btn o-btn--quiet o-btn--icon" data-secmv="' + i + ':1" aria-label="Move down" title="Move down (Alt+Down)"' + (i < a.sections.length - 1 ? "" : " disabled") + ">" + icon("arrow-down") + "</button></span>" : "") + "</div>";
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
        var nm = S.secDraft != null ? S.secDraft : s.name;
        center = '<section class="o-pane sg-center" aria-label="Section lyrics"><div class="o-pane-head"><div class="o-grow">' +
          (ro ? "<b>" + esc(s.name) + "</b>" : '<label class="sg-secname' + (S.secDraft != null ? " is-editing" : "") + '" title="Section name · Enter renames, Esc cancels"><input data-secname data-focus-key="secname" value="' + esc(nm) + '" size="' + Math.max(4, nm.length + 1) + '" aria-label="Section name">' + icon("pencil") + "</label>") +
          ' <span class="o-meta">' + (u ? "Sung " + u + (u === 1 ? " time" : " times") + " in the Sequence" : "Not in the Sequence yet") + "</span></div>" +
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

    // ── the Library's other views (Decisions 87, 221, 257; proposed 278, 279) ─────────────────
    // Each is the Songs master-detail: search and filters on the left, the selected entry on the
    // right. Entry content autosaves; a name commits on Enter or blur with Undo.
    var LV = { choirs: ["Choirs", "choir"], responses: ["Responses & Prayers", "Response or Prayer"], general: ["General Presentations", "General Presentation"] };
    function lentry(v) { v = v || S.lview; var l = S.lib[v]; for (var i = 0; i < l.length; i++) if (l[i].id === S.lsel[v]) return l[i]; return null; }
    function ltext(v, e) {
      if (v === "responses") return [e.name, e.role, e.lang].concat(e.ex.map(function (x) { return x.trig + " " + x.resp; })).join(" ");
      if (v === "general") return [e.name, e.note].concat(e.slides.map(function (x) { return x.kind === "image" ? x.file + " " + x.alt : x.h + " " + x.b + " " + x.f; })).join(" ");
      return [e.name, e.label, e.langs.join(" "), e.notes].join(" ");
    }
    function lvisible(v) {
      var q = norm(S.lq[v]);
      return S.lib[v].filter(function (e) {
        if (e.archived && !S.larch[v]) return false;
        if (v === "responses" && ((S.lf.lang && e.lang !== S.lf.lang) || (S.lf.role && e.role !== S.lf.role))) return false;
        return !q || norm(ltext(v, e)).indexOf(q) >= 0;
      }).sort(function (a, b) { return (a.name || "~").localeCompare(b.name || "~"); });
    }
    function repertoire(name) {
      return S.songs.filter(function (s) { return !s.archived; }).map(function (s) { return { s: s, arrs: s.arrs.filter(function (a) { return !a.archived && a.choirs.indexOf(name) >= 0; }) }; })
        .filter(function (r) { return r.arrs.length; });
    }
    function respHeader(e) { return e.hdr === "inherit" ? HDR_DEFAULT[e.lang] || "" : e.hdr === "override" ? e.hdrText : ""; }
    function incomplete(v, e) {
      if (v === "responses") return !e.ex.length || e.ex.some(function (x) { return !x.resp.trim(); }) ? "A response is empty" : null;
      if (v === "general") return !e.slides.length ? "No Slides" : e.slides.some(function (x) { return x.kind === "image" ? !x.file : !x.b.trim(); }) ? "A Slide is empty" : null;
      return null;
    }
    function lnew(v) {
      var snap = snapshot(), id = v.charAt(0) + Date.now() % 100000;
      if (v === "choirs") { S.newChoir = ""; S.focus = "lv.newchoir"; return; }
      S.lib[v].push(v === "responses" ? { id: id, name: "", role: S.lf.role || "Other", lang: S.lf.lang || "Cebuano", hdr: "inherit", hdrText: "", plans: 0, templates: 0, ex: [ex("P", "", "")] } :
        { id: id, name: "", note: "", dur: 8, plans: 0, slides: [tx("", "", "")] });
      S.lsel[v] = id; S.lq[v] = ""; S.focus = "lvname"; S.lslide = 0;
      done("Created a " + LV[v][1] + ". Name it, then write its " + (v === "responses" ? "exchanges." : "Slides."), snap);
    }
    function choirTaken(name, except) { var n = norm(name); return S.lib.choirs.filter(function (c) { return c.id !== except && norm(c.name) === n; })[0] || null; }
    function addChoir() {
      var name = (S.newChoir || "").trim(); if (!name || choirTaken(name)) return;
      var snap = snapshot(), c = addChoirRec(name); S.lsel.choirs = c.id; S.newChoir = null; S.lq.choirs = "";
      done("Added the choir " + name + ". Everything else about it is optional.", snap);
    }
    function commitLName(v) {
      var e = lentry(), name = v.trim(), old = e.name; S.lnameDraft = null;
      if (name === old) return;
      if (!name) { done("A name is required. Kept " + (old || "the old name") + "."); return; }
      if (S.lview === "choirs" && choirTaken(name, e.id)) { done("A choir named " + name + " already exists. Kept " + old + "."); return; }
      var snap = snapshot(); e.name = name;
      if (S.lview === "choirs") S.songs.forEach(function (s) { s.arrs.forEach(function (a) { var i = a.choirs.indexOf(old); if (i >= 0) a.choirs[i] = name; }); });
      done(old ? "Renamed " + old + " to " + name + (S.lview === "choirs" ? ". Its Arrangements and Mass plans show the new name." : ".") : "Named it " + name + ".", snap);
    }
    function larchive() {
      var e = lentry(), snap = snapshot(), was = e.archived; e.archived = !was; S.popover = null;
      if (!was && !S.larch[S.lview]) S.larch[S.lview] = true;   // keep the archived entry in view
      done((was ? "Restored " : "Archived ") + (e.name || "Untitled") + "." + (was ? "" : S.lview === "choirs" ? " Selectors stop suggesting it; Arrangements and Mass plans keep it." : " Mass plans that use it keep it."), snap);
    }
    function lduplicate() {
      var e = lentry(), snap = snapshot(), c = copy(e); c.id = S.lview.charAt(0) + Date.now() % 100000; c.name = e.name + " (copy)"; c.plans = 0; c.templates = 0; c.behind = 0;
      S.lib[S.lview].push(c); S.lsel[S.lview] = c.id; S.popover = null; done("Duplicated " + (e.name || "Untitled") + ". The copy isn't used anywhere yet.", snap);
    }
    function lmove(list, i, d, what) { var snap = snapshot(), x = list[i]; list[i] = list[i + d]; list[i + d] = x; done("Moved " + what + " " + (i + 1) + " to " + (i + d + 1) + ".", snap); }
    function lremove(list, i, what) { var snap = snapshot(); list.splice(i, 1); done("Removed " + what + " " + (i + 1) + ".", snap); }
    function lset(field, i, value) {
      var e = lentry(), p = field.split(".");
      if (p[0] === "ex") e.ex[i][p[1]] = value;
      else if (p[0] === "sl") e.slides[i][p[1]] = p[1] === "dur" ? (value === "" ? null : +value) : value;
      else if (field === "dur") e.dur = +value || 8;
      else e[field] = value;
    }

    function lfield(field, value, o) {
      o = o || {}; var i = o.i != null ? ' data-i="' + o.i + '"' : "", key = "lv." + field + (o.i != null ? "." + o.i : "");
      var attrs = ' data-lvin="' + field + '"' + i + ' data-focus-key="' + key + '"' + (o.ro ? " readonly" : ' placeholder="' + esc(o.ph || "") + '"') + (o.label ? ' aria-label="' + esc(o.label) + '"' : "");
      return o.rows ? '<textarea class="pl-input" rows="' + o.rows + '"' + attrs + ">" + esc(value) + "</textarea>" :
        '<input class="pl-input' + (o.small ? " sg-small" : "") + '"' + (o.type ? ' type="' + o.type + '"' : "") + attrs + ' value="' + esc(value) + '">';
    }
    function lselect(field, value, opts, o) {
      o = o || {};
      return '<select class="pl-input pl-input--sel' + (o.small ? " sg-small" : "") + '" data-lvin="' + field + '"' + (o.i != null ? ' data-i="' + o.i + '"' : "") + (o.ro ? " disabled" : "") + ' aria-label="' + esc(o.label || field) + '">' +
        opts.map(function (x) { var val = Array.isArray(x) ? x[0] : x, lab = Array.isArray(x) ? x[1] : x; return '<option value="' + esc(val) + '"' + (val === value ? " selected" : "") + ">" + esc(lab) + "</option>"; }).join("") + "</select>";
    }
    function frow(label, html, hint) { return '<div class="pl-frow"><span class="o-label">' + label + "</span><div>" + html + (hint ? '<div class="o-meta">' + hint + "</div>" : "") + "</div></div>"; }
    function lacts(kind, i, n, what) {
      return '<span class="pl-acts"><button class="o-btn o-btn--quiet o-btn--icon" data-lv="' + kind + '-mv" data-i="' + i + '" data-d="-1" aria-label="Move ' + what + ' up" title="Move up"' + (i ? "" : " disabled") + ">" + icon("arrow-up") + "</button>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-lv="' + kind + '-mv" data-i="' + i + '" data-d="1" aria-label="Move ' + what + ' down" title="Move down"' + (i < n - 1 ? "" : " disabled") + ">" + icon("arrow-down") + "</button>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-lv="' + kind + '-del" data-i="' + i + '" aria-label="Remove ' + what + '" title="Remove"' + (n > 1 ? "" : " disabled") + ">" + icon("x") + "</button></span>";
    }
    function lbrowser(v) {
      var all = S.lib[v], rows = lvisible(v), noun = LV[v][1], list;
      var newRow = v === "choirs" && S.newChoir != null ? (function () {
        var taken = S.newChoir.trim() && choirTaken(S.newChoir);
        return '<div class="sg-newrow"><input class="pl-input" data-lvin="newchoir" data-focus-key="lv.newchoir" value="' + esc(S.newChoir) + '" placeholder="Choir name" aria-label="New choir name">' +
          '<button class="o-btn o-btn--primary" data-lv="add-choir"' + (S.newChoir.trim() && !taken ? "" : " disabled") + ">Add</button><button class=\"o-btn\" data-lv=\"cancel-choir\">Cancel</button>" +
          (taken ? '<div class="sg-error">' + icon("circle-alert") + esc(taken.name) + (taken.archived ? " already exists and is archived." : " already exists.") + ' <button class="pl-link" data-lv="sel" data-id="' + taken.id + '">Select it</button></div>' :
            '<div class="o-meta">Only a name is needed now.</div>') + "</div>";
      })() : "";
      if (!all.length) list = '<div class="sg-empty-list"><b>No ' + (v === "choirs" ? "choirs" : LV[v][0].toLowerCase()) + ' yet</b><div class="o-meta">' +
        (v === "choirs" ? "Choirs help you find the Arrangements each one sings and fill in Mass plans. They never hold their own copy of the lyrics." : "Entries you create here can be linked into any Mass.") + "</div>" +
        (S.newChoir == null ? '<button class="o-btn o-btn--primary" data-lv="new">' + icon("plus") + (v === "choirs" ? "Add choir" : "New " + noun) + "</button>" : "") + "</div>";
      else if (!rows.length) list = '<div class="sg-empty-list"><b>Nothing matches</b><div class="o-meta">Nothing matches ' + (S.lq[v] ? "“" + esc(S.lq[v]) + "”" : "these filters") + '.</div><button class="o-btn" data-lv="clear">Clear search and filters</button></div>';
      else list = rows.map(function (e) {
        var inc = incomplete(v, e), meta = v === "choirs" ? [e.label, e.langs.join(", ")].filter(Boolean).join(" · ") || "No details yet" : v === "responses" ? e.role + " · " + e.lang : e.note || "—";
        var count = v === "choirs" ? (function () { var n = repertoire(e.name).length; return n + (n === 1 ? " song" : " songs"); })() : e.plans ? e.plans + (e.plans === 1 ? " Mass" : " Masses") : "Unused";
        return '<button class="o-row sg-result" data-lv="sel" data-id="' + e.id + '"' + (e.id === S.lsel[v] ? ' aria-selected="true"' : "") + '><span class="o-grow"><b class="o-ellipsis">' + esc(e.name || "Untitled") + "</b>" +
          '<span class="o-meta o-ellipsis">' + esc(meta) + "</span></span>" + (inc ? statusLabel("warning", "Incomplete", "diamond") : "") + '<span class="o-meta sg-count">' + count + "</span>" +
          (e.archived ? '<span class="pl-chip">Archived</span>' : "") + "</button>";
      }).join("");
      var filters = v === "responses" ? '<div class="sg-filters sg-filters--2">' + lselect("lf.lang", S.lf.lang, [["", "All languages"]].concat(S.langs), { label: "Language" }) +
        lselect("lf.role", S.lf.role, [["", "All roles"]].concat(ROLES), { label: "Liturgical role" }) + "</div>" : "";
      return '<section class="o-pane sg-browser" aria-label="' + LV[v][0] + '"><div class="sg-browser-head"><div class="sg-row"><h2 class="o-h2">' + LV[v][0] + '</h2><span class="o-grow"></span>' +
        '<button class="o-btn o-btn--primary" data-lv="new">' + icon("plus") + (v === "choirs" ? "Add choir" : "New") + "</button></div>" +
        '<div class="o-field">' + icon("search") + '<input data-lvin="lq" data-focus-key="lv.q" placeholder="Search ' + (v === "choirs" ? "names, labels, notes" : v === "responses" ? "names, roles, words" : "names and Slide text") + '" value="' + esc(S.lq[v]) + '" aria-label="Search"></div>' + filters +
        '<div class="sg-row o-meta"><span>' + (all.length ? rows.length + " shown" : "") + '</span><span class="o-grow"></span>' +
        '<button class="pl-link" data-lv="archived">' + (S.larch[v] ? "Hide archived" : "Show archived") + "</button><span>Name A–Z</span></div></div>" +
        '<div class="o-pane-body sg-results">' + newRow + list + "</div></section>";
    }
    function ldetailHead(v, e, extra) {
      var nm = S.lnameDraft != null ? S.lnameDraft : e.name;
      var name = e.archived ? '<h2 class="o-title">' + esc(e.name) + "</h2>" : '<label class="sg-secname sg-lvname' + (S.lnameDraft != null ? " is-editing" : "") + '" title="Name · Enter saves, Esc cancels"><input data-lvname data-focus-key="lvname" value="' + esc(nm) + '" size="' + Math.max(8, nm.length + 1) + '" placeholder="Name this ' + LV[v][1] + '" aria-label="Name">' + icon("pencil") + "</label>";
      var menu = S.popover && S.popover.kind === "lv-menu" ? '<div class="o-menu sg-overflow" role="menu">' + (v === "choirs" ? "" : '<button class="o-row" role="menuitem" data-lv="duplicate">' + icon("copy") + "Duplicate</button>") +
        '<button class="o-row" role="menuitem" data-lv="archive">' + icon(e.archived ? "archive-restore" : "archive") + (e.archived ? "Restore" : "Archive") + "</button></div>" : "";
      return '<div class="sg-detail-head"><div class="o-grow"><div class="o-meta">' + LV[v][1].charAt(0).toUpperCase() + LV[v][1].slice(1) + "</div>" + name + "</div>" + (extra || "") + (e.archived ? "" : saved()) +
        '<button class="o-btn o-btn--icon" data-lv="menu" aria-label="More actions" title="More">' + icon("ellipsis") + "</button></div>" + menu;
    }
    function usesNote(e) {
      if (!e.plans) return '<div class="sg-uses o-meta">' + icon("link") + "<span>Not linked in any Mass plan yet.</span></div>";
      return '<div class="sg-uses o-meta">' + icon("link") + "<span>Linked in " + e.plans + (e.plans === 1 ? " Mass plan" : " Mass plans") + (e.templates ? " and " + e.templates + (e.templates === 1 ? " template" : " templates") : "") +
        ". Each change saves a new revision; those plans keep the one they use and show <b>Update available</b> until someone reviews it." + (e.behind ? " " + e.behind + " of them already do." : "") + "</span></div>";
    }
    function archivedNote(v) {
      return '<div class="o-notice" data-state="info">' + icon("archive") + '<div class="o-grow"><b>Archived.</b> ' + (v === "choirs" ? "Choir selectors don’t suggest it. Arrangements and Mass plans that name it keep it." : "Inspect only. Mass plans that use it keep it; restore it to edit or link it again.") +
        '</div><button class="o-btn o-btn--primary" data-lv="archive">Restore</button></div>';
    }
    function respDetail(e) {
      var ro = !!e.archived, hd = respHeader(e), inc = incomplete("responses", e);
      var langs = S.langs.indexOf(e.lang) >= 0 ? S.langs : S.langs.concat([e.lang]);
      var blocks = e.ex.map(function (x, i) {
        return '<div class="pl-exblock"><div class="pl-block-head"><b>Exchange ' + (i + 1) + "</b>" + (ro ? "" : lacts("ex", i, e.ex.length, "exchange")) + "</div>" +
          '<div class="pl-ex-line">' + lselect("ex.who", x.who, WHO, { i: i, ro: ro, label: "Prompt role" }) + lfield("ex.trig", x.trig, { i: i, ro: ro, ph: "Optional private prompt", label: "Prompt" }) + '<span class="pl-chip">Operator only</span></div>' +
          '<div class="pl-ex-line">' + lselect("ex.rwho", x.rwho, RWHO, { i: i, ro: ro, label: "Response role" }) + lfield("ex.resp", x.resp, { i: i, ro: ro, rows: 2, ph: "Required congregation response", label: "Response" }) + '<span class="pl-chip">Projected</span></div>' +
          (x.resp.trim() ? "" : '<div class="sg-error">' + icon("circle-alert") + "Every exchange needs the congregation’s words.</div>") + "</div>";
      }).join("");
      var cards = e.ex.filter(function (x) { return x.resp.trim(); }).map(function (x) { return { k: "response", h: hd, b: x.resp }; });
      return '<section class="o-pane sg-detail" aria-label="Selected Response or Prayer">' + ldetailHead("responses", e, inc ? statusLabel("warning", "Incomplete", "diamond") : "") +
        '<div class="o-pane-body sg-detail-body">' + (ro ? archivedNote("responses") : "") + usesNote(e) +
        '<div class="pl-form">' + frow("Liturgical role", lselect("role", e.role, ROLES, { ro: ro, label: "Liturgical role" }), "What it does in the Mass. Responses in other languages are separate entries, never kept in step.") +
        frow("Language", lselect("lang", e.lang, langs, { ro: ro, label: "Language" })) +
        frow("Response header", '<div class="pl-inline">' + lselect("hdr", e.hdr, [["inherit", "Mass plan default" + (HDR_DEFAULT[e.lang] ? " · " + HDR_DEFAULT[e.lang] : "")], ["hide", "Hide"], ["override", "Override"]], { ro: ro, label: "Response header" }) +
          (e.hdr === "override" ? lfield("hdrText", e.hdrText, { ro: ro, small: true, ph: "TUBAG:", label: "Header text" }) : "") + "</div>", "Shown above every response Slide. Each Mass item can still inherit, hide or override it.") + "</div>" +
        '<div class="pl-block"><div class="pl-block-head"><b>Exchanges</b><span class="o-meta">' + e.ex.length + " · prompt, then response, in Presenter</span></div>" + blocks +
        (ro ? "" : '<button class="o-btn" data-lv="ex-add" data-focus-key="lv.exadd">' + icon("plus") + "Add exchange</button>") + "</div>" +
        '<div class="pl-block-head"><b>Generated slides</b><span class="o-meta">' + cards.length + (cards.length === 1 ? " slide" : " slides") + " · prompts stay on the operator’s screen</span></div>" +
        (cards.length ? thumbs(cards, -1, "data-lvslide") : '<div class="o-meta">No slides yet: write a response.</div>') + "</div></section>";
    }
    function imageFrame(x) { return '<div class="o-slide pl-imgslide"><div class="pl-img">' + icon("image") + "<span>" + esc(x.file || "No image chosen") + "</span></div></div>"; }
    function gpDetail(e) {
      var ro = !!e.archived, inc = incomplete("general", e);
      var slides = e.slides.map(function (x, i) {
        var fields = x.kind === "image" ?
          frow("Image", '<div class="pl-inline">' + lfield("sl.file", x.file, { i: i, ro: ro, ph: "photo.jpg", label: "Image file" }) + (ro ? "" : '<button class="o-btn">' + icon("upload") + "Choose…</button>") + "</div>", "Fitted inside the safe area; never cropped.") +
            frow("Description", lfield("sl.alt", x.alt, { i: i, ro: ro, ph: "What the image shows", label: "Description" }), "For the operator and screen readers; not projected.") :
          frow("Header", lfield("sl.h", x.h, { i: i, ro: ro, ph: "Optional", label: "Header" })) + frow("Text", lfield("sl.b", x.b, { i: i, ro: ro, rows: 3, ph: "Slide text", label: "Text" })) +
            frow("Footer", lfield("sl.f", x.f, { i: i, ro: ro, ph: "Optional", label: "Footer" }));
        return '<div class="pl-slide' + (i === S.lslide ? " is-current" : "") + '"><button class="pl-slide-thumb" data-lv="slide" data-i="' + i + '" aria-label="Select Slide ' + (i + 1) + '">' + (x.kind === "image" ? imageFrame(x) : frame({ k: "general", h: x.h, b: x.b || " ", f: x.f })) + "</button>" +
          '<div class="pl-form o-grow"><div class="pl-block-head"><b>Slide ' + (i + 1) + " · " + (x.kind === "image" ? "Image" : "Text") + "</b>" + (ro ? "" : lacts("sl", i, e.slides.length, "Slide")) + "</div>" + fields +
          frow("Auto-play", '<div class="pl-inline">' + lfield("sl.dur", x.dur == null ? "" : x.dur, { i: i, ro: ro, small: true, type: "number", ph: String(e.dur), label: "Seconds" }) + '<span class="o-meta">seconds' + (x.dur == null ? " · uses the default" : "") + "</span></div>") + "</div></div>";
      }).join("");
      return '<section class="o-pane sg-detail" aria-label="Selected General Presentation">' + ldetailHead("general", e, inc ? statusLabel("warning", "Incomplete", "diamond") : "") +
        '<div class="o-pane-body sg-detail-body">' + (ro ? archivedNote("general") : "") + usesNote(e) +
        '<div class="pl-form">' + frow("Purpose", lfield("note", e.note, { ro: ro, ph: "Optional, such as Weekly announcements", label: "Purpose" }), "Helps find it later; never projected.") +
        frow("Default auto-play", '<div class="pl-inline">' + lfield("dur", e.dur, { ro: ro, small: true, type: "number", label: "Default seconds" }) + '<span class="o-meta">seconds per Slide</span></div>', "Only used when the operator starts auto-play (A in Presenter). A Slide can override it.") + "</div>" +
        '<div class="pl-block"><div class="pl-block-head"><b>Slides</b><span class="o-meta">' + e.slides.length + "</span></div>" + slides +
        (ro ? "" : '<div class="pl-banner-acts"><button class="o-btn" data-lv="sl-text" data-focus-key="lv.sladd">' + icon("type") + 'Add text Slide</button><button class="o-btn" data-lv="sl-image">' + icon("image") + "Add image Slide</button></div>") +
        '<div class="o-meta">Notes for whoever reads it aloud belong to each Mass item, so they’re written in the Mass, not here.</div></div></div></section>';
    }
    function choirDetail(e) {
      var ro = !!e.archived, rep = repertoire(e.name), n = rep.reduce(function (k, r) { return k + r.arrs.length; }, 0);
      var langOpen = S.pick && S.pick.key === "langs" && S.pick.target === e;
      var langs = '<div class="sg-chipset">' + e.langs.map(function (l) { return '<button class="sg-toggle" aria-pressed="true" data-lv="lang-off" data-lang="' + esc(l) + '"' + (ro ? " disabled" : "") + ">" + icon("check") + esc(l) + (ro ? "" : " " + icon("x")) + "</button>"; }).join("") +
        (ro || langOpen ? "" : '<button class="sg-toggle sg-toggle--add" data-act="pick" data-key="langs" data-target="choir">' + icon("plus") + "Language</button>") + "</div>" + (langOpen ? pickerHtml("langs") : "");
      var list = rep.length ? '<div class="sg-arrs">' + rep.map(function (r) {
        return '<div class="sg-rep"><div class="sg-row"><b>' + esc(r.s.title) + '</b><span class="o-meta">' + esc(r.s.by) + "</span></div>" + r.arrs.map(function (a) {
          var others = a.choirs.filter(function (c) { return c !== e.name; });
          return '<div class="sg-arr is-compact"><div class="o-grow"><div class="sg-row"><span>' + esc(arrLabel(a)) + "</span>" + status(readiness(a)) + "</div>" +
            (others.length ? '<div class="o-meta">Shared with ' + esc(others.join(", ")) + "</div>" : "") + '</div><button class="o-btn" data-lv="open" data-song="' + r.s.id + '" data-arr="' + a.id + '">Open</button></div>';
        }).join("") + "</div>";
      }).join("") + "</div>" :
        '<div class="pl-empty">' + icon("list-music", "o-icon-20") + "<b>" + (ro ? "No active Arrangements name this choir" : "No Arrangements name this choir yet") + "</b>" + (ro ? "" : "<div class=\"o-meta\">Add the choir in an Arrangement’s details, under Choirs.</div>") + "</div>";
      return '<section class="o-pane sg-detail" aria-label="Selected choir">' + ldetailHead("choirs", e, e.archived ? '<span class="pl-chip">Archived</span>' : "") +
        '<div class="o-pane-body sg-detail-body">' + (ro ? archivedNote("choirs") : "") +
        '<div class="pl-form">' + frow("Short label", lfield("label", e.label, { ro: ro, small: true, ph: "Optional, such as KSR", label: "Short label" }), "Used where space is tight, such as the Masses list.") +
        frow("Preferred languages", langs, "The languages it usually sings in. Its Arrangements in those languages are suggested first.") +
        frow("Private notes", lfield("notes", e.notes, { ro: ro, rows: 2, ph: "Optional", label: "Private notes" }), "For the people preparing Masses. Never projected.") + "</div>" +
        '<div class="pl-block-head"><b>Repertoire</b><span class="o-meta">' + rep.length + (rep.length === 1 ? " song · " : " songs · ") + n + (n === 1 ? " Arrangement" : " Arrangements") + "</span></div>" + list +
        '<div class="o-meta">Members, rehearsals and contacts aren’t kept in Ordo.</div></div></section>';
    }
    function libView() {
      var v = S.lview, e = lentry(v), detail;
      if (!S.lib[v].length) detail = '<section class="o-pane sg-detail"><div class="o-pane-body sg-guide">' + (v === "choirs" ?
        '<h2 class="o-h2">Choirs are optional</h2><ol><li><b>Songs and Masses work without them.</b> An Arrangement or Mass can stay Not assigned.</li><li><b>Add a choir by name.</b> A label, preferred languages and notes can wait.</li><li><b>Name it on its Arrangements.</b> Its repertoire then fills in here.</li></ol>' :
        '<div class="o-meta">Nothing here yet.</div>') + "</div></section>";
      else if (!e) detail = '<section class="o-pane sg-detail"><div class="o-pane-body sg-guide"><div class="o-meta">Select an entry to see it.</div></div></section>';
      else detail = v === "choirs" ? choirDetail(e) : v === "responses" ? respDetail(e) : gpDetail(e);
      return '<div class="sg-lib">' + libHead() + '<div class="sg-panes">' + lbrowser(v) + detail + "</div></div>";
    }

    // ── overlays ──────────────────────────────────────────────────────────────────────────────
    function newSong() {
      var f = S.form, d = duplicates(), edit = S.overlay === "songdetails", s = song(), n = edit ? s.arrs.filter(function (a) { return !a.archived; }).length : 0;
      var dupHtml = d.length ? '<div class="sg-dups"><button class="pl-link" data-act="dups" aria-expanded="' + S.dupOpen + '">' + icon(S.dupOpen ? "chevron-down" : "chevron-right") + d.length + (d.length === 1 ? " possible existing Song" : " possible existing Songs") + "</button>" +
        (S.dupOpen ? '<div class="sg-dup-list">' + d.map(function (s) { return '<button class="o-row" data-dup="' + s.id + '"><span class="o-grow"><b>' + esc(s.title) + '</b><span class="o-meta">' + esc(s.by) + " · " + s.arrs.length + " Arrangements</span></span>" + icon("chevron-right") + "</button>"; }).join("") + "</div>" : "") + "</div>" : "";
      return '<div class="o-scrim"></div><div class="o-slideover sg-form' + (S.child ? " is-receded" : "") + '" role="dialog" aria-modal="true" aria-labelledby="sg-ns-t">' + closeTab(edit ? "Cancel" : "Cancel new song") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="sg-ns-t">' + (edit ? "Song details" : "New song") + '</h2><div class="o-meta">' + (edit ? esc(s.title) + " · the composition itself." : "The composition itself.") + " Lyrics, language and choirs belong to its Arrangements.</div></div></div>" +
        '<div class="o-overlay-body">' +
        (edit && n ? '<div class="o-notice" data-state="info">' + icon("link") + "<div><b>Shown by " + n + (n === 1 ? " Arrangement." : " Arrangements.") + "</b> The title and credits appear on each of them and in the Mass plans that use this Song. Past Presentation snapshots don’t change.</div></div>" : "") +
        '<div class="pl-form">' +
        '<div class="pl-frow"><span class="o-label">Primary title <span class="o-meta">required</span></span><div>' + nsField("title", f.title, "Ginoo, Kaloy-i Kami") + dupHtml + "</div></div>" +
        '<div class="pl-frow"><span class="o-label">Composer or original author</span><div>' + nsField("by", f.by, "Optional") + "</div></div>" +
        '<div class="pl-frow"><span class="o-label">Aliases</span><div>' + nsField("aliases", f.aliases, "Optional, separated by commas") + '<div class="o-meta">Other names people search for, such as Kyrie.</div></div></div>' +
        '<div class="pl-frow"><span class="o-label">Usual moment</span><div>' + nsField("moments", f.moments, "Optional, such as Entrance or Communion") + "</div></div></div></div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-act="close">Cancel</button>' + (edit ?
          '<button class="o-btn o-btn--primary" data-act="save-song"' + (f.title.trim() && songChanged() ? "" : " disabled") + ">Save details</button>" :
          '<button class="o-btn o-btn--primary" data-act="create-song"' + (f.title.trim() ? "" : " disabled") + ">Create song</button>") + "</div></div>" +
        (S.child === "dup" ? dupPreview() : "");
    }
    function nsField(k, v, ph) { return '<input class="pl-input" data-input="ns" data-key="' + k + '" data-focus-key="ns.' + k + '" value="' + esc(v) + '" placeholder="' + esc(ph) + '">'; }
    function dupPreview() {
      var s = song(S.dup);
      return '<div class="o-slideover pl-child sg-child" role="dialog" aria-modal="true" aria-labelledby="sg-dp-t"><div class="o-overlay-head"><button class="o-btn o-btn--quiet o-btn--icon" data-act="back-child" aria-label="Back to New song" title="Back">' + icon("arrow-left") + "</button>" +
        '<div class="o-grow"><h2 class="o-h2" id="sg-dp-t">' + esc(s.title) + '</h2><div class="o-meta">Existing Song · preview only</div></div></div>' +
        '<div class="o-overlay-body"><dl class="sg-meta">' + [["Composer / original author", s.by], ["Aliases", s.aliases.join(", ") || "—"], ["Usual moments", s.moments.join(", ") || "—"]].map(function (m) { return "<div><dt>" + m[0] + "</dt><dd>" + esc(m[1]) + "</dd></div>"; }).join("") + "</dl>" +
        '<div class="pl-block-head"><b>Arrangements</b></div>' + s.arrs.map(function (a) { return '<div class="sg-arr is-compact"><div class="o-grow"><b>' + esc(arrLabel(a)) + '</b><div class="o-meta">' + esc(a.langs.join(", ")) + " · " + (a.choirs.join(", ") || "Choir unspecified") + "</div></div></div>"; }).join("") + "</div>" +
        '<div class="o-overlay-foot">' + (S.overlay === "songdetails" ? '<span class="o-meta o-grow">Merging two Songs comes after the MVP.</span><button class="o-btn" data-act="back-child">Back</button>' :
          '<button class="o-btn" data-act="back-child">Back</button><button class="o-btn o-btn--primary" data-act="use-existing">Use this existing Song</button>') + "</div></div>";
    }
    function pickerHtml(key) {
      var rows = pickRows(), choir = key === "choirs";
      return '<div class="sg-pick"><div class="o-field">' + icon("search") + '<input data-input="pick" data-focus-key="pick" value="' + esc(S.pick.q) + '" placeholder="' + (choir ? "Find or create a choir" : "Find a language") + '" aria-label="' + (choir ? "Choir" : "Language") + '"></div>' +
        '<div class="sg-pick-list" role="listbox">' + (rows.length ? rows.map(function (r, i) {
          return r.create ? '<button class="o-row" role="option" data-act="create-choir"' + (i ? "" : ' aria-selected="true"') + ">" + icon("plus") + '<span class="o-grow">Create “' + esc(r.create) + '”</span><span class="o-meta">New choir · name only</span></button>' :
            '<button class="o-row" role="option" data-pickval="' + esc(r.val) + '"' + (i ? "" : ' aria-selected="true"') + '><span class="o-grow">' + esc(r.label) + "</span>" + (r.also ? '<span class="o-meta">also called ' + esc(r.also) + "</span>" : "") + "</button>";
        }).join("") : '<div class="o-meta sg-pick-none">' + (choir ? "Every choir is already chosen." : "No language in Ordo’s list matches “" + esc(S.pick.q) + "”.") + "</div>") + "</div>" +
        '<div class="sg-pick-foot o-meta"><span class="o-grow">' + (choir ? "Archived choirs aren’t suggested." : "Languages come from Ordo’s list, so every Arrangement and Mass names a language the same way.") + '</span><button class="pl-link" data-act="pick-cancel">Done</button></div></div>';
    }
    function details() {
      var f = S.form, s = song(), a = f.create ? null : arrangement();
      function chipset(key, values, extra) {
        var open = S.pick && S.pick.key === key;
        return '<div class="sg-chipset">' + values.map(function (v) { var on = f[key].indexOf(v) >= 0; return '<button class="sg-toggle" data-chip="' + key + ":" + esc(v) + '" aria-pressed="' + on + '">' + (on ? icon("check") : "") + esc(v) + "</button>"; }).join("") +
          (open ? "" : '<button class="sg-toggle sg-toggle--add"' + (key === "occ" ? "" : ' data-act="pick" data-key="' + key + '"') + ">" + icon("plus") + esc(extra) + "</button>") + "</div>" + (open ? pickerHtml(key) : "");
      }
      var shared = !f.create && a.choirs.length > 1;
      return '<div class="o-scrim"></div><div class="o-slideover sg-form" role="dialog" aria-modal="true" aria-labelledby="sg-ad-t">' + closeTab("Cancel") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="sg-ad-t">' + (f.create ? "New Arrangement" : "Arrangement details") + '</h2><div class="o-meta">' + esc(s.title) + (f.create ? "" : " · " + esc(arrLabel(a))) + "</div></div></div>" +
        '<div class="o-overlay-body">' +
        (shared ? '<div class="o-notice" data-state="info">' + icon("users") + "<div><b>Shared by " + esc(a.choirs.join(" and ")) + ".</b> Saving updates this one Arrangement for every choir that uses it. Mass plans keep their references; past Presentation snapshots don’t change.</div></div>" : "") +
        '<div class="pl-form">' +
        '<div class="pl-frow"><span class="o-label">Display title</span><div><input class="pl-input" data-input="ad" data-key="title" data-focus-key="ad.title" value="' + esc(f.title) + '" placeholder="Leave blank to show ' + esc(arrLabel({ title: "", langs: f.langs.length ? f.langs : ["Language"], choirs: f.choirs })) + '"></div></div>' +
        '<div class="pl-frow"><span class="o-label">Languages <span class="o-meta">at least one</span></span><div>' + chipset("langs", S.langs, "Language") +
          (f.tried && !f.langs.length ? '<div class="sg-error">' + icon("circle-alert") + "Choose the language this rendition is sung in.</div>" : '<div class="o-meta">Several only when this exact rendition mixes them.</div>') + "</div></div>" +
        '<div class="pl-frow"><span class="o-label">Choirs</span><div>' + chipset("choirs", choirNames(), "Choir") +
          (S.made ? '<div class="sg-made">' + icon("check") + "<span>Created the choir " + esc(S.made) + ". Add its short label and preferred languages later in Library → Choirs.</span></div>" :
            '<div class="o-meta">Choirs that sing this exact rendition. Different wording or order needs its own Arrangement.</div>') + "</div></div>" +
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
      var p = S.popover; if (!p || p.kind !== "add") return "";
      var sugg = ["Verse 1", "Bersikulo 1", "Koro", "Refrain", "Bridge", "Katapusan"];
      return '<div class="o-menu sg-pop" role="dialog" aria-label="Add section"><div class="sg-pop-body"><label class="o-label" for="sg-pop-name">Section name</label>' +
        '<input id="sg-pop-name" class="pl-input" data-input="pop" data-focus-key="pop.name" value="' + esc(p.name) + '" placeholder="Name this part">' +
        '<div class="sg-chipset">' + sugg.map(function (x) { return '<button class="sg-toggle" data-sugg="' + esc(x) + '">' + esc(x) + "</button>"; }).join("") + "</div>" +
        '<div class="sg-row"><span class="o-grow"></span><button class="o-btn" data-act="pop-cancel">Cancel</button><button class="o-btn o-btn--primary" data-act="pop-ok"' + (p.name.trim() ? "" : " disabled") + ">Add section</button></div></div></div>";
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
      } else if (S.lview !== "songs") body = libView();
      else body = '<div class="sg-lib">' + libHead() + '<div class="sg-panes">' + browser() + detail() + "</div></div>";
      if (S.overlay === "newsong" || S.overlay === "songdetails") ov = newSong();
      else if (S.overlay === "details") ov = details();
      else if (S.overlay === "history") ov = history();
      var toast = S.toast ? '<div class="o-toast" role="status">' + esc(S.toast) + (S.undo ? '<button class="o-btn" data-act="undo">Undo</button>' : "") + "</div>" : "";
      app.innerHTML = '<div class="o-body">' + K.navrail("Library") + '<div class="o-grow sg-ws">' + body + "</div></div>" + popoverHtml() + ov + toast;
      K.icons();
      Object.keys(keep).forEach(function (q) { var el = app.querySelector(q); if (el) el.scrollTop = keep[q]; });
      var pop = app.querySelector(".sg-pop, .sg-overflow");
      if (pop) {
        var anchor = app.querySelector(S.popover.kind === "song-menu" ? '[data-act="song-menu"]' : S.popover.kind === "lv-menu" ? '[data-lv="menu"]' : '[data-anchor="addsec"]');
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
      var t = e.target.closest("[data-act],[data-song],[data-open],[data-sec],[data-slide],[data-tab],[data-occ],[data-expand],[data-addocc],[data-occmv],[data-occdel],[data-secmv],[data-chip],[data-sugg],[data-dup],[data-sess],[data-rev],[data-cmptab],[data-doc],[data-pickval],[data-lv],[data-lview]");
      if (!t || t.disabled) return;
      e.preventDefault();
      var d = t.dataset, a = d.act;
      if (a !== "undo") { S.toast = null; S.undo = null; }
      if (S.popover && (S.popover.kind === "song-menu" && a !== "song-menu" || S.popover.kind === "lv-menu" && d.lv !== "menu")) S.popover = null;
      if (d.lview) { S.lview = d.lview; S.pick = null; S.newChoir = null; render(); return; }
      if (d.lv) { lvClick(d); render(); return; }
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
      else if (a === "song-details") songForm();
      else if (a === "save-song") saveSong();
      else if (a === "pick") { S.pick = { key: d.key, q: "", target: d.target === "choir" ? lentry("choirs") : null }; S.focus = "pick"; }
      else if (a === "pick-cancel") S.pick = null;
      else if (d.pickval) pickValue(d.pickval);
      else if (a === "create-choir") createChoir(S.pick.q.trim());
      else if (a === "new-arr") detailsForm(true);
      else if (a === "details") detailsForm(false);
      else if (d.chip) { var p = d.chip.split(":"), l = S.form[p[0]], at = l.indexOf(p[1]); if (at >= 0) l.splice(at, 1); else l.push(p[1]); }
      else if (a === "save-details") saveDetails();
      else if (a === "song-menu") S.popover = S.popover && S.popover.kind === "song-menu" ? null : { kind: "song-menu" };
      else if (d.tab) { S.tab = d.tab; S.slide = 0; S.popover = null; }
      else if (d.sec) { S.sec = d.sec; S.slide = 0; S.popover = null; S.secDraft = null; }
      else if (d.slide != null) S.slide = +d.slide;
      else if (a === "add-sec") openPopover("add");
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
    function lvClick(d) {
      var v = S.lview, e = lentry(), i = +d.i, x = d.lv;
      if (x === "sel") { S.lsel[v] = d.id; S.lslide = 0; S.pick = null; S.lnameDraft = null; if (S.newChoir != null && d.id) S.newChoir = null; var t = lentry(); if (t && t.archived) S.larch[v] = true; }
      else if (x === "new") lnew(v);
      else if (x === "add-choir") addChoir();
      else if (x === "cancel-choir") S.newChoir = null;
      else if (x === "clear") { S.lq[v] = ""; S.lf = { lang: "", role: "" }; }
      else if (x === "archived") S.larch[v] = !S.larch[v];
      else if (x === "menu") S.popover = S.popover && S.popover.kind === "lv-menu" ? null : { kind: "lv-menu" };
      else if (x === "archive") larchive();
      else if (x === "duplicate") lduplicate();
      else if (x === "ex-add") { var snap = snapshot(); e.ex.push(ex("P", "", "")); S.focus = "lv.ex.resp." + (e.ex.length - 1); done("Added exchange " + e.ex.length + ".", snap); }
      else if (x === "ex-mv") lmove(e.ex, i, +d.d, "exchange");
      else if (x === "ex-del") lremove(e.ex, i, "exchange");
      else if (x === "sl-text" || x === "sl-image") { var sn = snapshot(); e.slides.push(x === "sl-image" ? { kind: "image", file: "", alt: "", dur: null } : tx("", "", "")); S.lslide = e.slides.length - 1;
        S.focus = "lv." + (x === "sl-image" ? "sl.file." : "sl.b.") + S.lslide; done("Added Slide " + e.slides.length + ".", sn); }
      else if (x === "sl-mv") { lmove(e.slides, i, +d.d, "Slide"); S.lslide = i + +d.d; }
      else if (x === "sl-del") { lremove(e.slides, i, "Slide"); S.lslide = Math.max(0, Math.min(S.lslide, e.slides.length - 1)); }
      else if (x === "slide") S.lslide = i;
      else if (x === "lang-off") { var at = e.langs.indexOf(d.lang); if (at >= 0) e.langs.splice(at, 1); }
      else if (x === "open") { S.lview = "songs"; S.song = d.song; openArr(d.arr); }
    }
    function lvInput(el) {
      var f = el.dataset.lvin, v = el.value;
      if (f === "lq") S.lq[S.lview] = v;
      else if (f === "lf.lang") S.lf.lang = v;
      else if (f === "lf.role") S.lf.role = v;
      else if (f === "newchoir") S.newChoir = v;
      else lset(f, +el.dataset.i, v);
    }
    app.addEventListener("input", function (e) {
      var el = e.target, k = el.dataset.input;
      if (el.dataset.lvin) { if (el.tagName !== "SELECT") { lvInput(el); render(); } return; }
      if (el.dataset.lvname != null) { S.lnameDraft = el.value; el.size = Math.max(8, el.value.length + 1); return; }
      if (k === "q") S.q = el.value;
      else if (k === "lyrics") { var s = section(); s.text = el.value; s.approved = false; var n = slides(s).length; S.slide = Math.min(S.slide, Math.max(0, n - 1)); }
      else if (k === "ns") S.form[el.dataset.key] = el.value;
      else if (k === "ad") S.form[el.dataset.key] = el.value;
      else if (k === "pop") S.popover.name = el.value;
      else if (k === "pick") S.pick.q = el.value;
      else if (el.dataset.secname != null) { S.secDraft = el.value; el.size = Math.max(4, el.value.length + 1); return; }   // no re-render while naming
      else return;
      render();
    });
    app.addEventListener("change", function (e) {
      var el = e.target;
      if (el.dataset.secname != null) { S.toast = null; S.undo = null; commitSecName(el.value); render(); }
      else if (el.dataset.lvname != null) { S.toast = null; S.undo = null; commitLName(el.value); render(); }
      else if (el.dataset.lvin && el.tagName === "SELECT") { lvInput(el); render(); }
      else if (el.dataset.filter) { S.f[el.dataset.filter] = el.value; render(); }
      else if (el.dataset.stage) { S.staged[el.dataset.stage] = el.value; render(); }
    });
    app.addEventListener("keydown", function (e) {
      if (e.target.dataset && e.target.dataset.lvin === "newchoir" && (e.key === "Enter" || e.key === "Escape")) {
        if (e.key === "Enter") addChoir(); else S.newChoir = null;
        e.preventDefault(); render(); return;
      }
      if (e.target.dataset && (e.target.dataset.secname != null || e.target.dataset.lvname != null) && (e.key === "Enter" || e.key === "Escape")) {
        var isSec = e.target.dataset.secname != null;
        if (e.key === "Escape") { if (isSec) S.secDraft = null; else S.lnameDraft = null; }
        else { S.toast = null; S.undo = null; if (isSec) commitSecName(e.target.value); else commitLName(e.target.value); }
        e.preventDefault(); render(); return;   // the re-rendered field starts clean, so blur commits nothing twice
      }
      if (e.target.dataset && e.target.dataset.input === "pick" && (e.key === "Enter" || e.key === "Escape")) {
        var first = e.key === "Enter" ? pickRows()[0] : null;
        if (e.key === "Escape") S.pick = null; else if (first && first.create) createChoir(first.create); else if (first) pickValue(first.val);
        e.preventDefault(); render(); return;
      }
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
      songDetails: function () { songForm(); return api; },
      lview: function (v, id) { S.lview = v; if (id) S.lsel[v] = id; return api; }, lsearch: function (q) { S.lq[S.lview] = q; return api; }, lfilter: function (k, val) { S.lf[k] = val; return api; },
      larchived: function () { S.larch[S.lview] = true; return api; }, lnew: function () { lnew(S.lview); S.toast = null; S.undo = null; return api; },
      newChoir: function (name) { S.newChoir = name || ""; return api; }, noChoirs: function () { S.lib.choirs = []; return api; },
      lset: function (field, i, value) { lset(field, i, value); return api; }, lmenu: function () { S.popover = { kind: "lv-menu" }; return api; },
      pickFor: function (q) { S.pick = { key: "langs", q: q || "", target: lentry("choirs") }; return api; },
      pick: function (key, q) { S.pick = { key: key, q: q || "" }; return api; }, createChoir: function () { createChoir(S.pick.q.trim()); return api; },
      createSong: function (title) { newSongForm(); S.form.title = title; createSong(); S.toast = null; S.undo = null; return api; },
      open: function (song, arr, tab) { S.song = song; openArr(arr, tab); return api; }, sec: function (id) { S.sec = id; S.slide = 0; return api; }, slide: function (i) { S.slide = i; return api; },
      occ: function (i) { S.occ = i; return api; }, expand: function (i) { S.open[i] = true; return api; },
      details: function (create) { detailsForm(!!create); return api; }, form: function (k, v) { S.form[k] = v; return api; }, tryCreate: function () { saveDetails(); return api; },
      createArr: function (o) { detailsForm(true); Object.keys(o || {}).forEach(function (k) { S.form[k] = o[k]; }); saveDetails(); S.toast = null; S.undo = null; return api; },
      addSection: function () { openPopover("add"); return api; }, secName: function (v) { S.secDraft = v; return api; },
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
