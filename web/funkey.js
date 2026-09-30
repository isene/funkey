// funkey.js: plays a funkey game built for the web (see lib.rs) in a canvas.
//
//   funkey.play(canvas, "eliminator.wasm")
//
// Keys go to the game while the canvas has focus. A click or a key gives
// it focus and starts the sound: a browser plays nothing before that.
(function () {
  "use strict";
  // The keys with names, as src/web.rs numbers them.
  const CODES = { ArrowUp: 1, ArrowDown: 2, ArrowLeft: 3, ArrowRight: 4, Enter: 5, Escape: 6, Tab: 7, Backspace: 8 };

  async function play(canvas, url) {
    const bytes = await (await fetch(url)).arrayBuffer();
    const fk = (await WebAssembly.instantiate(bytes, {})).instance.exports;
    fk.fk_start((Math.random() * 4294967296) >>> 0);
    const w = fk.fk_width(), h = fk.fk_height(), rate = fk.fk_rate();

    // The game's own pixels, then drawn a whole number of times larger,
    // so every pixel stays square at any size.
    const off = document.createElement("canvas");
    off.width = w; off.height = h;
    const octx = off.getContext("2d");
    const image = octx.createImageData(w, h);
    const ctx = canvas.getContext("2d");
    function fit() {
      const k = Math.max(1, Math.ceil(canvas.clientWidth * (window.devicePixelRatio || 1) / w));
      if (canvas.width !== w * k) { canvas.width = w * k; canvas.height = h * k; }
      ctx.imageSmoothingEnabled = false;
    }

    // Sound: the game's mix, a little ahead of the clock.
    let audio = null, next = 0;
    function wake() {
      if (!audio) {
        const AC = window.AudioContext || window.webkitAudioContext;
        try { audio = new AC({ sampleRate: rate }); } catch (e) { audio = new AC(); }
      }
      if (audio.state !== "running") audio.resume();
    }
    function pump() {
      if (!audio || audio.state !== "running") return;
      if (next < audio.currentTime) next = audio.currentTime + 0.03;
      const n = Math.floor((audio.currentTime + 0.15 - next) * rate);
      if (n < 256) return;
      const pcm = new Float32Array(fk.memory.buffer, fk.fk_sound(n), n);
      const buf = audio.createBuffer(1, n, rate);
      buf.getChannelData(0).set(pcm);
      const src = audio.createBufferSource();
      src.buffer = buf;
      src.connect(audio.destination);
      src.start(next);
      next += n / rate;
    }

    // Keys. A key lets go with the code it went down with, whatever
    // Shift did in between.
    const held = new Map();
    canvas.tabIndex = 0;
    canvas.addEventListener("pointerdown", () => { canvas.focus(); wake(); });
    canvas.addEventListener("keydown", e => {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      const c = CODES[e.key] || (e.key.length === 1 ? e.key.toLowerCase().codePointAt(0) : 0);
      if (!c) return;
      e.preventDefault();
      wake();
      if (e.repeat || held.has(e.code)) return;
      held.set(e.code, c);
      fk.fk_key(c, 1);
    });
    canvas.addEventListener("keyup", e => {
      const c = held.get(e.code);
      if (c === undefined) return;
      held.delete(e.code);
      fk.fk_key(c, 0);
    });
    canvas.addEventListener("blur", () => { for (const c of held.values()) fk.fk_key(c, 0); held.clear(); });

    function tick(ms) {
      if (fk.fk_frame(ms)) {
        fit();
        image.data.set(new Uint8ClampedArray(fk.memory.buffer, fk.fk_pixels(), w * h * 4));
        octx.putImageData(image, 0, 0);
        ctx.drawImage(off, 0, 0, canvas.width, canvas.height);
        if (document.activeElement !== canvas) {
          ctx.fillStyle = "rgba(0,0,0,0.6)";
          ctx.fillRect(0, canvas.height * 0.42, canvas.width, canvas.height * 0.16);
          ctx.fillStyle = "#ffd040";
          ctx.font = "bold " + Math.round(canvas.height / 16) + "px monospace";
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.fillText("CLICK TO PLAY", canvas.width / 2, canvas.height / 2);
        }
      }
      pump();
      requestAnimationFrame(tick);
    }
    requestAnimationFrame(tick);
  }

  window.funkey = { play };
})();
