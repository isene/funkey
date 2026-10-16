// One page for every game in the room: play.html?g=wargame. The name only
// picks a row of ROOM.GAMES; nothing from the address goes into the page.
// Names, scores and comments from the server go in as text, never HTML.
(async function () {
  "use strict";
  const R = window.ROOM;
  const name = new URLSearchParams(location.search).get("g");
  const g = R.game(name);
  if (!g) { document.getElementById("none").hidden = false; return; }
  const $ = id => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };

  document.title = name + " · Game Room";
  $("name").textContent = name;
  $("kinds").textContent = g.kinds.join(" · ");
  $("about").textContent = g.about;
  $("help").innerHTML = g.help;
  $("src").href = "https://github.com/isene/funkey/" + (name === "wargame" ? "tree/master/examples/wargame" : "blob/master/examples/" + name + ".rs");
  $("room").hidden = false;
  const box = $("game");
  // Tall games stay on the screen: the width follows the height left.
  box.style.maxWidth = "min(100%, calc(78vh * " + g.size[0] + " / " + g.size[1] + "))";

  // More games.
  for (const n of Object.keys(R.GAMES)) {
    if (n === name) continue;
    const li = el("li");
    const a = el("a");
    a.href = "play.html?g=" + n;
    a.append(el("strong", "", n), el("span", "", R.GAMES[n].line));
    li.append(a);
    $("more").append(li);
  }

  // Keys on the screen for phones, and full screen.
  const button = ([k, label, small]) => {
    const b = el("button");
    b.type = "button";
    if (!k) { b.className = "gap"; b.tabIndex = -1; return b; }
    b.dataset.key = k;
    b.textContent = label;
    if (small) b.append(el("small", "", small));
    return b;
  };
  const group = (cls, list) => {
    const d = el("div", cls);
    d.append(...list.map(button));
    return d;
  };
  const pad = $("pad");
  pad.append(group("move", g.move), group(g.wide ? "keys wide" : "keys", g.keys));
  pad.hidden = !(window.matchMedia && matchMedia("(pointer: coarse)").matches);
  $("keys").addEventListener("click", () => { pad.hidden = !pad.hidden; });
  if (!document.fullscreenEnabled) $("full").hidden = true;
  $("full").addEventListener("click", () => box.requestFullscreen().catch(() => {}));

  // The top ten beside the game.
  const showScores = text => {
    const list = $("scores");
    list.textContent = "";
    for (const line of text.split("\n")) {
      const [who, score] = line.split(" ");
      if (!who || !score) continue;
      const li = el("li");
      li.append(el("span", "who", who), el("span", "pts", Number(score).toLocaleString()));
      list.append(li);
    }
    $("noscores").hidden = list.children.length > 0;
  };
  if (g.scores) {
    $("top").hidden = false;
    fetch(R.SCORES + name).then(r => r.ok ? r.text() : "").then(showScores).catch(() => {});
  }

  // Comments, newest first.
  const showTalk = list => {
    const ol = $("talk");
    ol.textContent = "";
    for (const c of list.slice().reverse()) {
      const li = el("li");
      const who = el("div", "who");
      who.append(el("strong", "", String(c.n)), el("span", "time", R.when(c.t)));
      li.append(who, el("p", "", String(c.c)));
      ol.append(li);
    }
    $("empty").hidden = list.length > 0;
  };
  fetch(R.COMMENTS + name).then(r => r.ok ? r.json() : []).then(showTalk).catch(() => {});
  $("who").value = R.store("room-name") || "";
  $("say").addEventListener("submit", e => {
    e.preventDefault();
    const who = $("who").value.trim(), text = $("text").value.trim();
    if (!text) { $("text").focus(); return; }
    R.store("room-name", who);
    $("send").disabled = true;
    fetch(R.COMMENTS + name, { method: "POST", body: who.replace(/\n/g, " ") + "\n" + text })
      .then(r => r.ok ? r.json().then(list => { showTalk(list); $("text").value = ""; $("note").textContent = "Thank you. It is up."; })
        : r.text().then(t => { $("note").textContent = t.trim() || "That did not go through. Try again in a minute."; }))
      .catch(() => { $("note").textContent = "That did not go through. Try again in a minute."; })
      .finally(() => { $("send").disabled = false; });
  });

  // The game, with what this browser kept from earlier visits.
  const KEEP = "keep:" + name + "/";
  const kept = {};
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k && k.startsWith(KEEP)) kept[k.slice(5)] = localStorage.getItem(k);
    }
  } catch (e) { /* nothing kept */ }
  const canvas = $("screen");
  canvas.setAttribute("aria-label", name);
  const game = await funkey.play(canvas, R.file(name), kept);
  funkey.pad(pad, game);

  // What the game says: "keep game/key" and a value to keep; "list" for
  // the top ten; "score ABC 12345" for a new score. The list goes back as
  // "scores" and a line per score.
  const answer = r => r.ok ? r.text().then(t => { game.send("scores\n" + t); showScores(t); }) : null;
  game.listen(m => {
    if (m.startsWith("keep ")) {
      const at = m.indexOf("\n");
      const key = m.slice(5, at);
      if (at > 0 && key.startsWith(name + "/")) R.store("keep:" + key, m.slice(at + 1));
    } else if (m === "list" && g.scores) {
      const n = R.store("funkey-name") || R.store("stack-name");
      if (n) game.send("name " + n);
      fetch(R.SCORES + name).then(answer).catch(() => {});
    } else if (m.startsWith("score ") && g.scores) {
      const line = m.slice(6);
      R.store("funkey-name", line.slice(0, 3));
      fetch(R.SCORES + name, { method: "POST", body: line }).then(answer).catch(() => {});
    }
  });
})();
