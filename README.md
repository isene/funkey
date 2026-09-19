# funkey

<img src="img/funkey.svg" align="right" width="150">

**A game engine for the terminal. Written in Rust.**

![Rust](https://img.shields.io/badge/language-Rust-orange) ![Unlicense](https://img.shields.io/badge/license-Unlicense-green) ![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-blue) ![Stay Amazing](https://img.shields.io/badge/Stay-Amazing-important)

A game draws pixels into a frame and reads keys. funkey runs the loop at a fixed rate and shows the frame in the terminal. That is half blocks, or real pixels where the terminal has the kitty graphics protocol. Every cell holds two pixels in full colour, so a 160 by 50 terminal is a 160 by 100 pixel screen. Only the cells that changed are sent. Where a terminal can show real pixels, a later backend will. Part of the [Fe₂O₃ Rust terminal suite](https://github.com/isene/fe2o3).

The first game on it is `climb`, a Jumpman-style platformer:

```bash
cargo run --release --example climb
```

Arrows or WASD move, Space jumps, Up and Down climb a ladder, R restarts, Q quits.

## What the engine gives a game

- **Frame**: a pixel framebuffer with rectangles, lines, sprites and a built-in 3 by 5 font.
- **Sprite**: rows of characters and a palette, so a game needs no image files. Transparent pixels, flipping.
- **Input**: keys with a held state. Where the terminal reports key releases (glass, kitty), a key is held from its press to its release; elsewhere it counts as held while its repeats keep coming. `pressed` for the tick a key went down, `axis_x` and `axis_y` for movement.
- **Tilemap and Body**: levels as lines of text, solid and one-way tiles. Boxes fall, run and stop at walls, one axis at a time.
- **run**: a fixed-step loop at the frame rate you ask for. The frame is scaled to the terminal by whole numbers and centred.

A game is a type with two methods:

```rust
use funkey::*;

struct Ball { x: f32, y: f32, vx: f32, vy: f32 }

impl Game for Ball {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) { return Flow::Quit; }
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        if self.x < 0.0 || self.x > 156.0 { self.vx = -self.vx; }
        if self.y < 0.0 || self.y > 96.0 { self.vy = -self.vy; }
        Flow::Continue
    }
    fn draw(&mut self, f: &mut Frame) {
        f.clear(0x102030);
        f.rect(self.x as i32, self.y as i32, 4, 4, 0xffcc00);
    }
}

fn main() {
    run(&mut Ball { x: 10.0, y: 10.0, vx: 60.0, vy: 40.0 }, Config { width: 160, height: 100, fps: 60 });
}
```

## Doom

The second renderer draws Doom levels along the level's own BSP tree, front to back:
- sectors with a floor and a ceiling, textured walls between them,
- flats read with one perspective division per pixel,
- things as sprites against a depth buffer,
- light from the sector and the distance.
 A frame of Freedoom's first map takes about a millisecond at 320 by 240.

```bash
cargo run --release --example doom            # ~/.funkey/freedoom1.wad, its first map
cargo run --release --example doom -- ~/.funkey/freedoom2.wad MAP03
```

Arrows turn and walk, A and D sidestep, Space opens a door, Q quits. Freedoom is free: [freedoom.github.io](https://freedoom.github.io/), drop the WADs in `~/.funkey/`. Doom, Heretic and Hexen WADs load the same way; monsters, weapons and the rest of the game are still to come.

## Where it is going

- Monsters, weapons, pickups and the exit switch on the Doom renderer.
- A software 3D rasterizer for driving games.
- Sound.

## Using it

```toml
[dependencies]
funkey = { version = "0.1", package = "fe2o3-funkey" }
```

Set `FUNKEY_PIXELS=kitty` to draw real pixels through the kitty graphics protocol instead of half blocks, in a terminal that has it. Set `FUNKEY_SHOT=/tmp/shot.ppm` to have the engine write the frame to a file twice a second, for screenshots and tests.

## License

Public domain (Unlicense).
