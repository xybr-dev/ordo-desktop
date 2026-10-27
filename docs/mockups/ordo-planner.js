// Ordo mockups: the Preparation Mass planner on the shared kit (load after ordo-kit.js).
// Each planner owns a working copy of the sample Mass, so its flows can add, replace, edit and
// customize items and Undo them without touching other mockups on the page. Mockup behaviour only.
(function () {
  "use strict";
  var K = window.OrdoKit, esc = K.esc, icon = K.icon, statusLabel = K.statusLabel, KIND = K.KIND_LABEL;
  KIND.blackout = "Blackout"; KIND.image = "Image"; KIND.general = KIND.general || "Text";
  function projectable(c) { return !!c && (c.k === "image" || K.projectable(c)); }
  function cueLabel(c) { return c.k === "image" ? "Image · " + c.file : K.cueLabel(c); }
  var OP_ONLY = { trigger: 1, wait: 1, hold: 1, blackout: 1 };
  var WHO = { P: "Priest", D: "Deacon", L: "Lector", C: "Cantor", K: "Commentator", T: "All", Ch: "Choir" };

  // ── Themes (Decision 95): canonical colours are Theme content, not operator chrome ───────────
  var THEMES = [
    { id: "ordo-black", name: "Ordo Black", note: "White on black. The protected fallback Theme." },
    { id: "parish-light", name: "Parish Light", note: "Dark text on warm paper, for a bright daytime nave." },
    { id: "ordinary-green", name: "Ordinary Time", note: "Cream text on deep green." },
    { id: "high-contrast", name: "High Contrast", note: "Yellow on black, for a long projector throw." }
  ];
  function themeName(id) { for (var i = 0; i < THEMES.length; i++) if (THEMES[i].id === id) return THEMES[i].name; return id; }
  function tslide(c, th) {
    if (c && c.k === "image") return '<div class="o-slide pl-imgslide" data-th="' + th + '"><div class="pl-img">' + icon("image") + "<span>" + esc(c.file || "No image chosen") + "</span></div></div>";
    return K.slide(c).replace('class="o-slide"', 'class="o-slide" data-th="' + th + '"');
  }

  // ── Library fixtures: Songs, Responses & Prayers, General Presentations (Decision 257) ─────
  function strip(c) { var o = {}; for (var k in c) if (k !== "item" && k !== "i" && k !== "ci") o[k] = c[k]; return o; }
  function copy(x) { return JSON.parse(JSON.stringify(x)); }
  function fromMass(title) { var r = null; K.MASS.sections.forEach(function (s) { s.items.forEach(function (it) { if (it.song === title || it.title === title) r = it.cues; }); }); return r; }
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

  // A Response sequence is ordered exchange blocks: an optional operator-only trigger, then one
  // required congregation response (Decision 111).
  function blocks(cues) {
    var out = [], cur = null;
    cues.forEach(function (c) {
      if (c.k === "trigger") { cur = { who: c.who, trig: c.b, rwho: "T", resp: "" }; out.push(cur); }
      else if (c.k === "response") { if (cur && !cur.resp) cur.resp = c.b; else out.push({ who: "P", trig: "", rwho: "T", resp: c.b }); cur = null; }
    });
    return out;
  }
  var RESP_LIB = [
    { name: "Pagtimbaya", note: "Greeting · Sign of the Cross and greeting", hdr: "inherit", ex: blocks(fromMass("Greeting")) },
    { name: "Pagtimbaya · grasya", note: "Greeting · the longer Trinitarian form", hdr: "inherit", ex: [
      { who: "P", trig: "Sa ngalan sa Amahan, ug sa Anak, ug sa Espiritu Santo.", rwho: "T", resp: "Amen." },
      { who: "P", trig: "Ang grasya sa atong Ginoong Jesukristo, ang gugma sa Dios, ug ang panag-uban sa Espiritu Santo maanaa kaninyong tanan.", rwho: "T", resp: "Ug maanaa usab sa imong espiritu." }] },
    { name: "Misteryo sa Pagtoo", note: "Memorial Acclamation", hdr: "inherit", ex: blocks(fromMass("Memorial Acclamation")) },
    { name: "Amahan Namo", note: "The Lord's Prayer, said by all", hdr: "hide", ex: blocks(fromMass("Our Father")) },
    { name: "Kordero sa Dios", note: "Agnus Dei", hdr: "hide", ex: [
      { who: "P", trig: "", rwho: "T", resp: "Kordero sa Dios, nga nagawagtang sa mga sala sa kalibutan, kaloy-i kami." },
      { who: "P", trig: "", rwho: "T", resp: "Kordero sa Dios, nga nagawagtang sa mga sala sa kalibutan, hatagi kami sa kalinaw." }] },
    { name: "Pagpadala", note: "Dismissal", hdr: "inherit", ex: blocks(fromMass("Dismissal")) }
  ];
  function textSlides(cues) { return cues.map(function (c) { return { kind: "text", h: c.h || "", b: c.b || "", f: c.f || "", dur: null }; }); }
  var GP_LIB = [
    { name: "Pahibalo sa Domingo", note: "Weekly announcements", dur: 8, slides: textSlides(K.MASS.sections[0].items[0].cues) },
    { name: "Pasalamat", note: "Closing thanks", dur: 8, slides: textSlides(K.MASS.sections[5].items[0].cues) },
    { name: "Ikaduhang Kolekta", note: "Second collection appeal", dur: 10, slides: [
      { kind: "text", h: "Ikaduhang kolekta", b: "Ang ikaduhang kolekta karong Domingo alang sa pag-ayo sa kampanaryo.", f: "Parokya ni San Roque", dur: null },
      { kind: "image", file: "kampanaryo-2026.jpg", alt: "The bell tower under repair", dur: 12 }] },
    { name: "Bible Study", note: "Recurring invitation", dur: 8, slides: [{ kind: "text", h: "Bible Study", b: "Matag Huwebes, 6:00 sa gabii, sa parish hall. Dad-a ang inyong Biblia.", f: "", dur: null }] }
  ];

  // Readings are Mass-owned: the title and closing exchange project, the passage is operator
  // context unless full-reading projection is switched on (MVP-PLAN §4, Decisions 71, 255, 262, 263).
  var FIRST_TEXT = "Mao kini ang giingon sa Ginoo: Pag-awit sa kalipay alang kang Jacob, pagsinggit alang sa nag-una sa kanasoran. Ipahayag, idayeg, ug isulti: Giluwas sa Ginoo ang iyang katawhan, ang salin sa Israel.\n\n" +
    "Tan-awa, dad-on ko sila gikan sa yuta sa amihanan, ug tigomon ko sila gikan sa kinatumyan sa kalibotan. Uban kanila ang buta ug ang bakol, ang mabdos ug ang nag-anak; usa ka dakong panon ang mobalik dinhi.\n\n" +
    "Moabot sila nga naghilak, ug giyahan ko sila samtang nag-ampo sila. Dad-on ko sila sa mga sapa sa tubig, sa tul-id nga dalan diin dili sila mapandol; kay ako amahan sa Israel, ug si Efraim akong panganay.";
  var SECOND_TEXT = "Ang matag labawng pari gipili gikan sa mga tawo ug gitudlo sa pag-alagad sa Dios alang kanila, sa paghalad og mga gasa ug mga halad alang sa mga sala.";
  function paginate(text) {
    var out = [];
    text.split(/\n\s*\n/).forEach(function (para) {                 // blank lines are hard boundaries
      var cur = "";
      (para.trim().match(/[^.!?;:]+[.!?;:]*\s*/g) || []).forEach(function (s) {
        if (cur && (cur + s).length > 240) { out.push(cur.trim()); cur = ""; }
        cur += s;
      });
      if (cur.trim()) out.push(cur.trim());
    });
    return out;
  }

  // ── regenerate an item's Cues from its editor model ───────────────────────────────────────
  function regen(it) {
    if (it.rd) {
      var r = it.rd, cues = [{ k: "reading-id", b: r.title, f: r.ref }];
      if (r.project && r.text.trim()) paginate(r.text).forEach(function (b) { cues.push({ k: "reading", b: b, f: r.ref }); });
      if (r.prompt.trim()) cues.push({ k: "trigger", who: r.who, b: r.prompt });
      if (r.resp.trim()) cues.push({ k: "response", h: "TUBAG:", b: r.resp });
      it.cues = cues;
    } else if (it.ex) {
      var h = it.hdr === "inherit" ? "TUBAG:" : it.hdr === "override" ? it.hdrText : "";
      it.cues = [];
      it.ex.forEach(function (x, bi) {
        if (x.trig.trim()) it.cues.push({ k: "trigger", who: x.who, b: x.trig, block: bi });
        it.cues.push({ k: "response", h: h, b: x.resp, block: bi, who: x.rwho });
      });
    } else if (it.gp) {
      it.cues = it.gp.slides.map(function (s, si) { return s.kind === "image" ? { k: "image", file: s.file, alt: s.alt, slide: si } : { k: "general", h: s.h, b: s.b, f: s.f, slide: si }; });
    } else if (it.hold) {
      it.cues = [{ k: it.hold.kind, b: it.hold.label }];
    }
  }

  // ── the working copy of the Mass ──────────────────────────────────────────────────────────
  var RESP_SRC = { "Greeting": "Pagtimbaya", "Memorial Acclamation": "Misteryo sa Pagtoo", "Our Father": "Amahan Namo", "Dismissal": "Pagpadala" };
  function respEntry(name) { for (var i = 0; i < RESP_LIB.length; i++) if (RESP_LIB[i].name === name) return RESP_LIB[i]; return null; }
  function model() {
    return K.MASS.sections.map(function (sec, si) {
      return { title: sec.title, items: sec.items.map(function (f) {
        var it = { title: f.title, type: f.type, song: f.song || null, cues: f.cues.map(strip),
          arr: f.song ? (f.song === "Ihalad Namo" ? CHOIR + " · scanned score" : CHOIR) : null,
          local: false, base: null, changes: [], empty: false,
          theme: f.title === "Kyrie" ? "high-contrast" : null,
          warn: f.song === "Pan sa Kinabuhi" ? "Pagination not reviewed since the lyrics changed" : null };
        if (f.type === "Reading" && f.title !== "Gospel") {
          var trig = it.cues.filter(function (c) { return c.k === "trigger"; })[0], resp = it.cues.filter(function (c) { return c.k === "response"; })[0];
          it.rd = { role: f.title, title: it.cues[0].b, ref: it.cues[0].f, text: f.title === "First Reading" ? FIRST_TEXT : SECOND_TEXT,
            project: f.title === "First Reading", who: trig ? trig.who : "L", prompt: trig ? trig.b : "", resp: resp ? resp.b : "" };
        } else if (f.type === "Response sequence" && RESP_SRC[f.title]) {
          var e = respEntry(RESP_SRC[f.title]); it.src = e.name; it.ex = copy(e.ex); it.hdr = e.hdr; it.hdrText = "";
        } else if (f.type === "General Presentation") {
          var g = GP_LIB[si === 0 ? 0 : 1]; it.src = g.name; it.gp = { slides: copy(g.slides), dur: g.dur, note: si === 0 ? "Basaha hinay-hinay ug hulata nga mohilom ang simbahan." : "" };
        }
        regen(it);
        return it;
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
  var CUE_TYPES = [
    ["response", "Response", "messages-square", "A projected congregation response"],
    ["general", "Text Slide", "type", "A projected text Slide"],
    ["trigger", "Operator trigger", "user-round", "A private line that cues the next step"],
    ["wait", "Wait", "hourglass", "Program stays as it is"],
    ["hold", "Hold", "square", "Black checkpoint"],
    ["blackout", "Blackout", "moon", "Enters the Blackout state"]
  ];
  function newCue(k) {
    return k === "response" ? { k: k, h: "TUBAG:", b: "" } : k === "general" ? { k: k, h: "", b: "", f: "" } : k === "trigger" ? { k: k, who: "P", b: "" } :
      k === "wait" ? { k: k, b: "" } : k === "hold" ? { k: k, b: "Hold" } : { k: k, b: "Blackout" };
  }

  // ── form helpers: every field writes through data-path into the selected item ─────────────
  function field(path, val, o) {
    o = o || {};
    var attrs = ' data-input="f" data-path="' + path + '" data-focus-key="' + path + '"' + (o.ro ? " readonly" : "") + (o.ph ? ' placeholder="' + esc(o.ph) + '"' : "") + (o.id ? ' id="' + o.id + '"' : "");
    if (o.rows) return '<textarea class="pl-input" rows="' + o.rows + '"' + attrs + ">" + esc(val) + "</textarea>";
    return '<input class="pl-input' + (o.small ? " pl-input--sm" : "") + '"' + (o.type ? ' type="' + o.type + '"' : "") + attrs + ' value="' + esc(val) + '">';
  }
  function choose(path, val, options, o) {
    o = o || {};
    return '<select class="pl-input pl-input--sel" data-input="f" data-path="' + path + '" data-focus-key="' + path + '"' + (o.ro ? " disabled" : "") + (o.label ? ' aria-label="' + esc(o.label) + '"' : "") + ">" +
      options.map(function (x) { return '<option value="' + esc(x[0]) + '"' + (x[0] === val ? " selected" : "") + ">" + esc(x[1]) + "</option>"; }).join("") + "</select>";
  }
  function toggle(path, on, label, o) {
    o = o || {};
    return '<label class="pl-switch"><input type="checkbox" data-input="f" data-path="' + path + '" data-focus-key="' + path + '"' + (on ? " checked" : "") + (o.ro ? " disabled" : "") + "><span></span>" + label + "</label>";
  }
  function row(label, control, hint) { return '<div class="pl-frow"><span class="o-label">' + label + "</span><div>" + control + (hint ? '<div class="o-meta">' + hint + "</div>" : "") + "</div></div>"; }
  function setPath(obj, path, v) {
    var p = path.split("."), o = obj;
    for (var i = 0; i < p.length - 1; i++) o = o[/^\d+$/.test(p[i]) ? +p[i] : p[i]];
    var last = p[p.length - 1];
    o[/^\d+$/.test(last) ? +last : last] = v;
  }

  // opts: w, h, item (number), cue, scale, setup(api), afterRender(api)
  function planner(wrapper, opts) {
    opts = opts || {};
    var W = opts.w || 1366, H = opts.h || 768;
    var app = document.createElement("div");
    app.className = "o-app";
    app.tabIndex = -1;
    wrapper.classList.add("o-fit");
    wrapper.appendChild(app);

    var S = { sec: model(), cue: opts.cue || 0, scale: opts.scale || 1, theme: "ordo-black",
      overlay: null, child: null, pick: null, stagedTheme: null, keepOverride: {}, dest: null, toast: null, undo: null, focus: null, cueMenu: false };
    function items() { var out = []; S.sec.forEach(function (sec, si) { sec.items.forEach(function (it, ai) { it.section = si; it.at = ai; it.n = out.length + 1; out.push(it); }); }); return out; }
    S.sel = items()[(opts.item || 2) - 1];

    function bad(it) { return it.cues.filter(function (c) { return c.k === "unavailable"; }).length; }
    function findings() {
      var inc = [], warn = [];
      items().forEach(function (it) {
        function add(title, text, action, focus) { inc.push({ it: it, title: title, text: text, action: action, focus: focus }); }
        if (it.type === "Song" && !it.song) add("No Song chosen", "Nothing can be presented for this item until a Song and Arrangement are chosen.", "Choose Song", "choose");
        else if (bad(it)) add(bad(it) + " Cues unavailable", it.cues[0].why + ", so these Cues can't produce a frame.", "Choose another Arrangement", "fix");
        else if (it.rd && !it.rd.title.trim()) add("No proclamation title", "The Reading has nothing to identify it on screen.", "Add the title", "rd.title");
        else if (it.rd && !it.rd.text.trim()) add("No reading text", "The reader's passage is missing" + (it.rd.project ? ", and full-reading projection has nothing to show." : "."), "Add the text", "rd.text");
        else if (it.ex && it.pending) add("No Response chosen", "Choose a Response or Prayer, or write one for this Mass.", "Choose Response", "choose");
        else if (it.ex && it.ex.some(function (x) { return !x.resp.trim(); })) add("A response is empty", "Every exchange needs the congregation's words.", "Write the response", "ex");
        else if (it.gp && it.pending) add("No General Presentation chosen", "Choose one from the Library, or make one for this Mass.", "Choose General Presentation", "choose");
        else if (it.gp && !it.gp.slides.length) add("No Slides", "This General Presentation shows nothing.", "Add a Slide", "addslide");
        else if (it.custom && !it.cues.length) add("No Cues yet", "This Custom sequence is empty.", "Add a Cue", "addcue");
        else if (it.empty) add("No content yet", "This " + it.type + " has no Cues, so nothing is presented for it.", "Add content", "fix");
        if (it.warn) warn.push({ it: it, title: "Pagination not reviewed", text: it.warn + ".", action: "Review pagination", focus: "fix" });
      });
      return { inc: inc, warn: warn };
    }
    function incomplete(it) { return findings().inc.some(function (f) { return f.it === it; }); }
    function readinessLabel(f) {
      var parts = [];
      if (f.inc.length) parts.push(f.inc.length + " incomplete");
      if (f.warn.length) parts.push(f.warn.length + (f.warn.length === 1 ? " warning" : " warnings"));
      if (!parts.length) return statusLabel("live", "Ready");
      return statusLabel(f.inc.length ? "warning" : "info", parts.join(" · "), f.inc.length ? "diamond" : null);
    }

    // ── undo: one level, enough to show the toast's promise ─────────────────────────────────
    function snapshot() { return { sec: copy(S.sec), n: S.sel.n, cue: S.cue, theme: S.theme }; }
    function done(text, snap) { S.undo = snap; S.toast = text; }
    function undo() {
      if (!S.undo) return;
      S.sec = S.undo.sec; S.theme = S.undo.theme; S.sel = items()[Math.min(S.undo.n, items().length) - 1]; S.cue = S.undo.cue;
      S.undo = null; S.toast = null; S.overlay = null; S.child = null;
    }

    // ── commands ────────────────────────────────────────────────────────────────────────────
    function select(n) { S.sel = items()[n - 1]; S.cue = 0; S.cueMenu = false; }
    function openAdd() { S.overlay = "add"; S.child = null; S.dest = { sec: S.sel.section, at: S.sel.at }; }
    function destLabel(d) { var sec = S.sec[d.sec]; return d.at < 0 ? "Start of " + sec.title : "After " + sec.items[d.at].title + " · " + sec.title; }
    function addItem(type) {
      var snap = snapshot(), spec = NEW_ITEM[type], d = S.dest || { sec: S.sel.section, at: S.sel.at };
      var it = { title: spec.title, type: type === "Response or Prayer" ? "Response sequence" : type, song: null, arr: null, local: false, base: null, changes: [], theme: null, warn: null, cues: [], empty: false };
      if (type === "Reading") it.rd = { role: "Reading", title: "", ref: "", text: "", project: false, who: "L", prompt: "Ang Pulong sa Dios.", resp: "Salamat sa Dios." };
      else if (type === "Response or Prayer") { it.ex = []; it.hdr = "inherit"; it.hdrText = ""; it.pending = true; }
      else if (type === "General Presentation") { it.gp = { slides: [], dur: 8, note: "" }; it.pending = true; }
      else if (type === "Hold" || type === "Blackout") it.hold = { kind: type === "Hold" ? "hold" : "blackout", label: type === "Hold" ? "Hold" : "Blackout" };
      else if (type === "Custom sequence") it.custom = true;
      regen(it);
      S.sec[d.sec].items.splice(d.at + 1, 0, it);
      items(); S.sel = it; S.cue = 0;
      done("Added " + it.title + " " + destLabel(d).replace(/^After /, "after ").replace(/^Start of /, "at the start of ") + ".", snap);
      S.overlay = null;
      if (type === "Song" || type === "Response or Prayer" || type === "General Presentation") openPicker();
      else S.focus = type === "Reading" ? "rd.title" : type === "Custom sequence" ? "title" : "hold.label";
    }
    function kindOf(it) { return it.ex ? "response" : it.gp ? "general" : "song"; }
    function libFor(kind) { return kind === "response" ? RESP_LIB : GP_LIB; }
    function openPicker() {
      var kind = kindOf(S.sel);
      S.overlay = "picker"; S.child = null;
      if (kind === "song") S.pick = { kind: kind, q: "", song: S.sel.song ? libIndex(S.sel.song) : null, arr: null };
      else S.pick = { kind: kind, q: "", song: null, arr: null };
    }
    function stage(name, arr) {
      if (S.pick.kind !== "song") { libFor(S.pick.kind).forEach(function (e, i) { if (e.name === name) S.pick.song = i; }); S.pick.arr = 0; return; }
      S.pick.song = libIndex(name); var a = LIB[S.pick.song].arrs; for (var j = 0; j < a.length; j++) if (a[j].name === arr) S.pick.arr = j;
    }
    function staged() {
      if (!S.pick || S.pick.song == null || S.pick.arr == null) return null;
      if (S.pick.kind === "song") return { song: LIB[S.pick.song], arr: LIB[S.pick.song].arrs[S.pick.arr], name: LIB[S.pick.song].title + " · " + LIB[S.pick.song].arrs[S.pick.arr].name, use: "Use " + LIB[S.pick.song].arrs[S.pick.arr].name + " arrangement" };
      var e = libFor(S.pick.kind)[S.pick.song];
      return { entry: e, name: e.name, use: "Use " + e.name };
    }
    function isCurrent(st) { return st.entry ? S.sel.src === st.entry.name && !S.sel.pending : S.sel.song === st.song.title && S.sel.arr === st.arr.name; }
    function currentName(it) { return it.song ? it.song + " · " + it.arr : it.src; }
    function commitPick() {
      var st = staged(); if (!st || isCurrent(st)) return;
      if (S.sel.local) { S.child = "review"; return; }
      applyPick(st);
    }
    function applyPick(st) {
      var snap = snapshot(), it = S.sel;
      if (st.entry) {
        it.src = st.entry.name; it.pending = false;
        if (it.ex) { it.ex = copy(st.entry.ex); it.hdr = st.entry.hdr; }
        else { var note = it.gp.note; it.gp = { slides: copy(st.entry.slides), dur: st.entry.dur, note: note }; }
        if (/^New /.test(it.title)) it.title = st.entry.name;
        regen(it);
      } else { it.song = st.song.title; it.arr = st.arr.name; it.cues = st.arr.cues.map(strip); it.warn = null; }
      it.local = false; it.base = null; it.changes = []; it.empty = false;
      S.cue = 0; S.overlay = null; S.child = null;
      done(it.title + " now uses " + st.name + ".", snap);
    }
    function writeLocal() {
      var snap = snapshot(), it = S.sel;
      it.pending = false; it.local = true; it.src = null; it.base = null; it.changes = [];
      if (it.ex) it.ex = [{ who: "P", trig: "", rwho: "T", resp: "" }];
      else it.gp.slides = [{ kind: "text", h: "", b: "", f: "", dur: null }];
      regen(it); S.overlay = null; S.focus = it.ex ? "ex.0.trig" : "gp.slides.0.b";
      done(it.title + " is written for this Mass only.", snap);
    }
    function customize() {
      var snap = snapshot(), it = S.sel;
      it.local = true; it.changes = [];
      it.base = copy({ cues: it.cues, ex: it.ex || null, gp: it.gp || null, hdr: it.hdr || null });
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
      var snap = snapshot(), it = S.sel, b = it.base;
      it.cues = b.cues; if (b.ex) it.ex = b.ex; if (b.gp) { b.gp.note = it.gp.note; it.gp = b.gp; } if (b.hdr) it.hdr = b.hdr;
      it.local = false; it.base = null; it.changes = []; regen(it);
      S.cue = Math.min(S.cue, it.cues.length - 1); S.overlay = null;
      done(it.title + " follows the Library " + (it.song ? "Arrangement" : "source") + " again.", snap);
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
    function editField(path, v) {
      var it = S.sel;
      setPath(it, path, v);
      if (path === "hold.label") it.title = v.trim() || KIND[it.hold.kind];
      if (path === "rd.text" || path === "rd.project") S.cue = Math.min(S.cue, 0);
      regen(it);
      if (it.local) change(path.indexOf("note") >= 0 ? "Edited the private context" : "Edited " + (it.ex ? "an exchange" : it.gp ? "a Slide" : "the content"));
      S.cue = Math.max(0, Math.min(S.cue, it.cues.length - 1));
    }
    // list editing for exchanges, Slides and custom Cues
    function listOf(it, which) { return which === "ex" ? it.ex : which === "slides" ? it.gp.slides : it.cues; }
    function cueOfEntry(it, which, i) {
      for (var j = 0; j < it.cues.length; j++) { var c = it.cues[j]; if ((which === "ex" && c.block === i) || (which === "slides" && c.slide === i)) return j; }
      return which === "cues" ? i : 0;
    }
    function moveEntry(which, i, d) { var l = listOf(S.sel, which), x = l[i]; l[i] = l[i + d]; l[i + d] = x; regen(S.sel); S.cue = cueOfEntry(S.sel, which, i + d); if (S.sel.local) change("Reordered"); }
    function removeEntry(which, i) { listOf(S.sel, which).splice(i, 1); regen(S.sel); S.cue = Math.max(0, Math.min(S.cue, S.sel.cues.length - 1)); if (S.sel.local) change("Removed an entry"); }

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
    function outline(wide, f) {
      var html = "";
      S.sec.forEach(function (sec) {
        html += '<div class="o-section-label pl-sec">' + icon("chevron-down") + esc(sec.title) + "</div>";
        sec.items.forEach(function (x) {
          var flag = f.inc.some(function (y) { return y.it === x; });
          var sub = x.song || x.src || "";
          html += '<button class="o-row pl-item" data-item="' + x.n + '"' + (x === S.sel ? ' aria-selected="true"' : "") + '><span class="o-meta">' + x.n + '</span><span class="o-grow o-ellipsis">' + esc(x.title) +
            (sub && sub !== x.title ? ' <span class="o-muted">· ' + esc(sub) + "</span>" : "") + "</span>" +
            (x.local ? '<span class="o-meta" title="Mass-local override">' + icon("pencil") + "</span>" : "") +
            (flag ? '<span class="o-status" data-state="warning" title="Incomplete" aria-label="Incomplete">' + icon("triangle-alert") + "</span>" : "") + "</button>";
        });
      });
      return '<section class="o-pane pl-outline" aria-label="Mass outline"><div class="o-pane-head"><b>Mass outline</b><span class="o-grow"></span>' +
        '<button class="o-btn' + (wide ? "" : " o-btn--icon") + '" data-act="add" data-anchor="add" aria-haspopup="menu" aria-expanded="' + (S.overlay === "add") + '"' + (wide ? "" : ' aria-label="Add Mass item" title="Add Mass item"') + ">" +
        icon("plus") + (wide ? "Add Mass item" : "") + "</button></div>" +
        '<div class="o-pane-body o-rows--open">' + html + "</div></section>";
    }
    function thumbs(it, list) {
      list = list || it.cues.map(function (q, i) { return i; });
      return '<div class="pl-thumbs">' + list.map(function (i) {
        var q = it.cues[i], cur = i === S.cue ? ' aria-current="true"' : "";
        var frame = OP_ONLY[q.k] ? '<div class="o-slide pl-private-thumb"><div class="o-slide-state">' + icon(q.k === "trigger" ? "user-round" : q.k === "wait" ? "hourglass" : q.k === "hold" ? "square" : "moon") +
          "<div>" + esc(q.k === "trigger" ? (WHO[q.who] || q.who) + " prompt" : KIND[q.k]) + "<br><span style=\"font-weight:400\">operator only</span></div></div></div>" : tslide(q, th(it));
        return '<button class="pr-tile pl-thumb" data-cue="' + i + '"' + cur + '><div class="o-slide-wrap"' + cur + ">" + frame + "</div>" +
          '<div class="o-caption"><span class="o-n">' + (i + 1) + '</span><span class="o-t">' + esc(q.k === "unavailable" ? "Unavailable" : cueLabel(q) || KIND[q.k]) + "</span></div></button>";
      }).join("") + "</div>";
    }
    function generated(it, wide) {
      return '<div class="pl-block"><div class="pl-block-head"><b>Generated Cues</b><span class="o-meta">' + (it.cues.length ? "Selected " + (S.cue + 1) + " of " + it.cues.length : "None yet") + (wide ? " · click a Cue to inspect" : "") + "</span></div>" +
        (it.cues.length ? thumbs(it) : '<div class="o-meta">Nothing is generated until the item has content.</div>') + "</div>";
    }
    function sourceCard(it, kindLabel, wide) {
      var local = it.local;
      var banner = local ? '<div class="o-notice" data-state="info">' + icon("pencil") + '<div class="o-grow"><b>Mass-local override.</b> Changes here belong only to this Mass plan; the Library ' + (it.song ? "Arrangement" : "source") + " is unchanged." +
        (it.base ? '<div class="pl-banner-acts"><button class="o-btn" data-act="return">' + icon("undo-2") + "Return to library " + (it.song ? "arrangement" : "source") + "</button>" +
        '<button class="o-btn o-btn--quiet">' + icon("library") + "Open in Library</button></div>" : "") + "</div></div>" : "";
      var what = it.song ? esc(it.arr) + " arrangement" : esc(it.src || "Written for this Mass");
      var card = '<div class="pl-card"><div class="o-grow"><div class="o-label">' + (local && it.base ? "Copied from " : local ? "" : "Using ") + what + "</div>" +
        '<div class="o-meta">' + (local ? (it.base ? "Mass-local override" : "Mass-local · not in the Library") : "Linked to Library · " + kindLabel + (wide && it.song ? " · last reviewed 17 Okt 2026" : "")) + "</div></div>" +
        '<button class="o-btn" data-act="picker">' + icon("replace") + (wide ? "Change " + (it.song ? "Song or Arrangement" : kindLabel.replace(/s$/, "")) : "Change") + "</button>" +
        (local ? "" : '<button class="o-btn" data-act="customize">' + icon("pencil") + "Customize for this Mass</button>") + "</div>";
      return banner + card;
    }
    function pendingEditor(it, what, icn) {
      return '<div class="pl-empty">' + icon(icn, "o-icon-20") + "<b>No " + what + " chosen yet</b>" +
        '<div class="o-meta">Choose one from the Library, or write one that belongs to this Mass only. Nothing is presented for this item until you do.</div>' +
        '<div class="pl-banner-acts"><button class="o-btn o-btn--primary" data-act="picker" data-focus-key="choose">' + icon("search") + "Choose " + what + "</button>" +
        '<button class="o-btn" data-act="local-new">' + icon("pencil") + "Write for this Mass only</button></div></div>";
    }
    function songEditor(it, wide) {
      if (!it.song) {
        return '<div class="pl-empty">' + icon("music", "o-icon-20") + "<b>No Song chosen yet</b>" +
          '<div class="o-meta">Pick a Song and one of its Arrangements. Nothing is presented for this item until you do.</div>' +
          '<button class="o-btn o-btn--primary" data-act="picker" data-focus-key="choose">' + icon("search") + "Choose Song</button></div>";
      }
      var local = it.local, seq;
      if (local) {
        seq = it.cues.map(function (q, i) {
          var name = q.k === "song-id" ? "Song title" : q.h + (q.forked ? " (this occurrence)" : "");
          var link = linked(it, i) ? '<span class="pl-chip">' + icon("link") + "Linked</span>" : "";
          return '<div class="o-row pl-seq pl-occ"' + (i === S.cue ? ' aria-selected="true"' : "") + ">" + icon("grip-vertical") +
            '<button class="pl-occ-name" data-cue="' + i + '"><span class="o-meta">' + (i + 1) + "</span> " + esc(name) + "</button>" + link + '<span class="o-grow"></span>' + mover("cues", i, it.cues.length) + "</div>";
        }).join("");
        var c = it.cues[S.cue];
        if (c && c.k === "lyrics") {
          seq += '<div class="pl-text"><label class="o-label" for="pl-text-' + it.n + '">' + esc(c.h) + (c.forked ? " · this occurrence" : "") + " text</label>" +
            '<textarea id="pl-text-' + it.n + '" class="pl-input" data-input="text" data-focus-key="text" rows="3">' + esc(c.b) + "</textarea>" +
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
      return '<div class="pl-ed-body">' + sourceCard(it, "Songs", wide) +
        '<div class="pl-block"><div class="pl-block-head"><b>Sequence</b><span class="o-meta">' + it.cues.length + " sections · " + it.cues.length + (wide ? " generated Cues" : " Cues") + "</span></div>" + seq + "</div>" +
        generated(it, wide) +
        (wide && !local ? '<div class="pl-block"><div class="pl-block-head"><b>Mass-local state</b></div><div class="o-meta">Following the library Arrangement; no changes for this Mass.</div></div>' : "") + "</div>";
    }
    function mover(which, i, n) {
      return '<button class="o-btn o-btn--quiet o-btn--icon" data-mv="' + which + ":" + i + ':-1" aria-label="Move up" title="Move up"' + (i === 0 ? " disabled" : "") + ">" + icon("arrow-up") + "</button>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-mv="' + which + ":" + i + ':1" aria-label="Move down" title="Move down"' + (i === n - 1 ? " disabled" : "") + ">" + icon("arrow-down") + "</button>" +
        '<button class="o-btn o-btn--quiet o-btn--icon" data-del="' + which + ":" + i + '" aria-label="Remove" title="Remove">' + icon("x") + "</button>";
    }
    function readingEditor(it, wide) {
      var r = it.rd, n = paginate(r.text).length;
      var who = Object.keys(WHO).filter(function (k) { return k !== "T" && k !== "Ch"; }).map(function (k) { return [k, WHO[k]]; });
      return '<div class="pl-ed-body"><div class="pl-meta-line">' + icon("book-open") + '<span class="o-meta">Mass-owned Reading · ' + esc(K.MASS.language) + " · edits save to this plan</span></div>" +
        '<div class="pl-form">' +
          row("Proclamation title", field("rd.title", r.title, { ph: "Pagbasa gikan sa …" }), "Projected in sentence case, with no First or Second label (Decision 262).") +
          row("Reference", field("rd.ref", r.ref, { ph: "Jeremias 31:7-9", small: true }), "Shown at the bottom left of every Reading Slide.") +
          row("Reading text", field("rd.text", r.text, { rows: wide ? 8 : 6, ph: "Paste or type the passage" }),
            r.project ? "Projected as " + n + (n === 1 ? " passage Slide" : " passage Slides") + ". Blank lines always start a new Slide." : "The reader's text: shown to you in Presenter as private context, not projected.") +
          row("Full-reading projection", toggle("rd.project", r.project, "Project the passage for the congregation"), "Off by default; responses and acclamations project either way.") +
        "</div>" +
        '<div class="pl-block"><div class="pl-block-head"><b>Closing exchange</b><span class="o-meta">Optional prompt, then the congregation response</span></div>' +
          '<div class="pl-exchange"><div class="pl-ex-line">' + choose("rd.who", r.who, who, { label: "Prompt role" }) + field("rd.prompt", r.prompt, { ph: "Optional private prompt" }) + '<span class="pl-chip">Operator only</span></div>' +
          '<div class="pl-ex-line"><span class="pl-who-t">All</span>' + field("rd.resp", r.resp, { ph: "Congregation response" }) + '<span class="pl-chip">Projected</span></div></div></div>' +
        generated(it, wide) + "</div>";
    }
    function responseEditor(it, wide) {
      if (it.pending) return pendingEditor(it, "Response", "messages-square");
      var ro = !it.local, who = Object.keys(WHO).filter(function (k) { return k !== "T"; }).map(function (k) { return [k, WHO[k]]; });
      var rwho = [["T", "All"], ["Ch", "Choir"], ["C", "Cantor"]];
      var hdr = '<div class="pl-form">' + row("Response header", '<div class="pl-inline">' + choose("hdr", it.hdr, [["inherit", "Plan default · TUBAG:"], ["hide", "Hide"], ["override", "Override"]], { ro: ro, label: "Response header" }) +
        (it.hdr === "override" ? field("hdrText", it.hdrText, { ph: "TUBAG:", ro: ro, small: true }) : "") + "</div>", "Repeated above every response Slide; the Cebuano template's default is TUBAG: (Decision 255).") + "</div>";
      var bl = it.ex.map(function (x, bi) {
        var cues = []; it.cues.forEach(function (c, ci) { if (c.block === bi) cues.push(ci); });
        return '<div class="pl-exblock"><div class="pl-block-head"><b>Exchange ' + (bi + 1) + "</b>" + (ro ? "" : '<span class="pl-acts">' + mover("ex", bi, it.ex.length) + "</span>") + "</div>" +
          '<div class="pl-ex-line">' + choose("ex." + bi + ".who", x.who, who, { ro: ro, label: "Trigger role" }) + field("ex." + bi + ".trig", x.trig, { ro: ro, ph: "Optional private prompt" }) + '<span class="pl-chip">Operator only</span></div>' +
          '<div class="pl-ex-line">' + choose("ex." + bi + ".rwho", x.rwho, rwho, { ro: ro, label: "Response role" }) + field("ex." + bi + ".resp", x.resp, { ro: ro, ph: "Required congregation response", rows: 2 }) + '<span class="pl-chip">Projected</span></div>' +
          thumbs(it, cues) + "</div>";
      }).join("");
      return '<div class="pl-ed-body">' + sourceCard(it, "Responses & Prayers", wide) + hdr +
        '<div class="pl-block"><div class="pl-block-head"><b>Exchanges</b><span class="o-meta">' + it.ex.length + " · flattened as prompt → response in Presenter</span></div>" + bl +
        (ro ? '<div class="o-meta">Linked to the Library. Customize for this Mass to edit these words here.</div>' : '<button class="o-btn" data-addex data-focus-key="ex">' + icon("plus") + "Add exchange</button>") + "</div></div>";
    }
    function generalEditor(it, wide) {
      if (it.pending) return pendingEditor(it, "General Presentation", "presentation");
      var ro = !it.local, g = it.gp;
      var slides = g.slides.map(function (s, si) {
        var ci = cueOfEntry(it, "slides", si), cur = ci === S.cue;
        var fields = s.kind === "image" ?
          row("Image", '<div class="pl-inline">' + field("gp.slides." + si + ".file", s.file, { ro: ro, ph: "photo.jpg" }) + (ro ? "" : '<button class="o-btn">' + icon("upload") + "Choose…</button>") + "</div>", "Fitted inside the safe area; never cropped.") +
          row("Description", field("gp.slides." + si + ".alt", s.alt, { ro: ro, ph: "What the image shows" }), "For the operator and screen readers; not projected.") :
          row("Header", field("gp.slides." + si + ".h", s.h, { ro: ro, ph: "Optional" })) +
          row("Text", field("gp.slides." + si + ".b", s.b, { ro: ro, rows: 3, ph: "Slide text" })) +
          row("Footer", field("gp.slides." + si + ".f", s.f, { ro: ro, ph: "Optional" }));
        return '<div class="pl-slide' + (cur ? " is-current" : "") + '"><button class="pl-slide-thumb" data-cue="' + ci + '" aria-label="Inspect Slide ' + (si + 1) + '">' + tslide(it.cues[ci], th(it)) + "</button>" +
          '<div class="pl-form o-grow"><div class="pl-block-head"><b>Slide ' + (si + 1) + " · " + (s.kind === "image" ? "Image" : "Text") + "</b>" + (ro ? "" : '<span class="pl-acts">' + mover("slides", si, g.slides.length) + "</span>") + "</div>" + fields +
          row("Auto-play", '<div class="pl-inline">' + field("gp.slides." + si + ".dur", s.dur == null ? "" : s.dur, { ro: ro, ph: String(g.dur), small: true, type: "number" }) + '<span class="o-meta">seconds' + (s.dur == null ? " · uses the default" : "") + "</span></div>") +
          "</div></div>";
      }).join("");
      return '<div class="pl-ed-body">' + sourceCard(it, "General Presentations", wide) +
        '<div class="pl-block"><div class="pl-block-head"><b>Slides</b><span class="o-meta">' + g.slides.length + " · default " + g.dur + " s each when auto-play runs (A in Presenter)</span></div>" + slides +
        (ro ? '<div class="o-meta">Linked to the Library. Customize for this Mass to change the Slides here.</div>' :
          '<div class="pl-banner-acts"><button class="o-btn" data-addslide="text" data-focus-key="addslide">' + icon("type") + "Add text Slide</button>" +
          '<button class="o-btn" data-addslide="image">' + icon("image") + "Add image Slide</button></div>") + "</div>" +
        '<div class="pl-block"><div class="pl-block-head"><b>Private delivery context</b><span class="pl-chip">This Mass only · never projected</span></div>' +
          field("gp.note", g.note, { rows: 2, ph: "Notes for whoever reads or runs this, e.g. wait for silence" }) +
          '<div class="o-meta">Belongs to this Mass item even while the Slides stay linked to the Library.</div></div></div>';
    }
    function holdEditor(it) {
      var hold = it.hold.kind === "hold";
      return '<div class="pl-ed-body"><div class="pl-form">' +
        row("Label", field("hold.label", it.hold.label, { ph: hold ? "Homily" : "Blackout" }), "For you only: shown in the outline, the timeline and Preview context; never projected.") + "</div>" +
        '<div class="pl-explain">' + (hold ?
          "<b>A Hold is a black checkpoint.</b> While Live, reaching it fades Program to black and stays linked: the next Cue goes live as usual. It doesn't latch like Blackout. Use Freeze in Presenter when the previous frame should stay up instead." :
          "<b>A Blackout item enters the Blackout state.</b> Program stays black while you navigate Preview, until Space (or B) takes Preview live. Use a Hold when the next Cue should go live without that step.") + "</div>" +
        '<div class="pl-block"><div class="pl-block-head"><b>In Presenter</b></div>' + thumbs(it) + "</div></div>";
    }
    function customEditor(it, wide) {
      var types = CUE_TYPES.map(function (t) { return '<button class="o-row pl-type" role="menuitem" data-addcue="' + t[0] + '">' + icon(t[2]) + '<span class="o-grow"><b>' + t[1] + '</b><span class="o-meta">' + t[3] + "</span></span></button>"; }).join("");
      var list = it.cues.map(function (q, i) {
        var t = CUE_TYPES.filter(function (x) { return x[0] === q.k; })[0];
        return '<div class="o-row pl-seq pl-occ"' + (i === S.cue ? ' aria-selected="true"' : "") + ">" + icon("grip-vertical") +
          '<button class="pl-occ-name" data-cue="' + i + '"><span class="o-meta">' + (i + 1) + "</span> " + icon(t ? t[2] : "square") + " " + esc(t ? t[1] : KIND[q.k]) +
          ' <span class="o-muted">' + esc(cueLabel(q) ? "· " + cueLabel(q) : "") + "</span></button>" + '<span class="o-grow"></span>' + mover("cues", i, it.cues.length) + "</div>";
      }).join("");
      var c = it.cues[S.cue], p = "cues." + S.cue + ".", fields = "";
      if (c) {
        if (c.k === "response") fields = row("Header", field(p + "h", c.h, { ph: "TUBAG:", small: true })) + row("Response", field(p + "b", c.b, { rows: 2, ph: "Congregation response" }));
        else if (c.k === "general") fields = row("Header", field(p + "h", c.h, { ph: "Optional" })) + row("Text", field(p + "b", c.b, { rows: 3 })) + row("Footer", field(p + "f", c.f, { ph: "Optional" }));
        else if (c.k === "trigger") fields = row("Role", choose(p + "who", c.who, Object.keys(WHO).filter(function (k) { return k !== "T"; }).map(function (k) { return [k, WHO[k]]; }))) + row("Private prompt", field(p + "b", c.b, { ph: "The line that cues the next step" }));
        else fields = row("Label", field(p + "b", c.b, { ph: KIND[c.k] }), c.k === "wait" ? "Program stays as it is." : c.k === "hold" ? "Black checkpoint; the next Cue goes live as usual." : "Program stays black until Space.");
      }
      return '<div class="pl-ed-body"><div class="pl-form">' + row("Title", field("title", it.title, { ph: "What this moment is called" })) + "</div>" +
        '<div class="pl-block"><div class="pl-block-head"><b>Cues</b><span class="o-meta">In Presenter order · ' + it.cues.length + "</span></div>" + list +
        '<div class="pl-addcue"><button class="o-btn" data-act="cue-menu" data-focus-key="addcue" aria-expanded="' + S.cueMenu + '">' + icon("plus") + "Add Cue" + icon("chevron-down") + "</button>" +
        (S.cueMenu ? '<div class="o-menu pl-cue-menu" role="menu">' + types + "</div>" : "") + "</div></div>" +
        (c ? '<div class="pl-block"><div class="pl-block-head"><b>Cue ' + (S.cue + 1) + " · " + esc(KIND[c.k]) + "</b>" + (OP_ONLY[c.k] ? '<span class="pl-chip">Operator only</span>' : '<span class="pl-chip">Projected</span>') + '</div><div class="pl-form">' + fields + "</div></div>" : "") +
        "</div>";
    }
    function cueEditor(it) {
      if (it.empty) {
        return '<div class="pl-empty">' + icon("list-plus", "o-icon-20") + "<b>This " + esc(it.type) + " is empty</b>" +
          '<div class="o-meta">Its editor is a separate screen, not part of this mockup.</div></div>';
      }
      return '<div class="pl-ed-body"><div class="pl-block"><div class="pl-block-head"><b>' + esc(it.type) + '</b><span class="o-meta">' + it.cues.length + (it.cues.length === 1 ? " Cue" : " Cues") + "</span></div>" +
        it.cues.map(function (q, i) {
          var lead = q.k === "trigger" ? '<span class="pr-who">' + esc(q.who) + "</span>" : q.k === "unavailable" ? icon("circle-off") :
            q.k === "wait" ? icon("hourglass") : q.k === "hold" ? icon("square") : q.k === "blackout" ? icon("moon") : icon("monitor");
          var tag = q.k === "trigger" ? '<span class="o-meta">Operator only</span>' : OP_ONLY[q.k] ? '<span class="o-meta">' + KIND[q.k] + " · operator only</span>" :
            q.k === "unavailable" ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : '<span class="o-meta">Output</span>';
          var sub = q.k === "trigger" ? (WHO[q.who] || q.who) + " · private timing prompt" : q.k === "response" ? "Congregation response" : q.k === "blackout" ? "Program stays black until Space" : KIND[q.k];
          return '<button class="o-row pl-cue" data-cue="' + i + '"' + (i === S.cue ? ' aria-selected="true"' : "") + '><span class="o-meta">' + (i + 1) + "</span>" + lead +
            '<span class="o-grow"><span class="o-ellipsis pl-cue-t">' + esc(cueLabel(q)) + '</span><span class="o-meta">' + esc(sub) + "</span></span>" + tag + "</button>";
        }).join("") + "</div></div>";
    }
    function editor(it, wide) {
      var nBad = bad(it);
      var body = it.type === "Song" ? songEditor(it, wide) : it.rd ? readingEditor(it, wide) : it.ex ? responseEditor(it, wide) : it.gp ? generalEditor(it, wide) :
        it.hold ? holdEditor(it) : it.custom ? customEditor(it, wide) : cueEditor(it);
      var sub = it.song || it.src || "";
      return '<section class="o-pane pl-editor" aria-label="Selected Mass item"><div class="o-pane-head"><div class="o-grow"><div class="o-meta">' + esc(S.sec[it.section].title) + " · " + esc(it.type) + "</div><b>" + it.n + ". " + esc(it.title) + "</b>" +
        (sub && sub !== it.title ? ' <span class="o-muted">' + esc(sub) + "</span>" : "") + "</div>" + (it.theme ? '<span class="o-meta">' + icon("palette") + " " + esc(themeName(it.theme)) + "</span>" : "") + "</div>" +
        (nBad ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div><b>" + nBad + " Cues unavailable.</b> " + esc(it.cues[0].why) + '. <button class="pl-link" data-act="picker" data-focus-key="fix">Choose another Arrangement</button></div></div>' : "") +
        (it.warn ? '<div class="o-notice" data-state="info">' + icon("info") + "<div><b>Pagination not reviewed.</b> " + esc(it.warn) + '. <button class="pl-link" data-focus-key="fix">Review pagination</button></div></div>' : "") +
        '<div class="o-pane-body">' + body + "</div></section>";
    }
    function inspector(it, wide) {
      var n = it.cues.length, c = it.cues[S.cue];
      var head = '<div class="o-pane-head"><b>Inspector</b>';
      if (!c) return '<aside class="o-pane pl-inspector" aria-label="Inspector">' + head + '</div><div class="o-pane-body pl-insp-body">' + tslide(null, th(it)) + '<div class="o-meta">Nothing to inspect: this item has no Cues yet.</div></div></aside>';
      var first = S.cue === 0, last = S.cue === n - 1, opOnly = !!OP_ONLY[c.k];
      var frame = opOnly ? tslide(null, th(it)) : tslide(c, th(it));
      var ctx = "";
      if (c.k === "trigger") ctx = '<div class="pr-private"><span class="pr-who">' + esc(c.who) + "</span><div><b>" + esc(WHO[c.who] || c.who) + ": " + esc(c.b || "(no prompt yet)") + '</b><div class="o-meta">Advance when this line is finished.</div></div></div>';
      else if (opOnly) ctx = '<div class="pr-private">' + icon(c.k === "wait" ? "hourglass" : c.k === "hold" ? "square" : "moon") + "<div><b>" + KIND[c.k] + " · " + esc(c.b) + '</b><div class="o-meta">' +
        (c.k === "wait" ? "Program stays as it is" : c.k === "hold" ? "Black checkpoint, not Blackout" : "Program stays black until Space takes Preview live") + "</div></div></div>";
      else if (c.k === "unavailable") ctx = '<div class="pr-private" data-state="unavailable">' + icon("circle-off") + "<div><b>Unavailable</b><div>" + esc(c.why) + "</div></div></div>";
      else if (c.k === "reading-id" && it.rd && !it.rd.project) ctx = '<div class="pr-private">' + icon("book-open") + '<div><b>Reading text stays private</b><div class="o-meta">In Presenter you see the passage beside this frame; the congregation sees only the title.</div></div></div>';
      else if (it.gp && it.gp.note) ctx = '<div class="pr-private">' + icon("sticky-note") + "<div><b>Private delivery context</b><div>" + esc(it.gp.note) + "</div></div></div>";
      var all = items(), nextItem = all[it.n], prevItem = all[it.n - 2];
      var edge = last ? "End of " + it.title + "." + (nextItem ? " Select " + nextItem.title + " in the Mass outline to inspect it." : "") : first && prevItem ? "Start of " + it.title + "." : "";
      var meta = [["Congregation frame", opOnly ? "Black" : c.k === "unavailable" ? "Unavailable" : "Canonical 16:9"], ["Theme", themeName(th(it)) + (it.theme ? " · item override" : "")],
        ["Pagination", c.k === "unavailable" ? "—" : it.warn ? "Not reviewed" : "Fits safely"], ["Output", "1920 × 1080"]];
      if (it.gp && !opOnly) meta.push(["Auto-play", (it.gp.slides[c.slide] && it.gp.slides[c.slide].dur || it.gp.dur) + " s"]);
      if (wide || it.local) meta.push(["Source", it.local ? (it.base ? "Mass-local override" : "Mass-local") : it.song ? "Library Arrangement" : it.src ? "Library · " + it.src : "Mass item"]);
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
      var st = staged(), it = S.sel, q = S.pick.q.toLowerCase(), kind = S.pick.kind;
      var list, preview = "", title, ph;
      if (kind === "song") {
        title = it.song ? "Change Song or Arrangement" : "Choose Song"; ph = "Search Songs and composers";
        list = LIB.map(function (song, si) {
          if (q && (song.title + " " + song.by).toLowerCase().indexOf(q) < 0) return "";
          var open = S.pick.song === si;
          var r = '<button class="o-row pl-result" data-song="' + si + '" aria-expanded="' + open + '">' + icon(open ? "chevron-down" : "chevron-right") +
            '<span class="o-grow"><b>' + esc(song.title) + '</b> <span class="o-meta">' + esc(song.by) + '</span></span><span class="o-meta">' + song.arrs.length + (song.arrs.length === 1 ? " Arrangement" : " Arrangements") + "</span></button>";
          if (!open) return r;
          return r + '<div class="pl-arrs">' + song.arrs.map(function (a, ai) {
            var cur = it.song === song.title && it.arr === a.name, sel = S.pick.arr === ai;
            return '<button class="o-row pl-arr" data-arr="' + ai + '"' + (sel ? ' aria-selected="true"' : "") + (a.broken ? " disabled" : "") + ">" + icon(sel ? "circle-dot" : "circle") +
              '<span class="o-grow"><span class="o-label">' + esc(a.name) + '</span><span class="o-meta">' + (a.broken ? esc(a.broken) : a.cues.length + " Cues · " + esc(K.MASS.language)) + "</span></span>" +
              (cur ? '<span class="pl-chip">Current</span>' : "") + (a.broken ? '<span class="o-status" data-state="unavailable">Unavailable</span>' : "") + "</button>";
          }).join("") + "</div>";
        }).join("");
        if (st) {
          preview = '<div class="pl-staged"><div class="pl-block-head"><b>Staged</b><span class="o-meta">' + esc(st.name) + "</span></div>" + tslide(st.arr.cues.filter(projectable)[0], th(it)) +
            '<div class="pl-seqchips">' + st.arr.cues.map(function (c) { return '<span class="pl-chip">' + esc(c.k === "song-id" ? "Title" : c.h) + "</span>"; }).join("") + "</div>" +
            (it.local ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div>This item has Mass-local changes. Using another Arrangement asks you to confirm first.</div></div>" : "") + "</div>";
        }
      } else {
        var lib = libFor(kind), noun = kind === "response" ? "Response or Prayer" : "General Presentation";
        title = it.src || it.local ? "Change " + noun : "Choose " + noun; ph = "Search " + (kind === "response" ? "Responses & Prayers" : "General Presentations");
        list = lib.map(function (e, ei) {
          if (q && (e.name + " " + e.note).toLowerCase().indexOf(q) < 0) return "";
          var sel = S.pick.song === ei, count = kind === "response" ? e.ex.length + (e.ex.length === 1 ? " exchange" : " exchanges") : e.slides.length + (e.slides.length === 1 ? " Slide" : " Slides");
          return '<button class="o-row pl-arr pl-entry" data-entry="' + ei + '"' + (sel ? ' aria-selected="true"' : "") + ">" + icon(sel ? "circle-dot" : "circle") +
            '<span class="o-grow"><span class="o-label">' + esc(e.name) + '</span><span class="o-meta">' + esc(e.note) + " · " + count + "</span></span>" + (it.src === e.name && !it.pending ? '<span class="pl-chip">Current</span>' : "") + "</button>";
        }).join("");
        list += '<button class="o-row pl-arr pl-entry" data-act="local-new">' + icon("pencil") + '<span class="o-grow"><span class="o-label">Write for this Mass only</span><span class="o-meta">Starts empty; not added to the Library</span></span></button>';
        if (st) {
          var probe = { ex: st.entry.ex ? copy(st.entry.ex) : null, hdr: st.entry.hdr, hdrText: "", gp: st.entry.slides ? { slides: copy(st.entry.slides), dur: st.entry.dur } : null };
          regen(probe);
          preview = '<div class="pl-staged"><div class="pl-block-head"><b>Staged</b><span class="o-meta">' + esc(st.name) + "</span></div>" + tslide(probe.cues.filter(projectable)[0], th(it)) +
            '<div class="pl-seqchips">' + probe.cues.map(function (c) { return '<span class="pl-chip">' + esc(OP_ONLY[c.k] ? (WHO[c.who] || KIND[c.k]) + " · private" : KIND[c.k]) + "</span>"; }).join("") + "</div>" +
            (it.local ? '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div>This item has Mass-local changes. Using another source asks you to confirm first.</div></div>" : "") + "</div>";
        }
      }
      if (!list) list = '<div class="o-meta pl-none">Nothing matches “' + esc(S.pick.q) + "”.</div>";
      var use = st ? (isCurrent(st) ? "Already in use" : st.use) : (kind === "song" ? "Use arrangement" : "Use");
      return '<div class="o-scrim pl-scrim--light"></div><div class="o-slideover pl-picker' + (S.child ? " is-receded" : "") + '" role="dialog" aria-modal="true" aria-labelledby="pl-pick-t">' + closeTab("Close picker") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="pl-pick-t">' + title + "</h2>" +
        '<div class="o-meta">For ' + it.n + ". " + esc(it.title) + " · " + esc(S.sec[it.section].title) + "</div></div>" +
        '<button class="o-btn o-btn--primary" data-act="use"' + (!st || isCurrent(st) ? " disabled" : "") + ">" + icon("check") + esc(use) + "</button></div>" +
        '<div class="o-overlay-body"><div class="o-field">' + icon("search") + '<input data-input="q" data-focus-key="q" placeholder="' + ph + '" value="' + esc(S.pick.q) + '" aria-label="' + ph + '"></div>' +
        preview + '<div class="pl-results">' + list + "</div></div></div>";
    }
    function changesList(it) {
      var ch = it.changes.length ? it.changes : ["No edits yet beyond the copy"];
      return '<ul class="pl-changes">' + ch.map(function (c) { return "<li>" + esc(c) + "</li>"; }).join("") + "</ul>";
    }
    function review() {
      var st = staged(), it = S.sel;
      return '<div class="o-slideover pl-child" role="dialog" aria-modal="true" aria-labelledby="pl-rev-t">' +
        '<div class="o-overlay-head"><button class="o-btn o-btn--quiet o-btn--icon" data-act="back" aria-label="Back to picker" title="Back">' + icon("arrow-left") + "</button>" +
        '<div class="o-grow"><h2 class="o-h2" id="pl-rev-t">Replace customized content?</h2><div class="o-meta">' + it.n + ". " + esc(it.title) + "</div></div></div>" +
        '<div class="o-overlay-body"><div class="pl-compare"><div><div class="o-section-label">Current</div><b>' + esc(currentName(it) || "Written for this Mass") + '</b><div class="o-meta">Mass-local' + (it.base ? " override" : "") + "</div>" + changesList(it) + "</div>" +
        '<div><div class="o-section-label">Replacement</div><b>' + esc(st.name) + '</b><div class="o-meta">From the Library</div></div></div>' +
        '<div class="o-notice" data-state="warning">' + icon("triangle-alert") + "<div>The local changes are detached from this item; Ordo never merges them into other content. Undo brings them back.</div></div></div>" +
        '<div class="o-overlay-foot"><button class="o-btn" data-act="back">Back</button><button class="o-btn o-btn--primary" data-act="confirm-replace">Replace</button></div></div>';
    }
    function returnConfirm() {
      var it = S.sel;
      return '<div class="o-scrim"></div><div class="o-slideover" role="dialog" aria-modal="true" aria-labelledby="pl-ret-t">' + closeTab("Cancel") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="pl-ret-t">Return to library ' + (it.song ? "arrangement" : "source") + "?</h2><div class=\"o-meta\">" + it.n + ". " + esc(it.title) + "</div></div></div>" +
        '<div class="o-overlay-body"><div class="pl-compare"><div><div class="o-section-label">Library ' + (it.song ? "Arrangement" : "source") + "</div><b>" + esc(it.song || it.src) + '</b><div class="o-meta">' + (it.song ? esc(it.arr) + " · " : "") + it.base.cues.length + " Cues</div></div>" +
        '<div><div class="o-section-label">Local changes removed</div>' + changesList(it) + "</div></div>" +
        '<p class="o-meta">The item follows the Library again. Undo restores the local copy.</p></div>' +
        '<div class="o-overlay-foot"><button class="o-btn" data-act="close">Cancel</button><button class="o-btn o-btn--primary" data-act="confirm-return">Return to library ' + (it.song ? "arrangement" : "source") + "</button></div></div>";
    }
    function readiness() {
      var f = findings();
      function group(title, list, state) {
        if (!list.length) return "";
        return '<div class="pl-fgroup"><div class="o-section-label">' + title + "</div>" + list.map(function (x) {
          return '<div class="pl-find">' + statusLabel(state, "", state === "warning" ? "diamond" : null) + '<div class="o-grow"><button class="pl-link pl-find-t" data-find="' + x.it.n + '">' + x.it.n + ". " + esc(x.it.title) + "</button>" +
            "<div><b>" + esc(x.title) + ".</b> " + esc(x.text) + '</div><button class="o-btn" data-fix="' + x.it.n + '" data-ff="' + x.focus + '">' + esc(x.action) + "</button></div></div>";
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
      var sample = c && projectable(c) ? c : S.sel.cues.filter(projectable)[0] || null;
      return '<div class="o-scrim"></div><div class="o-slideover pl-theme-panel' + (S.child ? " is-receded" : "") + '" role="dialog" aria-modal="true" aria-labelledby="pl-th-t">' + closeTab("Close Mass Theme") +
        '<div class="o-overlay-head"><div class="o-grow"><h2 class="o-h2" id="pl-th-t">Mass Theme</h2><div class="o-meta">Applies to every item that doesn\'t keep its own Theme</div></div>' +
        '<button class="o-btn o-btn--primary" data-act="apply-theme"' + (changed ? "" : " disabled") + ">" + icon("check") + (S.stagedTheme !== S.theme ? "Apply " + esc(themeName(S.stagedTheme)) : changed ? "Apply override changes" : "Apply") + "</button></div>" +
        '<div class="o-overlay-body"><div class="pl-block-head"><b>Preview</b><span class="o-meta">' + S.sel.n + ". " + esc(S.sel.title) + " · Cue " + (S.cue + 1) + (S.sel.theme ? " · keeps its override" : "") + "</span></div>" +
        tslide(sample, S.sel.theme && S.keepOverride[S.sel.n] !== false ? S.sel.theme : S.stagedTheme) +
        disclose + '<div class="pl-themes">' + rows + "</div>" +
        '<button class="o-btn o-btn--quiet">' + icon("pencil") + "Edit selected Theme</button></div></div>";
    }
    function overridesPanel() {
      var rows = overridden().map(function (it) {
        var keep = S.keepOverride[it.n] !== false, sample = it.cues.filter(projectable)[0] || null;
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
        '<div class="pl-panes">' + outline(wide, f) + editor(it, wide) + inspector(it, wide) + "</div></div></div>" + ov + toast;
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
        var t = app.querySelector('[data-focus-key="' + want + '"]') || (S.focus ? app.querySelector('[data-focus-key^="' + want + '."]') : null);
        if (t) {
          t.focus({ preventScroll: !S.focus });
          if (S.focus) { t.classList.add("o-focus"); var pane = t.closest(".o-pane-body"); if (pane) { var kk = app.getBoundingClientRect().width / app.offsetWidth || 1; pane.scrollTop += (t.getBoundingClientRect().top - pane.getBoundingClientRect().top) / kk - 88; } }
          if (caret != null && t.setSelectionRange && !S.focus) try { t.setSelectionRange(caret, caret); } catch (err) { /* number inputs */ }
        }
      }
      S.focus = null;
      if (opts.afterRender) opts.afterRender(api);
    }

    app.addEventListener("click", function (e) {
      var t = e.target.closest("[data-act],[data-item],[data-cue],[data-step],[data-add],[data-song],[data-arr],[data-entry],[data-theme],[data-ovr],[data-find],[data-fix],[data-mv],[data-del],[data-addex],[data-addslide],[data-addcue]");
      if (!t || t.disabled) return;
      e.preventDefault();
      var d = t.dataset, a = d.act;
      if (a !== "undo") { S.toast = null; S.undo = null; }   // a toast and its Undo last until the next action
      if (a !== "cue-menu" && d.addcue == null) S.cueMenu = false;
      if (a === "add") { if (S.overlay === "add") S.overlay = null; else openAdd(); }
      else if (d.add) addItem(d.add);
      else if (a === "picker") openPicker();
      else if (a === "local-new") writeLocal();
      else if (d.song != null) { var si = +d.song; S.pick.song = S.pick.song === si ? null : si; S.pick.arr = null; }
      else if (d.arr != null) S.pick.arr = +d.arr;
      else if (d.entry != null) { S.pick.song = +d.entry; S.pick.arr = 0; }
      else if (a === "use") commitPick();
      else if (a === "confirm-replace") applyPick(staged());
      else if (a === "back") S.child = null;
      else if (a === "close") { if (S.child) S.child = null; else S.overlay = null; }
      else if (a === "customize") customize();
      else if (a === "return") S.overlay = "return";
      else if (a === "confirm-return") returnToLibrary();
      else if (a === "fork") { var c = S.sel.cues[S.cue]; c.forked = true; change("Forked one " + c.h + " occurrence"); }
      else if (d.mv) { var p = d.mv.split(":"); if (p[0] === "cues" && S.sel.type === "Song") { var cs = S.sel.cues, i = +p[1], j = i + +p[2], x = cs[i]; cs[i] = cs[j]; cs[j] = x; S.cue = j; change("Reordered the sequence"); } else moveEntry(p[0], +p[1], +p[2]); }
      else if (d.del) { var q = d.del.split(":"); if (q[0] === "cues" && S.sel.type === "Song") { var gone = S.sel.cues.splice(+q[1], 1)[0]; S.cue = Math.max(0, Math.min(S.cue, S.sel.cues.length - 1)); change("Removed a " + (gone.h || "title") + " occurrence"); } else removeEntry(q[0], +q[1]); }
      else if (d.addex != null) { S.sel.ex.push({ who: "P", trig: "", rwho: "T", resp: "" }); regen(S.sel); S.cue = S.sel.cues.length - 1; S.focus = "ex." + (S.sel.ex.length - 1) + ".trig"; change("Added an exchange"); }
      else if (d.addslide) { S.sel.gp.slides.push(d.addslide === "image" ? { kind: "image", file: "", alt: "", dur: null } : { kind: "text", h: "", b: "", f: "", dur: null }); regen(S.sel); S.cue = S.sel.cues.length - 1;
        S.focus = "gp.slides." + (S.sel.gp.slides.length - 1) + (d.addslide === "image" ? ".file" : ".b"); change("Added a Slide"); }
      else if (a === "cue-menu") S.cueMenu = !S.cueMenu;
      else if (d.addcue) { S.sel.cues.push(newCue(d.addcue)); S.cue = S.sel.cues.length - 1; S.cueMenu = false; S.focus = "cues." + S.cue + ".b"; }
      else if (a === "readiness") S.overlay = S.overlay === "readiness" ? null : "readiness";
      else if (d.find) select(+d.find);
      else if (d.fix) { select(+d.fix); S.overlay = null; S.focus = d.ff || "fix"; }
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
      var el = e.target, k = el.dataset.input; if (!k) return;
      if (k === "q") S.pick.q = el.value;
      else if (k === "text") editText(el.value);
      else if (k === "f") {
        var v = el.type === "checkbox" ? el.checked : el.type === "number" ? (el.value === "" ? null : +el.value) : el.value;
        editField(el.dataset.path, v);
      }
      render();
    });
    app.addEventListener("change", function (e) {
      if (e.target.hasAttribute("data-dest")) { var p = e.target.value.split(":"); S.dest = { sec: +p[0], at: +p[1] }; render(); }
    });
    app.addEventListener("keydown", function (e) {
      if (e.key !== "Escape" || !(S.overlay || S.cueMenu)) return;
      if (S.cueMenu) S.cueMenu = false; else if (S.child) S.child = null; else S.overlay = null;
      e.preventDefault(); render();
    });

    // A small scripting surface so a page can open the planner in a given state.
    var api = {
      app: app, state: S, render: render, items: items,
      select: function (n) { select(n); return api; }, cue: function (i) { S.cue = i; return api; },
      openAdd: function () { openAdd(); return api; }, add: function (type) { openAdd(); addItem(type); return api; },
      openPicker: function () { openPicker(); return api; }, stage: function (name, arr) { stage(name, arr); return api; },
      commit: function () { commitPick(); return api; }, readiness: function () { S.overlay = "readiness"; return api; },
      theme: function (id) { openTheme(); if (id) S.stagedTheme = id; return api; }, overrides: function () { S.child = "overrides"; return api; },
      customize: function () { customize(); S.toast = null; return api; }, edit: function (v) { editText(v); return api; },
      set: function (path, v) { editField(path, v); return api; }, writeLocal: function () { writeLocal(); return api; },
      addExchange: function (x) { S.sel.ex.push(x); regen(S.sel); return api; },
      addSlide: function (s) { S.sel.gp.slides.push(s); regen(S.sel); S.cue = S.sel.cues.length - 1; return api; },
      addCue: function (c) { S.sel.cues.push(c); S.cue = S.sel.cues.length - 1; return api; },
      cueMenu: function () { S.cueMenu = true; S.focus = "addcue"; return api; },
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
