// Tests for the game room (room/) in a hidden Chromium: the room is served
// on localhost, with server/scores.rb and server/comments.rb run as nginx
// runs them, on lists in a scratch folder. Nothing goes to isene.com.
// Needs node, playwright and ruby.
//
//   node web/room-test.js
const http = require("http"), fs = require("fs"), os = require("os"), path = require("path");
const { spawn } = require("child_process");
const { chromium } = require("playwright");

const top = path.join(__dirname, "..");
const room = path.join(top, "room");
const lists = fs.mkdtempSync(path.join(os.tmpdir(), "room-test-"));
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".png": "image/png",
  ".svg": "image/svg+xml", ".wasm": "application/wasm" };
let fail = 0;
const check = (what, ok, got) => {
  console.log((ok ? "ok   " : "FAIL ") + what + (ok ? "" : ": " + JSON.stringify(got)));
  if (!ok) fail = 1;
};

const server = http.createServer((req, res) => {
  const u = new URL(req.url, "http://x");
  const cgi = u.pathname.match(/^\/cgi-bin\/funkey-(scores|comments)\.rb$/);
  if (cgi) {
    const body = [];
    req.on("data", c => body.push(c));
    req.on("end", () => {
      const b = Buffer.concat(body);
      const p = spawn("ruby", [path.join(top, "server", cgi[1] + ".rb")], { env: { PATH: "/usr/bin:/bin", FUNKEY_SCORES: lists,
        REQUEST_METHOD: req.method, QUERY_STRING: u.search.slice(1), REMOTE_ADDR: "10.0.0." + (1 + Math.floor(Math.random() * 250)),
        CONTENT_LENGTH: String(b.length) } });
      const out = [];
      p.stdout.on("data", c => out.push(c));
      p.on("close", () => {
        const t = Buffer.concat(out).toString();
        res.writeHead(parseInt(t.slice(8, 11)), { "Content-Type": "text/plain; charset=utf-8" });
        res.end(t.slice(t.indexOf("\r\n\r\n") + 4));
      });
      p.stdin.end(b);
    });
    return;
  }
  let f = path.join(room, path.normalize(decodeURIComponent(u.pathname)));
  if (f.endsWith("/")) f += "index.html";
  fs.readFile(f, (e, d) => {
    if (e) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { "Content-Type": TYPES[path.extname(f)] || "application/octet-stream" });
    res.end(d);
  });
});

server.listen(0, "127.0.0.1", async () => {
  const base = "http://127.0.0.1:" + server.address().port + "/";
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on("console", m => { if (m.type() === "error") errors.push(m.text()); });
  page.on("pageerror", e => errors.push(String(e)));
  await page.route(u => !u.href.startsWith(base), r => r.abort());
  try {
    await page.goto(base);
    await page.waitForSelector(".card");
    check("the lobby shows twelve games", (await page.$$(".card")).length === 12);
    check("the lobby greets", (await page.textContent("h1")) === "Welcome to the funkey game room.");
    // The heading's gradient is painted only inside its box: the tail of
    // the g in "funkey" once fell outside it. Look for gold just under
    // the line.
    const box = await page.$eval("h1", h => {
      const r = h.getBoundingClientRect(), s = getComputedStyle(h);
      return { x: r.x, y: r.y + parseFloat(s.lineHeight), w: r.width };
    });
    const strip = await page.screenshot({ clip: { x: box.x, y: box.y, width: box.w, height: 6 } });
    const { data, info } = await require("sharp")(strip).raw().toBuffer({ resolveWithObject: true });
    let tail = 0;
    for (let i = 0; i < data.length; i += info.channels) if (data[i] > 180 && data[i + 1] > 100 && data[i + 2] < 120) tail++;
    check("the g in the heading has its tail", tail > 0, tail);
    await page.fill("#search", "tetris");
    const found = await page.$$eval(".card:not([hidden])", e => e.map(x => x.dataset.name));
    check("a search finds stack by what it is like", found.join() === "stack", found);
    await page.fill("#search", "");
    await page.click(".chip[data-kind=Thinking]");
    const thinking = await page.$$eval(".card:not([hidden])", e => e.map(x => x.dataset.name));
    check("a kind shows its games", thinking.join() === "wargame,stack,again", thinking);

    // A big saved value grows the game's memory as it is handed over.
    // funkey.js once looked at the memory first, and the page broke.
    await page.evaluate(() => localStorage.setItem("keep:wargame/test", "x".repeat(200000)));
    await page.goto(base + "play.html?g=wargame");
    await page.waitForTimeout(1500);
    check("a big saved value does not break the page", errors.length === 0, errors);

    // The banner over a game goes with the click, even in a game that
    // waits for a key and draws nothing new: count its gold in the band.
    const gold = () => page.evaluate(() => {
      const c = document.getElementById("screen");
      const d = c.getContext("2d").getImageData(0, Math.round(c.height * 0.42), c.width, Math.round(c.height * 0.16)).data;
      let n = 0;
      for (let i = 0; i < d.length; i += 4) if (d[i] === 255 && d[i + 1] === 208 && d[i + 2] === 64) n++;
      return n;
    });
    const before = await gold();
    await page.click("textarea[inputmode=none]");
    await page.waitForTimeout(300);
    const after = await gold();
    check("the banner goes with the click", before > 0 && after === 0, [before, after]);

    // Start a war and leave it: Q saves it, and the page keeps it.
    for (const k of ["Enter", "q"]) { await page.keyboard.press(k); await page.waitForTimeout(400); }
    const war = await page.evaluate(() => localStorage.getItem("keep:wargame/war") || "");
    check("the page keeps the saved war", war.startsWith("MODE 0 1 0"), war.slice(0, 20));

    await page.fill("#who", "Alice");
    await page.fill("#text", "Fun!\n<b>bold?</b>");
    await page.click("#send");
    await page.waitForSelector(".talk li");
    const said = await page.$$eval(".talk li p", e => e.map(x => [x.textContent, x.children.length]));
    check("a comment shows at once, as text", said.length === 1 && said[0][0] === "Fun!\n<b>bold?</b>" && said[0][1] === 0, said);
    await page.reload();
    await page.waitForSelector(".talk li");
    check("it is there for the next visitor", (await page.$$(".talk li")).length === 1);
    check("no errors", errors.length === 0, errors);
  } catch (e) {
    check("the test ran to the end", false, String(e));
  }
  await browser.close();
  server.close();
  fs.rmSync(lists, { recursive: true, force: true });
  console.log(fail ? "SOME FAILED" : "ALL OK");
  process.exit(fail);
});
