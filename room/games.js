// The games in the room, in the order the lobby shows them. Both pages
// read this table; a name from the address only picks a row of it.
//
// kinds: the lobby's filters. line: the card's one line. about: the play
// page's paragraph. size: the game's picture. v: the cache tag, the
// game's version and the engine's. help: the keys, as HTML written here.
// move and keys: the buttons on the screen for phones. Every game has a
// shared top ten.
(function () {
  "use strict";
  const K = (k, label, small) => [k, label, small || ""];
  const gap = K("", "");
  const ARROWS = [gap, K("ArrowUp", "↑"), gap, K("ArrowLeft", "←"), gap, K("ArrowRight", "→"), gap, K("ArrowDown", "↓"), gap];
  const QUIT = K("q", "Q", "quit");
  const ENGINE = "0.9";

  window.ROOM = {
    KINDS: ["Arcade", "Shooting", "Thinking", "Adventure", "Racing"],
    GAMES: {
      wargame: {
        kinds: ["Thinking"], v: "0.3", size: [960, 600], isnew: true,
        line: "A hex wargame: North Africa or Stalingrad, 1942.",
        about: "A board wargame in the spirit of the 1980s classics, one side against the other on a hex map. " +
          "Rommel's drive for the Nile, or the drive for the Volga and the Soviet answer in the snow. " +
          "Supply, zones of control, odds and a die. Play either side against the computer, or two players at one keyboard.",
        alt: "wargame: the desert map of North Africa, German, Italian and British counters around Tobruk",
        help: "Arrows (or <kbd>h</kbd> <kbd>j</kbd> <kbd>k</kbd> <kbd>l</kbd>) move the cursor. " +
          "<kbd>Enter</kbd> picks a unit and moves it, <kbd>Tab</kbd> jumps to the next unit that can still move or attack, <kbd>U</kbd> takes a move back. " +
          "To attack, <kbd>Enter</kbd> on an enemy marks it and lights your units that can join; " +
          "<kbd>Enter</kbd> on one of yours puts it in or out, <kbd>Enter</kbd> on the enemy again rolls the die. " +
          "<kbd>N</kbd> ends the phase, <kbd>?</kbd> shows the rules. <kbd>Q</kbd> saves the war: this browser keeps it for your next visit. " +
          "The computer's units have one more attack and defence than a person's would. " +
          "A war against it ends with a score: the result counts most, then the steps each side lost.",
        move: ARROWS,
        keys: [K("Enter", "Enter", "pick"), K("Tab", "Tab", "next"), K("u", "U", "undo"), K("n", "N", "phase"),
          K("Escape", "Esc", "back"), K("?", "?", "rules"), QUIT],
      },
      raid: {
        kinds: ["Shooting"], v: "1.2", size: [640, 400],
        line: "A gunship over fractal mountains, six missions.",
        about: "Six missions in the spirit of Comanche, from a radar post at dawn to a fortress at night. " +
          "A gun, rockets and guided missiles against trucks, tanks, flak, gunboats, missile sites and other gunships.",
        alt: "raid: a gunship shot down over a lake, a second one coming in, a flak gun in the target box",
        help: "<kbd>←</kbd> <kbd>→</kbd> turn, <kbd>↑</kbd> <kbd>↓</kbd> set the speed, <kbd>W</kbd> <kbd>S</kbd> the height over the ground, " +
          "<kbd>A</kbd> <kbd>D</kbd> slide sideways. <kbd>Space</kbd> fires the gun, <kbd>F</kbd> a rocket, " +
          "<kbd>E</kbd> a guided missile at the boxed target. <kbd>P</kbd> pauses. " +
          "Hills are cover: what cannot see you cannot shoot you. Hover low over the pad to rearm and repair.",
        move: ARROWS,
        keys: [K("w", "W", "up"), K("s", "S", "down"), K("a", "A", "left"), K("d", "D", "right"), K(" ", "Space", "gun"),
          K("f", "F", "rocket"), K("e", "E", "missile"), K("Enter", "Enter", "start"), K("p", "P", "pause"), QUIT],
        wide: true,
      },
      kart: {
        kinds: ["Racing"], v: "1.3", size: [640, 400],
        line: "Eight karts, four tracks, drifts and item boxes.",
        about: "Eight karts on four tracks: a meadow, a beach, an icy pass and a desert. " +
          "Hop into a bend and hold the drift until the sparks turn blue or orange, then let go for a push. " +
          "A cup is four races, at 50, 100 or 150cc. It scores a thousand for each point and a little for speed, " +
          "times two at 100cc and three at 150cc.",
        alt: "kart: a red and a cyan kart side by side on a grey road, with more karts ahead and trees behind the barrier",
        help: "<kbd>↑</kbd> is the gas, <kbd>↓</kbd> the brake, <kbd>←</kbd> <kbd>→</kbd> steer. " +
          "<kbd>Space</kbd> hops, and held through a bend it drifts: let go when the sparks turn blue or orange for a push. " +
          "<kbd>X</kbd> uses the item from a box, <kbd>P</kbd> pauses. On the title, <kbd>←</kbd> <kbd>→</kbd> pick the class.",
        move: ARROWS,
        keys: [K(" ", "Space", "hop"), K("x", "X", "item"), K("p", "P", "pause"), QUIT],
      },
      salvo: {
        kinds: ["Shooting", "Arcade"], v: "1.3", size: [480, 270],
        line: "A side-scrolling shooter, seven stages and their bosses.",
        about: "Seven stages in the spirit of Gradius, a boss at the end of each: a volcano, stone heads, crystals, living cells, " +
          "the sun, a high-speed maze and the fortress. Capsules light the power-up bar one step at a time.",
        alt: "salvo: the ship flies past a volcano throwing rocks, a row of fans coming at it",
        help: "Arrows fly, <kbd>Space</kbd> fires (hold it), <kbd>Z</kbd> takes the lit power-up, <kbd>P</kbd> pauses.",
        move: ARROWS,
        keys: [K(" ", "Space", "fire"), K("z", "Z", "power"), K("p", "P", "pause"), K("Enter", "Enter", "start"), QUIT],
      },
      stack: {
        kinds: ["Arcade", "Thinking"], v: "1.1", size: [480, 270],
        line: "Falling blocks, a well ten wide, Korobeiniki playing.",
        about: "Seven pieces of four blocks in the spirit of Tetris. The pieces turn and kick off the walls, a bag of seven deals them. " +
          "Hold one, see the next five, drop soft or hard. Four rows at once and a T spun into its slot score more.",
        alt: "stack: the game playing itself, a combo of two, the next five pieces on the right",
        help: "<kbd>←</kbd> <kbd>→</kbd> move, <kbd>↑</kbd> turns, <kbd>Z</kbd> turns back, <kbd>↓</kbd> drops soft, " +
          "<kbd>Space</kbd> drops hard, <kbd>C</kbd> holds a piece, <kbd>P</kbd> pauses.",
        move: [K("z", "↺", "left"), K("ArrowUp", "↻", "right"), K("c", "C", "hold"),
          K("ArrowLeft", "←"), K("ArrowDown", "↓", "soft"), K("ArrowRight", "→")],
        keys: [K(" ", "Space", "drop"), K("Enter", "Enter", "start"), K("p", "P", "pause"), QUIT],
      },
      again: {
        kinds: ["Thinking"], v: "1.2", size: [480, 270],
        line: "A time-loop puzzle you solve with your own past.",
        about: "A life is a handful of steps in a small room. Then time starts over, and the self you just were walks beside you, " +
          "pressing the same keys. One of you stands on a plate while the next walks through the door it opens. Twenty rooms.",
        alt: "again: three selves in a room of doors and gates, and under it their steps as rows of arrows",
        help: "Arrows step, <kbd>Space</kbd> waits a step. <kbd>Enter</kbd> lets the rest of the life run out: " +
          "then time starts over, and the self you just were walks beside you. " +
          "<kbd>U</kbd> takes back a step, <kbd>Backspace</kbd> the life, <kbd>R</kbd> the room. <kbd>N</kbd> and <kbd>P</kbd> change rooms. " +
          "This browser keeps the rooms you have solved. A room solved the first time scores 100, " +
          "10 more for each step its last life had left and 5 less for each take-back. The score goes to the top ten when you leave for the title.",
        move: ARROWS,
        keys: [K(" ", "Space", "wait"), K("Enter", "Enter", "run on"), K("u", "U", "undo"), K("Backspace", "⌫", "life"),
          K("r", "R", "room"), K("n", "N", "next"), K("p", "P", "back"), QUIT],
      },
      marble: {
        kinds: ["Arcade"], v: "1.0", size: [640, 400],
        line: "A glass ball down six courses that hang in the dark.",
        about: "A tribute to Marble Madness: six sloping courses against the clock. The ball gathers speed downhill, " +
          "flies off a ramp and breaks after a long fall. Steel balls shove it, green springs eat it, acid melts it.",
        alt: "marble: a blue marble on a patch of ice, a pool of acid on the road behind it and a green spring waiting ahead",
        help: "The arrows roll the marble along the tiles: <kbd>→</kbd> is down to the right, <kbd>↓</kbd> is down to the left. " +
          "Two at once roll it straight. <kbd>Space</kbd> starts, <kbd>P</kbd> pauses. " +
          "The buttons under the game roll it the way their arrows point.",
        move: [K("ArrowLeft", "↖"), K("ArrowLeft|ArrowUp", "↑"), K("ArrowUp", "↗"), K("ArrowLeft|ArrowDown", "←"), gap,
          K("ArrowRight|ArrowUp", "→"), K("ArrowDown", "↙"), K("ArrowRight|ArrowDown", "↓"), K("ArrowRight", "↘")],
        keys: [K(" ", "Space", "start"), K("p", "P", "pause"), QUIT],
      },
      chomp: {
        kinds: ["Arcade"], v: "1.0", size: [224, 288],
        line: "Eat the dots while four chasers hunt you.",
        about: "In the spirit of Pac-Man: four chasers hunt you by the arcade's rules. One comes straight for you, one aims ahead, " +
          "one closes in from the other side, and one loses heart up close. The maze, the art and the sound are new.",
        alt: "chomp: a yellow muncher in a blue maze of dots, three chasers out in the corridors",
        help: "The arrows steer: a turn asked for early is taken at the next opening. <kbd>Space</kbd> starts, <kbd>P</kbd> pauses. " +
          "A power dot turns the hunt around for a few seconds.",
        move: ARROWS,
        keys: [K(" ", "Space", "start"), K("p", "P", "pause"), QUIT],
      },
      eliminator: {
        kinds: ["Adventure"], v: "1.4", size: [640, 360],
        line: "A dungeon by the Amar RPG rules, every roll shown.",
        about: "The maze beneath the royal castle of Amar: five levels up to the gate, and the Raven Demon guarding it. " +
          "Stances, double attacks, wounds, light, fear, and marks that raise your skills.",
        alt: "eliminator: a sellsword fights an Araxi by torchlight, the dice of every roll shown below the map",
        help: "<kbd>i</kbd> on the title shows how Amar's dice and fights work, in three pages. " +
          "Arrows or <kbd>y</kbd> <kbd>u</kbd> <kbd>b</kbd> <kbd>n</kbd> move, walk into a foe to fight. " +
          "<kbd>1</kbd> to <kbd>6</kbd> pick how you strike, <kbd>?</kbd> shows all the keys.",
        move: [K("y", "↖"), K("ArrowUp", "↑"), K("u", "↗"), K("ArrowLeft", "←"), K("s", "wait"),
          K("ArrowRight", "→"), K("b", "↙"), K("ArrowDown", "↓"), K("n", "↘")],
        keys: [K("1", "1", "normal"), K("2", "2", "offence"), K("3", "3", "defence"), K("4", "4", "guard"),
          K("5", "5", "power"), K("6", "6", "double"), K("f", "F", "fire"), K("p", "P", "potion"),
          K("m", "M", "bandage"), K("r", "R", "rest"), K("t", "T", "light"), K("g", "G", "take"),
          K("<", "<", "climb"), K("?", "?", "rules"), K("i", "I", "intro"), QUIT,
          K("Enter", "Enter"), K(" ", "Space"), K("Escape", "Esc"), K("Tab", "Tab")],
        wide: true,
      },
      vector: {
        kinds: ["Shooting", "Arcade"], v: "1.0", size: [640, 400],
        line: "Glowing lines on black: sixteen webs and what climbs them.",
        about: "A tribute to Tempest. Sixteen webs seen down their length, a claw on the rim, and what climbs the lanes toward it: " +
          "flippers, tankers, spikers, fuseballs and pulsars. The lines add their light where they cross, as on a vector tube.",
        alt: "vector: a heart-shaped web, a pulsar lighting its lane white, the claw moving off along the rim",
        help: "<kbd>←</kbd> <kbd>→</kbd> move the claw along the rim, <kbd>Space</kbd> fires down the lane (hold it). " +
          "<kbd>Z</kbd> is the superzapper: once a web it clears the web, a second time it kills one. <kbd>P</kbd> pauses. " +
          "On the title, <kbd>←</kbd> <kbd>→</kbd> pick the web to start at.",
        move: [gap, gap, gap, K("ArrowLeft", "←"), gap, K("ArrowRight", "→")],
        keys: [K(" ", "Space", "fire"), K("z", "Z", "zap"), K("p", "P", "pause"), QUIT],
      },
      blocks: {
        kinds: ["Adventure"], v: "0.4", size: [640, 400],
        line: "A world of blocks to dig into and build on.",
        about: "In the spirit of Minecraft: a world grown from a seed, with hills, a sea, tunnels, groves and one great mountain. " +
          "Dig into it and build on it, make planks, glass, brick and a pickaxe. Cats come to see who is there.",
        alt: "blocks: two cats on the grass above a blue bay, with a wooded hill and clouds behind them",
        help: "<kbd>W</kbd> and <kbd>S</kbd> walk, <kbd>A</kbd> and <kbd>D</kbd> sidestep, the arrows look round, <kbd>Space</kbd> jumps and swims. " +
          "<kbd>F</kbd> digs, <kbd>E</kbd> builds, <kbd>1</kbd> to <kbd>9</kbd> and <kbd>0</kbd> pick the kind, <kbd>C</kbd> lists the things to make. " +
          "<kbd>P</kbd> pauses. This browser keeps your world; <kbd>N</kbd> on the title grows a new one. " +
          "Each visit scores a point for every block dug or built and ten for each thing made. <kbd>Q</kbd> leaves for the title with the score.",
        move: ARROWS,
        keys: [K("w", "W", "walk"), K("s", "S", "back"), K("a", "A", "left"), K("d", "D", "right"), K(" ", "Space", "jump"),
          K("f", "F", "dig"), K("e", "E", "build"), K("c", "C", "make"), K("Enter", "Enter"),
          K("1", "1"), K("2", "2"), K("3", "3"), K("4", "4"), K("5", "5"), K("6", "6"), K("7", "7"), K("8", "8"), K("9", "9"), K("0", "0"),
          K("p", "P", "pause"), QUIT],
        wide: true,
      },
      jumpman: {
        kinds: ["Arcade"], v: "1.1", size: [256, 192],
        line: "Girders, ladders, ropes and bombs, five levels.",
        about: "A platformer in the spirit of Jumpman Junior. Take every bomb, jump the bullets, dodge the robots, " +
          "and do not fall too far. One bomb reveals hidden ladders, one takes girders away.",
        alt: "jumpman: girders, ladders and bombs",
        help: "Arrows run and climb, <kbd>Space</kbd> jumps, <kbd>R</kbd> gives up a life when you are stuck.",
        move: ARROWS,
        keys: [K(" ", "Space", "jump"), K("Enter", "Enter", "start"), K("r", "R", "stuck"), QUIT],
      },
    },
    file: name => "wasm/" + name + ".wasm?v=" + window.ROOM.GAMES[name].v + "-" + ENGINE,
    game: name => Object.prototype.hasOwnProperty.call(window.ROOM.GAMES, name) ? window.ROOM.GAMES[name] : null,
    SCORES: "/cgi-bin/funkey-scores.rb?game=",
    COMMENTS: "/cgi-bin/funkey-comments.rb?game=",
    // localStorage, which a browser may refuse: then nothing is kept.
    store: (k, v) => {
      try { return v === undefined ? localStorage.getItem(k) : localStorage.setItem(k, v); } catch (e) { return null; }
    },
    // "3 hours ago", or the date when it is more than a week back.
    when: t => {
      const s = Math.max(0, Date.now() / 1000 - t);
      if (s < 90) return "just now";
      if (s < 5400) return Math.round(s / 60) + " minutes ago";
      if (s < 129600) return Math.round(s / 3600) + " hours ago";
      if (s < 604800) return Math.round(s / 86400) + " days ago";
      return new Date(t * 1000).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
    },
  };
})();
