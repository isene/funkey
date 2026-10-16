// The lobby: the newest game on top, the cards, a search box and the
// kinds, each game's best score and comment count, the latest comments.
// Everything that comes from the server goes in as text.
(function () {
  "use strict";
  const R = window.ROOM;
  const names = Object.keys(R.GAMES);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const link = name => "play.html?g=" + name;
  const shot = (name, g) => {
    const i = el("img", "shot");
    i.src = "img/" + name + ".png";
    i.alt = g.alt;
    i.loading = "lazy";
    i.width = 960;
    i.height = 600;
    return i;
  };

  // The newest game, large.
  const fresh = names.find(n => R.GAMES[n].isnew);
  if (fresh) {
    const g = R.GAMES[fresh];
    const hero = document.getElementById("hero");
    const a = el("a", "hero-card");
    a.href = link(fresh);
    a.append(shot(fresh, g));
    const t = el("div", "hero-text");
    t.append(el("span", "badge", "New"), el("h2", "", fresh), el("p", "", g.about), el("span", "play", "Play " + fresh));
    a.append(t);
    hero.append(a);
    hero.hidden = false;
  }

  // The cards.
  const cards = {};
  const grid = document.getElementById("grid");
  for (const name of names) {
    const g = R.GAMES[name];
    const a = el("a", "card");
    a.href = link(name);
    a.dataset.name = name;
    const pic = el("div", "pic");
    pic.append(shot(name, g));
    if (g.isnew) pic.append(el("span", "badge", "New"));
    const body = el("div", "body");
    const head = el("div", "head");
    head.append(el("h3", "", name), el("span", "kinds", g.kinds.join(" · ")));
    const meta = el("div", "meta");
    const best = el("span", "best");
    const talk = el("span", "count");
    meta.append(best, talk);
    body.append(head, el("p", "", g.line), meta);
    a.append(pic, body);
    grid.append(a);
    cards[name] = { a, best, talk, text: (name + " " + g.kinds.join(" ") + " " + g.line + " " + g.about).toLowerCase() };
  }

  // Search and kinds.
  const search = document.getElementById("search");
  const chips = document.getElementById("chips");
  const none = document.getElementById("none");
  let kind = "";
  const chip = (label, value) => {
    const b = el("button", "chip", label);
    b.type = "button";
    b.setAttribute("aria-pressed", value === kind ? "true" : "false");
    b.addEventListener("click", () => { kind = value; show(); });
    b.dataset.kind = value;
    return b;
  };
  chips.append(chip("All", ""), ...R.KINDS.map(k => chip(k, k)));
  function show() {
    const words = search.value.toLowerCase().split(/\s+/).filter(Boolean);
    let n = 0;
    for (const name of names) {
      const c = cards[name];
      const on = (!kind || R.GAMES[name].kinds.includes(kind)) && words.every(w => c.text.includes(w));
      c.a.hidden = !on;
      if (on) n++;
    }
    for (const b of chips.children) b.setAttribute("aria-pressed", b.dataset.kind === kind ? "true" : "false");
    none.hidden = n > 0;
  }
  search.addEventListener("input", show);
  search.addEventListener("keydown", e => {
    if (e.key !== "Enter") return;
    const first = names.find(n => !cards[n].a.hidden);
    if (first) location.href = link(first);
  });
  document.getElementById("clear").addEventListener("click", () => { search.value = ""; kind = ""; show(); search.focus(); });

  // The best score of each game: lines of "game NAME SCORE".
  fetch(R.SCORES + "all").then(r => r.ok ? r.text() : "").then(t => {
    for (const line of t.split("\n")) {
      const [g, who, score] = line.split(" ");
      if (cards[g] && who && score) cards[g].best.textContent = "Best " + who + " " + Number(score).toLocaleString();
    }
  }).catch(() => {});

  // Comment counts and the latest comments.
  fetch(R.COMMENTS + "all").then(r => r.ok ? r.json() : null).then(j => {
    if (!j) return;
    for (const g of names) {
      const n = (j.counts && j.counts[g]) | 0;
      cards[g].talk.textContent = n === 1 ? "1 comment" : n + " comments";
    }
    const latest = (j.latest || []).filter(c => R.game(c.g));
    if (!latest.length) return;
    const list = document.getElementById("talk");
    for (const c of latest) {
      const li = el("li");
      const a = el("a", "", "");
      a.href = link(c.g) + "#comments";
      const top = el("div", "who");
      top.append(el("strong", "", String(c.n)), el("span", "", " on "), el("em", "", c.g), el("span", "time", " · " + R.when(c.t)));
      a.append(top, el("p", "", String(c.c).length > 220 ? String(c.c).slice(0, 220) + "…" : String(c.c)));
      li.append(a);
      list.append(li);
    }
    document.getElementById("latest").hidden = false;
  }).catch(() => {});
})();
