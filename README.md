# Netrek for the terminal

A graphical client and game server for **Netrek**, the 1988 multiplayer space battle game,
that runs entirely inside a terminal window on macOS.

![A game in vector mode: the tactical view with terrain (asteroid fields, a slipstream, a comet) on the left, the galactic map in the middle, the player list with ranks on the right, and below them the ship controls with a relic ship and its advanced tech, and the message panel with orders and supplies](screeNew1.png)

- **Classic layout:** tactical and galactic maps side by side (square, or taller on a tall
  window), with the
  dashboard, player list and message window underneath, like the original X11 client.
- **Real graphics in the terminal:** vector-style maps and control panel drawn with
  [tiny-skia](https://github.com/linebender/tiny-skia) and sent as SIXEL images. On
  terminals without image support it falls back to braille line art.
- **Authentic rules:** the 40-planet galaxy, the six ship classes, and weapons, fuel,
  heat, orbiting, bombing, army carrying and planet capture use numbers from the
  Vanilla Netrek server source.
- **Trek-style ships:** each empire has its own ship designs (Federation saucers and
  nacelles, Klingon D7s and Birds-of-Prey, Romulan warbirds, Orion raiders).
- **Robots:** AI pilots fight, bomb, carry armies and capture planets, so you can play solo.
- **Alien incursions** (optional): thirty-two threats, from Khan, the Borg and the
  planet killer to V'Ger, Species 8472, the Jem'Hadar, General Chang, Q, Nomad, the
  Metrons' arena, a Dyson sphere and a plague of tribbles, drop into the game, and the
  Kzinti arrive with a Ringworld that stays until they lose it.
- **Extras** (optional, each its own server option): career ranks and service records,
  personal orders from command, treaties between empires, 21 kinds of space terrain (10
  in any one galaxy), supply convoys that buy upgrades for your empire, subsystem damage,
  boarding parties that capture enemy ships, and outposts (defences, shipyards and
  sensor arrays) built on your planets.
- **Observers:** watch any game without flying, with the whole galaxy revealed, following
  any ship.
- **Sound:** synthesized retro sound effects, with no audio libraries required.
- **Mouse and keyboard:** aim and steer with the mouse, with the classic Netrek key bindings.

---

## Contents

- [Requirements](#requirements)
- [Build and install](#build-and-install)
- [Quick start](#quick-start)
- [Command-line reference](#command-line-reference)
  - [Observers](#observers)
- [Terminal setup and graphics modes](#terminal-setup-and-graphics-modes)
- [The screen](#the-screen)
- [Controls](#controls)
- [How to play](#how-to-play)
- [Ships](#ships)
- [Planets](#planets)
  - [Taking and retaking planets](#taking-and-retaking-planets)
- [Robots](#robots)
- [Alien incursions](#alien-incursions)
- [Extras](#extras)
  - [Ranks and service records](#ranks-and-service-records)
  - [Advanced tech](#advanced-tech)
  - [Special and relic ships](#special-and-relic-ships)
  - [Orders](#orders)
  - [Diplomacy](#diplomacy)
  - [Space terrain](#space-terrain)
  - [Supply convoys and upgrades](#supply-convoys-and-upgrades)
  - [Subsystem damage](#subsystem-damage)
  - [Boarding parties](#boarding-parties)
  - [Outposts](#outposts)
- [Sound](#sound)
- [Hosting a server](#hosting-a-server)
- [Troubleshooting](#troubleshooting)
- [Differences from classic Netrek](#differences-from-classic-netrek)
- [Project layout](#project-layout)
- [Development](#development)
- [Credits](#credits)
- [License](#license)

---

## Requirements

- **macOS** (the primary target). Linux works too; see the notes under
  [Sound](#sound).
- **Rust**, a recent stable toolchain: `brew install rust` or [rustup](https://rustup.rs).
- A terminal of at least **80×24**. **160×50 or larger** gives the classic proportions.
- Optional, for vector graphics: a terminal with SIXEL support (iTerm2, WezTerm, foot),
  or tmux 3.4+ in front of one. See [Terminal setup](#terminal-setup-and-graphics-modes).

## Build and install

```sh
git clone https://github.com/MWChapel/netrek-terminal.git && cd netrek-terminal
cargo build --release
./target/release/netrek --help
```

To put `netrek` on your `PATH`:

```sh
cargo install --path .
```

## Quick start

```sh
# Play right away: a private local server with 7 robots, you on the Federation
netrek solo

# Pick your side and ship
netrek solo --team rom --ship DD --name Tomalak
```

On the **outfit screen**, choose an empire with `f` `r` `k` `o` and a ship with
`s` `d` `c` `b` `a` `x`, then press **Enter** to launch. Move the mouse over the
tactical map to aim. Left-click fires torpedoes, right-click steers, and `0`–`9` set
your speed. Press `?` at any time for the full key list.

For a four-way war with robots flying for every empire:

```sh
netrek solo --empires all --bots 15
```

Add Star Trek villains to the mix:

```sh
netrek solo --empires all --bots 15 --aliens
```

Switch on every extra (ranks, orders, diplomacy, terrain, supplies, subsystem damage,
boarding parties and outposts):

```sh
netrek solo --empires all --bots 12 --aliens --extras
```

To play with friends, one person runs a server and everyone else connects:

```sh
netrek server --bots 4            # on the host machine
netrek play host.example.com      # everyone else
netrek observe host.example.com   # or just watch
```

## Command-line reference

```
netrek <COMMAND>

  server   Run a game server
  solo     Start a private local server with robots and play on it
  play     Connect to a server and play
  observe  Connect to a server and watch as an observer
```

### `netrek server`

| Option | Default | Meaning |
|---|---|---|
| `-p, --port <PORT>` | `2592` | TCP port to listen on (2592 is the traditional Netrek port) |
| `-b, --bind <ADDR>` | `0.0.0.0` | Address to bind; use `127.0.0.1` for local-only |
| `--bots <N>` | `6` | Robot players kept in the game |
| `-e, --empires <LIST>` | `fed,rom` | Empires the robots play for: `all`, or a comma list such as `fed,rom,kli` |
| `--aliens [LIST]` | off | Alien incursions: bare `--aliens` for all thirty-two, or a list such as `khan,borg,vger`. See [Alien incursions](#alien-incursions) |
| `--alien-interval <SECS>` | `150` | Average seconds between incursions |
| `--ranks [FILE]` | off | Career ranks, service records and a leaderboard, saved to `FILE` (default `~/.netrek/service-records.tsv`). See [Extras](#extras) |
| `--orders` | off | Personal orders from command, with rewards |
| `--diplomacy` | off | Treaties between empires |
| `--terrain` | off | Space terrain: 10 of 21 kinds per galaxy (nebulae, a black hole, minefields, a galactic barrier...) |
| `--supply` | off | Supply convoys and empire upgrades |
| `--subsystems` | off | [Subsystem damage](#subsystem-damage): hits can knock out warp, phasers, shields and more |
| `--boarding` | off | [Boarding parties](#boarding-parties): capture enemy ships with the armies you carry |
| `--outposts` | off | [Outposts](#outposts): build defence outposts, shipyards and sensor arrays on your planets |
| `--extras` | off | All eight of the above |
| `--no-rank-tech` | off | With ranks: don't give Captains and above their [advanced tech](#advanced-tech) |
| `--no-observers` | off | Don't let anyone [watch as an observer](#observers) |

The server logs connections, joins, kills and planet captures to stdout.

### `netrek solo`

Starts a server on a random localhost port in the background and connects to it.

| Option | Default | Meaning |
|---|---|---|
| `--bots <N>` | `7` | Number of robots |
| `-e, --empires <LIST>` | `fed,rom` | Empires the robots play for: `all`, or a comma list |
| `--aliens [LIST]` | off | Alien incursions (all, or a list such as `khan,borg`) |
| `--alien-interval <SECS>` | `150` | Average seconds between incursions |
| `--ranks [FILE]`, `--orders`, `--diplomacy`, `--terrain`, `--supply`, `--subsystems`, `--boarding`, `--outposts`, `--extras` | off | The same [extras](#extras) as `server` |
| `-n, --name <NAME>` | `$USER` | Your callsign |
| `-t, --team <TEAM>` | `fed` | Preferred team: `fed`, `rom`, `kli`, `ori` |
| `-s, --ship <SHIP>` | `CA` | Preferred ship: `SC`, `DD`, `CA`, `BB`, `AS`, `SB` |
| `-g, --gfx <MODE>` | `auto` | Map graphics: `auto`, `vector` (alias `sixel`), `braille` |
| `--mute` | off | Start with sound effects off |

### `netrek play [HOST]`

| Option | Default | Meaning |
|---|---|---|
| `HOST` | `localhost` | Server host name or address |
| `-p, --port <PORT>` | `2592` | Server port |
| `-n`, `-t`, `-s`, `-g`, `--mute` | | Same as `solo` |

The team and ship options only preselect choices on the outfit screen. You still press
Enter to launch.

### `netrek observe [HOST]`

Watch a game without flying: see [Observers](#observers).

| Option | Default | Meaning |
|---|---|---|
| `HOST` | `localhost` | Server host name or address |
| `-p, --port <PORT>` | `2592` | Server port |
| `-n`, `-g`, `--mute` | | Your name, graphics and sound, as for `solo` |

### Observers

Observers watch the game without a ship. Start with `netrek observe HOST`, or press **W**
on the outfit screen to switch from playing to watching (your player slot is freed).
Press **J** at any time to take a slot and join the game.

- **Everything is visible.** Every planet is shown scouted, with its armies. Cloaked ships,
  ships hidden in nebulae, Talosian illusions and Changelings all appear as they really
  are, and so do the armies aboard every ship.
- **Following a ship.** `Tab` / `Shift-Tab` step through the ships in play, or click one.
  The maps follow it, and the gauges and status line show its fuel, shields and damage.
- **Free camera.** The arrow keys pan (a click on empty space looks there), and `Esc`
  stops following. `+` / `-` or the mouse wheel zoom.
- **Talking.** `m` sends a message to everyone, shown as `Name (obs)`. Observers hear
  everything said to all, but not team or private messages, and can't use slash commands.
- **Everything else:** `L` / `P` lists, `i` info, `g` graphics, `S` sound, `?` help, `q` quit.

Observers never take a player slot, so they don't crowd out robots or aliens. Up to 8 can
watch at once, and the player list shows who's watching. Servers allow observers unless
started with `--no-observers`.

## Terminal setup and graphics modes

The client picks the best graphics your terminal supports. Press `g` in game to switch
between the modes, or force one with `--gfx`.

| Mode | Looks like | Works in |
|---|---|---|
| **vector** | Real pixel images: thin anti-aliased lines, outlined planets, crisp labels, graphical dashboard | iTerm2, WezTerm, foot, mlterm; tmux 3.4+ when configured (below) |
| **braille** | Braille-dot line art | Any Unicode terminal |

In both modes the layout is sized in real screen pixels, so the maps stay square
whatever your font's cell shape. In vector mode the help, player and planet popups are
drawn as images over the maps, so the maps stay in vector mode while a popup is open.

### iTerm2

Vector mode works out of the box. Make sure **Settings → Profiles → Terminal → Enable
mouse reporting** is on (the default) for mouse aiming.

### tmux

tmux 3.4 and newer can pass SIXEL images through, but only after you tell it the outer
terminal supports them. Add this to `~/.tmux.conf`:

```
set -as terminal-features ',xterm*:sixel'
```

Then reload the config and **detach and reattach**, because terminal features are read
when a client attaches:

```sh
tmux source ~/.tmux.conf
# prefix + d, then: tmux attach
```

If you run inside tmux without this setting, the client falls back to braille mode and
shows a one-line hint. The outer terminal must itself support SIXEL. Inside tmux, the
client checks which app is hosting your tmux client (by walking its process tree). If
that's Terminal.app (or another terminal without SIXEL), it stays in braille mode and
suggests attaching from iTerm2. The same tmux session can be attached from Terminal.app
and from iTerm2 at different times, and the client picks the right mode each time it
starts.

### Terminal.app

Terminal.app can't display images, so it uses **braille** line art. For the full vector
look, run in iTerm2 instead.

### Colors and fonts

- **Color:** 24-bit color is used when `COLORTERM=truecolor` (or on iTerm2, WezTerm and
  Ghostty). Otherwise it falls back to the 256-color palette.
- **Fonts:** vector mode draws text into its images with the system Menlo font (falling
  back to Monaco or SF Mono). No font setup is needed.

## The screen

```
┌──────────────── tactical ───────────────┐┌──────────────── galactic ───────────────┐┌─ players ─┐
│ your neighbourhood of space, centred on ││ the whole 100,000 × 100,000 galaxy,     ││ No Ty Name│
│ your ship. Border colour = alert level  ││ planets, every visible ship, and a box  ││ F0 CA Kirk│
│ (green clear / yellow near / red close) ││ showing where the tactical view is      ││ R3 BB Kor │
└─────────────────────────────────────────┘└─────────────────────────────────────────┘│ …         │
┌────────────── ship controls ────────────┐┌──────────────── messages ───────────────┐│           │
│ status lamps · speed/shield/hull/fuel/  ││ warnings                                ││           │
│ weapon-heat/engine-heat gauges · status ││ talk line · message log                 ││           │
└─────────────────────────────────────────┘└─────────────────────────────────────────┘└───────────┘
```

- **Tactical:** about 20,000 galaxy units across at zoom 1 (the original client's scale).
  Planets are labelled with their name. Planets your team has scouted also show their
  army count and resources: **R**epair, **F**uel, **A**gricultural. Ships show their
  player slot.
- **Aliens:** ships from alien incursions are drawn in their own colours and labelled by
  name (`Khan`, `Borg`, …). Tholian webs show as glowing orange strands. See
  [Alien incursions](#alien-incursions).
- **Galactic:** unscouted planets are grey with a `?`. Planets with more than 4 armies
  are marked, and home worlds have a double ring. Cloaked enemies appear as `??` at a
  rough position.
- **Controls:** status lamps (shields, cloak, repair, orbit, bomb, beam up/down,
  tractor, pressor, lock), gauges with current/max values, and a status line with kills,
  armies carried, torpedoes out, orbit/lock target and the game clock.
- **Player list:** shows in a column beside the maps when the window is wide enough,
  otherwise under the controls. With ranks on there's always a Rank column. Names get
  the room that's left, and the robot/convoy note is dropped first when space is short. Robots are marked, and your own line is bold. With a lot
  of players the rows close up so everyone fits.
- **Panels below the maps:** the controls and message panels keep a fixed height (room
  for the talk line, orders, supplies and a handful of messages, with the tech list on the
  left) and their text stays the same size whatever the window size. The maps take all
  the remaining height: on a tall window they grow taller than they are wide (up to 1.6
  times), so the tactical view shows more of space above and below you and the galaxy map
  stretches to match. Nothing is left empty at the bottom.
- **Messages:** the panel under the galactic map runs the full width of the window. It
  holds warnings, the talk line, any orders, alliance, supply and tech lines, and the
  message log. Long lines **wrap** instead of being cut off, and messages can be up to 160
  characters (classic Netrek allowed 80). While you type a long message, the end of it
  stays in view.

## Controls

The keys follow the original Netrek client where possible. Commands that need a
direction use the **mouse pointer**, whether it's over the tactical or the galactic
map. With no mouse, they fire along your current heading.

### Flying

| Key | Action |
|---|---|
| right click, `k` | Set course toward the pointer |
| `←` `→` | Turn by 22.5° |
| `0`–`9` | Set speed 0–9 |
| `)` `!` `@` | Speed 10, 11, 12 (fast ships only) |
| `%` / `#` | Maximum / half speed |
| `↑` `↓` | Speed up / slow down by one |
| `o` | Orbit the nearby planet (you must be within range and at warp 2 or less) |
| `l` | Lock on to the planet or ship nearest the pointer. The autopilot steers there and drops into orbit at planets |

### Weapons and systems

| Key | Action |
|---|---|
| left click, `t` | Fire a photon torpedo toward the pointer (up to 8 in flight) |
| middle click, `p` | Fire a phaser toward the pointer (hits the first enemy near the beam) |
| `f` | Fire a plasma torpedo (homing; DD, CA, BB and SB only, needs 2 kills) |
| `s` or `u` | Shields up / down |
| `c` | Cloak on / off (drains fuel; you can't fire while cloaked) |
| `d` | Detonate enemy torpedoes near you (costs fuel) |
| `B` | Board the enemy ship nearest the pointer (with `--boarding`): see [Boarding parties](#boarding-parties) |
| `v` / `e` / `j` | Use your [advanced tech](#advanced-tech) (Commodore / Rear Admiral / Admiral, with `--ranks`), aimed at the pointer |
| `w` | **Overwatch** on / off, **in orbit only**: automatically fire at the nearest enemy that comes into weapons range (also `/overwatch`) |
| `D` | Detonate your own torpedoes |
| `T` | Tractor beam on the ship nearest the pointer (again to release) |
| `y` | Pressor beam on the ship nearest the pointer (again to release) |
| `R` | Repair mode: stop, drop shields, repair faster |


**Overwatch** (`w`) turns your ship into a sentry guarding a planet. It can only be
switched on **while you're orbiting a planet**, and it switches itself off the moment you
leave orbit (or your ship is destroyed). While it's on, the status line shows
**OVERWATCH** (vector mode: the orange **OVWT** lamp), and every enemy that comes into
range draws fire from you. It picks the nearest enemy you can see and reaches for **special
weapons first**:
1. Your heavy weapon (`e`), whenever it's ready: the isokinetic cannon out to 9,000, the
   antiproton burst within 4,500, or the tricobalt device, but only at a safe 4,500 to
   5,400 and never with a friendly ship near the target.
2. A starbase's fighter wing (`j`), when an enemy comes within 15,000.
3. Plasma, whenever your ship has it, you have the kills and none is already in flight.
4. Otherwise phasers when the enemy is close, and torpedoes aimed ahead of it at longer
   range, about two a second.

Against Species 8472 it only fires plasma. You still steer, and the sensor and escape
techs are left to you.

It holds fire:
- at allies, at ships you can't see (cloaked or hidden in a nebula), and at targets
  weapons can't hurt (Q, V'Ger, the whale probe)
- while you're cloaked or in repair mode
- when your fuel is below 25% or your weapons are running hot, so it never leaves you
  stranded

A starbase's fighters are the exception: they fly on overwatch wherever they go.
Overwatch works on any server (protocol version 6).
### Planets and armies

| Key | Action |
|---|---|
| `b` | Bomb the enemy planet you're orbiting (kills armies, down to 4) |
| `z` | Beam armies up from a friendly planet |
| `x` | Beam armies down onto the planet you're orbiting |
| `r` then a ship key | Refit to another ship class (while orbiting your home planet, with no armies aboard). `e` / `u` for a special or relic ship |

### Information and interface

| Key | Action |
|---|---|
| `i` | Info about the planet or ship under the pointer |
| `m` | Send a message: then `A` all, `T` your team, `F`/`R`/`K`/`O` a team, or a player slot (`0`–`9`, `a`–`v`). Type, then Enter |
| `/` | Type a server command, such as `/record`, `/orders`, `/treaty rom` or `/upgrade torps` (see [Extras](#extras)) |
| `L` | Player list |
| `P` | Planet list |
| `?` or `h` | Help |
| `+` / `-`, mouse wheel | Zoom the tactical view |
| `g` | Switch graphics: vector / braille |
| `S` | Sound on / off |
| `Ctrl-L` | Redraw the screen |
| `q` | Quit (asks to confirm); `Ctrl-C` quits immediately |
| `Esc` | Close a popup or cancel a message |

### Outfit screen

| Key | Action |
|---|---|
| `f` `r` `k` `o`, `←` `→` | Choose Federation, Romulan, Klingon, Orion |
| `s` `d` `c` `b` `a` `x`, `↑` `↓` | Choose Scout, Destroyer, Cruiser, Battleship, Assault ship, Starbase |
| `e` / `u` | Choose your empire's [special ship](#special-and-relic-ships) (Captain) or a relic (Commodore), with `--ranks` |
| Enter or Space | Launch |
| `m` | Send a message |
| `q` / `Esc` | Quit |

## How to play

Four empires share a 100,000 × 100,000 galaxy of 40 planets, 10 per empire in its home
quadrant:

| Empire | Home | Quadrant | Colour |
|---|---|---|---|
| Federation | Earth | bottom left | yellow |
| Romulan | Romulus | top left | red |
| Klingon | Klingus | top right | green |
| Orion | Orion | bottom right | cyan |

**The goal is to conquer the galaxy.** The core loop:

1. **Fight** enemy ships with torpedoes and phasers. Destroying a ship gives you a kill
   worth `1 + (victim's kills × 0.1) + (armies they carried × 0.1)`.
2. **Kills let you carry armies:** 2 per kill (3 in an Assault ship), up to your ship's
   capacity. Your kills reset when you die.
3. **Bomb** an enemy planet (`b` while orbiting) to reduce its armies to 4. Bombing
   gives a small amount of kill credit.
4. **Pick up** armies from one of your own planets (`z` while orbiting).
5. **Invade:** orbit the enemy planet and beam your armies down (`x`). Each army you
   beam down kills one defender. When the defenders hit zero the planet becomes neutral,
   and your next army takes it. After bombing a planet to 4, you need **5 armies** to
   capture it. See [Taking and retaking planets](#taking-and-retaking-planets).
6. When an empire loses all its planets it has been **genocided**, and its ships are
   destroyed. When only one empire still holds planets, **the galaxy is conquered**. A
   banner announces the winner and the galaxy resets after 15 seconds.

**Tips:**

- **Planets fire back:** any planet not owned by your team shoots at you when you're
  within 1,500 units. Damage grows with its army count, so keep shields up when
  bombing.
- **Repair and refuel:** orbit a friendly **repair** planet to fix hull and shields
  faster (faster still in repair mode, `R`), and a **fuel** planet to refuel.
  **Agricultural** planets grow armies faster.
- **Speed costs:** warp speed drains fuel and heats your engines. If engine temperature
  passes 100% the engines cut out until they cool. Firing heats your weapons, and
  overheated weapons can't fire for a few seconds.
- **Turning:** fast ships turn slowly (the "new-style" turn rate, which falls with the
  square of speed). Slow down to turn tight.
- **Damage limits speed:** a damaged hull lowers your top speed.
- **Blast radius:** exploding ships damage everything within 3,000 units, so don't die
  next to your friends.
- **Scouting:** your team only sees an enemy planet's owner and armies once someone has
  flown within about 6,000 units of it.

## Ships

Stats come from the Vanilla Netrek server. Speed is warp. Torpedo and phaser figures are
damage; phaser range scales with phaser damage (6,000 units at 100).

| Key | Class | Speed | Shields | Hull | Fuel | Max armies | Torpedo | Phaser | Plasma |
|---|---|---|---|---|---|---|---|---|---|
| `s` | Scout (SC) | 12 | 75 | 75 | 5,000 | 2 | 25 | 75 | – |
| `d` | Destroyer (DD) | 10 | 85 | 85 | 7,000 | 5 | 30 | 85 | 75 |
| `c` | Cruiser (CA) | 9 | 100 | 100 | 10,000 | 10 | 40 | 100 | 100 |
| `b` | Battleship (BB) | 8 | 130 | 130 | 14,000 | 6 | 40 | 105 | 130 |
| `a` | Assault ship (AS) | 8 | 80 | 200 | 6,000 | 20 | 30 | 80 | – |
| `x` | Starbase (SB) | 2 | 500 | 600 | 60,000 | 25 | 30 | 120 | 150 |

Each team can have only one starbase in play.

**Ship designs.** Each empire has its own design language and a distinct shape for each
class:

- **Federation:** saucer section, engineering hull and twin warp nacelles. Miranda-style
  escorts, a Constitution-class cruiser, a Galaxy-class battleship, a cargo-hulled
  assault ship and a Spacedock-style starbase.
- **Klingon:** Birds-of-Prey with wingtip disruptors for the small classes, the D7 and
  K't'inga (command bulb on a long neck, swept wings, wingtip nacelles) for the large
  ones, and a three-armed station.
- **Romulan:** the classic Bird-of-Prey with swept wings for the small classes, the
  double-hulled D'deridex warbird with its open wing for the battleship and assault
  ship, and a finned ring station.
- **Orion:** dagger-hulled pirate raiders with forked side pods, and a hexagonal outpost.

Engines glow brighter with speed, your own ship has a white outline, and the shield
ring changes from blue to yellow to red as your shields weaken.

## Planets

The 40 planets use the original names and coordinates, from Earth, Rigel and Vega
through Romulus, Klingus and Orion.

- **Starting armies:** each planet starts with 17 armies. Home worlds start with 30.
- **Resources:** home worlds have repair, fuel and agriculture. Each quadrant's other
  planets get a random mix of repair, fuel and agricultural worlds each game.
- **Growth:** armies grow slowly on owned planets, and faster on agricultural ones.
- **Neutral planets:** a planet destroyed down to zero armies becomes neutral (grey)
  until someone beams armies onto it.

### Taking and retaking planets

Every planet is captured the same way, whether it belongs to a rival empire, is neutral,
or is held by aliens:

1. **Earn kills.** You can only carry armies once you have kills: 2 per kill (3 in an
   Assault ship), up to your ship's capacity. A cruiser with 2 kills carries 4 armies.
   Kills reset when you die.
2. **Pick up armies.** Orbit one of your own planets (`o`, or `l` on it to autopilot there)
   and press `z`. Armies come aboard about one every 0.8 seconds, and you must leave at
   least one behind.
3. **Bomb the target** (`b` while orbiting) until it's down to **4 armies**; bombing
   can't go lower. The planet fires at you while you're close, and harder the more armies it
   has, so keep your shields up.
4. **Invade.** Orbit it and press `x`. About every 0.8 seconds one army beams down and
   kills one defender. At zero defenders the planet turns **neutral**, and your **next**
   army captures it with 1 army.

**Tips:**

- **Share the load:** one ship rarely carries enough. The defender count carries over
  between ships, so a teammate can beam down first to wear the defenders down and you
  finish the job.
- **Neutral planets** with no armies fall to a single army.
- **Hold what you take:** a new capture starts with just 1 army and grows slowly. Beam
  more armies onto it (`x` while orbiting your own planet) before the enemy comes back.
- **Losing your last planet** means your empire has been genocided (see
  [How to play](#how-to-play)).

**Planets taken by aliens** (with `--aliens`):

| Planet state | How to get it back |
|---|---|
| **Khan's stronghold** | Starts with 40 armies and fires on anyone nearby. Bomb it down to 4, then invade as usual. |
| **Terran Empire conquest** | Starts with 10 armies. Bomb and invade as usual. |
| **Wiped out by Gorn, stripped by the Crystalline Entity, or purged by V'Ger** | Left with no armies. Gorn and V'Ger leave it neutral; the Crystalline Entity leaves the owner in place but kills the armies and its farming. Beam down one army to take it. |
| **Devoured by the planet killer, or destroyed by Species 8472** | Dead grey rock with no armies. One army resettles it, but as **bare rock**: its repair, fuel and farming are gone until the galaxy resets. |

Once the defenders are wiped out, a liberated planet loses its alien colour and turns
neutral grey. Your first army makes it yours. Everything returns to normal when the
galaxy resets.

## Robots

Robots keep the game lively when there aren't enough people. They:

- pick a ship (mostly cruisers, some destroyers and battleships, the occasional scout or
  assault ship)
- fight with leading torpedo shots, phasers and occasional plasma, and weave while
  closing in
- raise shields near enemies, torpedoes or hostile planets, and detonate incoming
  torpedo spreads
- retreat to a repair planet when damaged or low on fuel, and repair there
- bomb enemy planets, pick up armies once they have kills, and invade the weakest
  enemy planets
- go into a **ring frenzy** when the Kzinti's Ringworld arrives: until it leaves they
  fight only for the ring (see [Alien incursions](#alien-incursions))

By default robots play the classic two-empire **Federation vs Romulan** game. Use
`--empires all` (or a list such as `fed,rom,kli`) to have them fly for more empires:

```sh
netrek server --empires all --bots 16    # four empires, 4 ships each
```

Robots keep the chosen empires the same size, counting humans. When a human joins an
empire, a robot on that side steps out. If an empire is genocided, its robots leave and
the rest are shared among the survivors until the galaxy resets. Robots attack every
empire that has ships in play, so in a four-way game each empire fights its neighbours
on two fronts. Robots are marked in the player list.

## Alien incursions

Pass `--aliens` to `server` or `solo` and episodes from Star Trek drop into the galaxy.
One arrives roughly every `--alien-interval` seconds (150 by default, with some random
variation), and at most **two** are active at once. The galaxy has 32 player slots, so
with aliens on the server keeps 8 of them free for incursions (11 with the Kzinti), running fewer robots than
`--bots` asks for if it has to (it says so at startup). Each arrival, defeat and withdrawal
is announced to everyone as a magenta **ALERT** message with a klaxon.

```sh
netrek server --empires all --bots 12 --aliens            # all thirty-two
netrek solo --aliens khan,borg,doomsday --alien-interval 90
```

| Name (`--aliens`) | What happens |
|---|---|
| `khan` | **Khan Noonien Singh** seizes a planet as his Augment stronghold (40 armies, and it fires on everyone). Three augmented Reliant-style ships, faster and much tougher than stock Federation ships, hunt Federation ships first and bomb Federation worlds. |
| `gorn` | **Gorn raiders** (four heavy hammerhead ships) go from colony to colony, orbiting and wiping out the inhabitants all the way to zero armies, and fight anyone who comes close. |
| `tholian` | **Tholian vessels** pick a planet and its two nearest neighbours and **race flat out (warp 12)** round the triangle between them, laying a strand of **Tholian web** behind them as they go and stringing more strands across between each other. Every lap runs a little further inside, so the web fills in with nested triangles until it covers all three planets. Any non-Tholian ship touching a strand takes damage, including ships orbiting those planets. Strands dissolve after 60 seconds, but the Tholians keep spinning. |
| `fesarius` | **The Fesarius**, Balok's vast globe ship, wanders the galaxy hunting ships with heavy beams and tractoring them in. |
| `mirror` | A rift opens and the **Terran Empire** arrives in ships identical to Starfleet's (but silver). They fight everyone, bomb planets down and **conquer** them for the Empire. |
| `doomsday` | **The planet killer** drifts from world to world and **devours** them, leaving dead rock with no armies or resources. Its antiproton beam hits nearby ships, and anything in front of its maw is eaten. Its neutronium hull shrugs off most damage, but, as Commodore Decker showed, **a ship exploding in its maw does 8× damage**. |
| `amoeba` | **The space amoeba** drifts toward ships, drains the fuel of everything within reach, damages it and pulls it in. |
| `borg` | **The Borg cube** hunts the nearest ship, cuts it with beams and torpedoes, and grabs it with a tractor beam. Hold a ship for 4 seconds and it is **assimilated**: destroyed, and replaced by a new cube (up to three). Cubes **adapt**: every hit makes them more resistant, down to taking 30% damage. |
| `vger` | **V'Ger** (*The Motion Picture*): an immense energy cloud heading for **Earth** (then the other home worlds), purging any it reaches. Ships inside the cloud crawl at warp 3, and every few seconds a plasma bolt **digitizes** a ship outright. Weapons are useless. The only way to stop it is to **join with it**: hold position at its core for 10 seconds. That ship is lost, V'Ger transcends, and the pilot gets 5 career kills. |
| `crystal` | **The Crystalline Entity** (TNG): strips all life from planets, farming worlds first, killing their armies and their agriculture for good. It shreds ships that come close. Almost nothing hurts it, but phasers from **three different ships within 2.5 seconds** reach **resonance** and shatter it. |
| `probe` | **The whale probe** (*Star Trek IV*): invulnerable. It travels planet to planet, **draining the power** of every ship within 10,000 units (engines drop to warp 1, shields fail, fuel stops recharging) and stopping army growth where it stops. Bring it **two armies** (the whales) to answer its call: it departs, and the courier earns 3 kills. |
| `8472` | **Species 8472** (Voyager): three bioships from fluidic space with devastating beams. Photon torpedoes and phasers do only 10% damage; **plasma torpedoes** (our nanoprobe warheads) do full damage, and their beams can't shoot plasma down. When the bioships gather at a planet they focus their beams and **destroy it** (never a home world), then recharge for about 40 seconds. They're also **at war with the Borg**. |
| `jemhadar` | **The Jem'Hadar** (DS9): a wormhole opens with a warning, and six seconds later five fast attack ships pour out. Their phased polaron beams **ignore shields**, and a fighter below 30% hull **rams** the nearest enemy for heavy damage. |
| `tribbles` | **Tribbles** (*The Trouble with Tribbles*): an outbreak with no ships. An infested planet (marked `T`) stops growing armies and slowly loses them as its food is eaten, and every 30 seconds the tribbles breed their way to a neighbouring world. Ships that orbit an infested planet **pick them up**: they drain fuel, and the ship **infests the next planet it orbits**. **Tribbles hate Klingons**: Klingon planets are never infested, a Klingon ship orbiting an infested planet clears it in 3 seconds, and they flee any ship a Klingon comes within 2,000 units of. Bombing a planet also clears it. The outbreak is over when no planet or ship carries them. |
| `chang` | **General Chang** (*The Undiscovered Country*): a Klingon Bird-of-Prey that **fires while cloaked**, quoting Shakespeare. You see only a fuzzy blip and its torpedoes. **Any hit** (a stray torpedo, a phaser, even planetary fire) lights up its exhaust and makes it **visible for 20 seconds**. |
| `hirogen` | **Hirogen hunters** (Voyager): three hunters mark the **best pilot in the galaxy** (most kills this life) as their **prey** and chase only them, fending off anyone who gets close. If they kill their prey they take a **trophy**: half the kills the prey made that life come off its career total, and the hunters repair. Then they pick the next prey. A prey that destroys a hunter earns **an extra kill**. |
| `q` | **Q** (TNG): appears near the home world of the empire holding the most planets and puts it **on trial** for 90 seconds, with one of three tests: **hold** your territory (lose no more than two planets), bring Q a **tribute** of five armies, or destroy Q's **champion**, a warship only that empire's weapons can hurt. Pass and every ship of that empire is repaired and refuelled and its home world gains 5 armies. Fail and Q hands its richest colony to the weakest empire. Q himself is invulnerable. |
| `ferengi` | **Ferengi marauders** (*The Last Outpost*): three marauders loot armies from undefended colonies, tractor passing ships to siphon their fuel, and once full (6 armies) run for the edge of the galaxy. If one escapes, its armies are gone. Destroy one and its stolen armies **spill into space** (shown as a gold `+N`), where the first ship of any empire to fly over them takes them. Marauders **surrender** to any battleship or starbase that gets within 2,500 units (+1 kill). |
| `swarm` | **The Swarm** (Voyager): eight tiny, fast ships that **latch onto hulls**, draining fuel and slowly eating the ship. Detonating (`d`) shakes off every swarm ship within range, and the swarm always leaves enough fuel in the tank to do it. Each swarm ship is worth only a fifth of a kill. |
| `tempest` | **The Tempest** (after Atari's 1981 vector arcade game): a neon-blue web, 13,000 across, forms in open space. As in the arcade, it comes in several shapes (a round 16-lane tube, a **square** of 16 lanes, or a **triangle** of 15), starts in a random one, and **reshapes at every new level**. Any empire ship that touches it is **trapped on its rim**: it can only slide around the edge, and there's no escape until the Tempest dies or dissolves. Creatures climb the lanes out of the core in waves, each wave bigger and faster, and ride the web as in the arcade game: small deep in the tube, growing to fill their lane as they reach the rim. **Flippers** (red bowties) climb while flipping end over end into neighbouring lanes, then flip along the rim toward you and **grab** you, dragging you into the core after 5 seconds unless someone shoots them. **Tankers** (purple diamonds) climb dead straight and split into two flippers. **Pulsars** (cyan zigzags) climb slowly, flipping now and then, and electrify their whole lane. **Fuseballs** ride the spokes between lanes, drifting in and out and darting across lanes. They can only be hit while crossing a lane (on a spoke they show as a ghost), and they burn anything they touch at the rim. Trapped ships get one **Superzapper** each (`d`), which destroys everything on the web. The **core** is untouchable while anything is on the web; clear a wave and it's exposed for 20 seconds. Ships trapped on the rim hit it full on; ships outside the web do half damage. It slowly regenerates while shielded, has 1,600 hull, and is worth 6 kills. |
| `nomad` | **Nomad** (*The Changeling*, TOS): a probe out to "sterilise" everything imperfect. It hunts the **most damaged** ships in reach and, with nobody to fix, drains the armies of weak colonies (6 or fewer) until they fall. Weapons can't touch it. Kirk's trick works, though: fly within **3,500 units** and send a message telling it that it is **imperfect** (or has made an **error**, a **mistake**, or has a **flaw**), and it destroys itself. You get the credit (3 kills). |
| `armus` | **Armus** (*Skin of Evil*, TNG): an oily black slick, 5,000 across, that oozes after the nearest ship. Ships inside it are **held** (warp 2 at most), dragged toward the middle and slowly eaten. It feeds on violence: **every shot fired into it makes it bigger**, and every ship it eats makes it bigger still. It never takes damage. Starve it: after 45 seconds without being hit it shrinks back, then **withers away** over 30 seconds. Robots and overwatch hold their fire. |
| `nanites` | **Nanites** (*Evolution*, TNG): an outbreak with no ships. Two ships are infected. Their systems **glitch** every few seconds (the helm swings off course, shields drop, a torpedo misfires), and the nanites **spread** to ships that fly within 1,500 units (up to ten ships). Cure: **orbit a friendly repair world** for 3 seconds, or **detonate** (`d`), which burns them out of your ship and any ship within 3,400 units. The outbreak is over when no ship carries them. |
| `changeling` | **Changelings** (DS9): three ships that **look like one of your own cruisers**, whichever empire you're in: same colour, no alien tag, a plausible name, even their torpedoes. They pick off ships flying alone and slip into orbit over colonies to wipe out the garrison. **Any hit** (yours, a rival's, planetary fire) makes one lose its shape for 30 seconds. |
| `metrons` | **The Metrons** (*Arena*, TOS): the **best pilot in the galaxy** and the best pilot of an empire at war with theirs are snatched away, fully repaired, into a **sealed arena** 10,000 across in open space. Nobody can get in, nobody can get out, torpedoes can't cross the wall, and nothing but the other champion can hurt them. The winner is restored again and gets **3 extra kills**. If neither wins in 2 minutes the Metrons release them, unimpressed. |
| `pakled` | **Pakleds** (*Samaritan Snare*, TNG): three slow, tough clunkers. "We look for things. Things to make us go." One tractors the nearest ship, holds it for 4 seconds and **takes something clever**: an **advanced tech** (with `--ranks` and `--rank-tech`), or else a level of the empire's **supply upgrades** (with `--supply`), or else most of its fuel. Then it runs for the edge of the galaxy. **Destroy the thief** to get it back; if it escapes, it's gone. |
| `10c` | **Species 10-C** (Discovery): a **dark matter anomaly** drifts across the galaxy at warp 2. Any planet it passes over is **wiped clean** (no armies, no owner), and ships within 7,000 units are hurled aside and, close in, torn at. Weapons are useless. Three **hyperfield beacons** circle it 10,000 units out. When ships (any empires: rivals can cooperate) hold **all three at once** for 5 seconds, that's **first contact**: the anomaly withdraws and every ship at a beacon earns 2 kills. |
| `caretaker` | **The Caretaker** (Voyager): a vast array appears in open space. Every 20 seconds its **displacement wave** pulls two ships from anywhere in the galaxy to it, and its beam burns anything that comes close. It's **heavily shielded**: at first it takes only 15% of the damage done to it, but every wave it sends out weakens it by another 15%, so it gets easier to kill the longer it stays. Worth 4 kills. |
| `horta` | **The Horta** (*The Devil in the Dark*, TOS): something tunnels through the rock of a colony and kills an army every 2.5 seconds, then moves on to the next when it's empty. Destroy it (it's a slow, unshielded rock creature, 150 hull, 2 kills), or **make peace**: orbit its planet for **10 seconds without firing**. "NO KILL I." The colony gains 10 armies and becomes a **repair world**, and the peacemaker earns 2 kills. |
| `spheres` | **The Sphere Builders** (Enterprise): four **Delphic Expanse spheres** appear in open space. Each one scorches ships within 3,500 units every 5 seconds and **warps the space around it into anomalies** (a spatial eddy, a chroniton field, a tetryon field or a spatial rift; see [Space terrain](#space-terrain)), up to three each, even when `--terrain` is off. The anomalies vanish when their sphere is destroyed. |
| `kzinti` | **The Kzinti and their Ringworld** (the Kzinti are from *The Slaver Weapon*, the animated series; the Ringworld is from Larry Niven, who wrote it): a vast **Ringworld**, 14,000–18,000 across, appears around a random planet. It can't be destroyed and never withdraws on its own; even a galaxy reset keeps it (with its sections reset). But it only stays while the Kzinti hold it: **once they've lost their last section, the Ringworld is free and jumps out of the galaxy**, taking all ten sections with it, **including any an empire has claimed**. (An empire whose only planets were on the ring is wiped out with it.) Ships orbiting a section are left in open space, orders naming a section are cancelled, and it never comes back. It has **ten sections** that are planets in their own right: ships can orbit (dock at) them, bomb them, and land armies to **claim them for their empire**. Each has its own population (4 to 10 armies) and resources (repair, fuel and farming in different mixes). They're named after Niven's Ringworld: Fist-of-God, Great Ocean, Map of Earth, and so on. One section is **Kzin**, the Kzinti homeworld (12 armies, repair, fuel and farming). The **Kzinti** play like a small extra empire with **three warships, always**: the Chuft-Captain's **dreadnought**, the Telepath's **cruiser** and Flyer's **striker**. A lost ship relaunches from Kzin a minute later (or from any Kzinti world if Kzin has fallen). They carry warriors from home and work hard to **take the rest of the Ringworld**, going for the weakest sections first; once it's all theirs they patrol it. They defend Kzin fiercely, patch up at home when hurt, and "scream and leap" at any ship that comes near. Their worlds regrow slowly (an army every 45 seconds, up to 12), and they'll always try to take Kzin back. A determined push by the empires usually breaks them within a quarter of an hour. Kzinti ships are worth 2 kills. The Kzinti come only once and don't count toward the two incursions at a time; when the Ringworld jumps away, their fleet goes with it. **Robots go into a ring frenzy** while it's here: their goal is to conquer as much of the Ringworld as they can. They land armies on **unclaimed sections first**, then on any enemy section they can take; **bomb the Kzinti's sections first**, then the enemy's lightest, to soften them up; fetch armies from whichever of their worlds is the shortest trip to the ring; and only break off to fight ships that come close or are **raiding their own sections**. They ignore the rest of the galaxy until the ring leaves. |
| `dyson` | **A Dyson sphere** (*Relics*, TNG): a shell 9,000 across materialises around an out-of-the-way colony (in the episode it encloses a star). Nothing gets through the shell, ships or torpedoes, except by its **hatch**. The planet keeps its owner, but only ships inside can reach it (and repair, refuel or fight over it there). An automated **tractor beam** at the hatch locks onto the nearest ship within 15,000 units every 10 seconds and drags it in, whatever its engines do; the hatch opens to take it and shuts behind it. Heavy fire on the **hatch emitter** (200 damage while it holds you) breaks the beam. The hatch only opens to take a ship, but **a ship in the doorway holds it open**, as the Jenolan did, letting everyone in or out. **Destroy the hatch emitter** (600 shields, 1,200 hull, fast repairs, 5 kills) to jam the doors open for good. It's very tough, so bring friends: **a ship blowing up in the doorway does 6× damage** (Scotty's gambit), about a third of its strength for a cruiser. The **wreck of the USS Jenolan** lies on the shell and can be salvaged. |

**How aliens behave:**

- **Enemies of everyone:** aliens fly as independents, so they're hostile to all four
  empires (and robots will fight them). They don't fight each other, except that
  Species 8472 and the Borg are at war and will go after each other on sight. Aliens'
  exploding ships don't hurt their own kind.
- **Callsigns and colours:** they have their own colours and designs, and are labelled by
  name on the tactical view (`Khan`, `Borg`, `ISS`, …). On the galaxy map and player list
  they show as a two-letter tag (`KH`, `BG`, `VG`, `JH`, …) plus a slot.
- **Rewards:** destroying an alien is worth more than a normal kill. Fleet ships give 1.5
  kills, Khan's ships 2, and the monsters 3 to 5.
- **Planets:** planets taken by Khan or the Terran Empire are shown in the alien's colour.
  Planets devoured by the planet killer or destroyed by Species 8472 turn grey.
  Planets infested with tribbles are marked `T` (and shown with a dashed ring in vector
  mode). All of
  them can be retaken with armies, and everything is
  restored when the galaxy resets.
- **Ending:** an incursion ends when all its ships are destroyed (or V'Ger is joined, the
  probe answered, Q's trial judged, Nomad talked into destroying itself, Armus starved,
  the Metrons' duel decided, first contact made with Species 10-C, peace made with the
  Horta, the Dyson sphere's hatch destroyed, or the last tribble or nanite cleared), or it withdraws after 4–6 minutes. The Kzinti
  are the exception: they stay until they've lost the whole Ringworld. If an empire loses its last planet to aliens, it
  has been wiped out by alien invaders.

**Who's who on screen:**

| Alien | Colour | Tag | Looks like |
|---|---|---|---|
| Khan | magenta | `KH` | Reliant: saucer with a roll bar and nacelles slung below |
| Gorn | olive | `GN` | Hammerhead hull with swept engine fins |
| Tholian | orange | `TH` | Crystalline wedge; its web is glowing orange strands |
| Fesarius | pale blue | `FS` | A vast globe of smaller modules |
| Terran Empire | silver | `MU` | Exactly like Federation ships, but silver (labelled `ISS`) |
| Planet killer | steel grey | `PK` | A long cone with an open maw at the front |
| Space amoeba | sea green | `AM` | A wobbling cell with a nucleus |
| Borg | neon green | `BG` | The cube, criss-crossed with conduits |
| V'Ger | blue | `VG` | A vast glowing cloud around a bright core (also shown on the galaxy map) |
| Crystalline Entity | ice white | `CE` | A snowflake of crystal spines |
| Whale probe | bronze | `WP` | A long cylinder with a small sphere at one end |
| Species 8472 | pink | `85` | Organic bioship with a spine and swept claws |
| Jem'Hadar | violet | `JH` | Beetle-shaped attack ship with forward prongs |
| General Chang | crimson | `CH` | Bird-of-Prey: a head on a long neck, wings swept down |
| Hirogen | gunmetal | `HG` | A long arrowhead with a spine and rear fins |
| Q | warm white | `QQ` | A flash of light; his champion is a dark angular warship |
| Ferengi | copper | `FE` | A horseshoe with its prongs forward; spilled armies are a gold `+N` |
| Swarm | lime | `SW` | A tiny dart |
| Tempest | neon blue web | `TP` | Arcade vector art: red bowtie flippers, purple diamond tankers, cyan zigzag pulsars, white spiky fuseballs, a yellow starburst core |
| Nomad | silver | `NM` | A squat cylinder with a sensor head and two side panels |
| Armus | oily violet | `AR` | A pool of tar with a face in it, inside a wobbling black slick with an oily sheen |
| Nanites | aqua | – | No ships: infected ships show **NANITES** (vector mode: **NANO**) on their status line |
| Changelings | amber | `CL` | Your own empire's cruisers, until hit; then a drop of molten gold |
| Metrons | white | `ME` | A radiant figure of light beside a glowing white arena ring |
| Pakleds | khaki | `PA` | A lumpy hull with mismatched pods bolted on |
| Species 10-C | purple | `TC` | A dark spiral swirl, with three purple beacon rings circling it (lit up when held) |
| Caretaker | light blue | `CT` | A vast flat panel array around a central emitter; its waves ripple out in rings |
| Horta | rust | `HO` | A lumpy, speckled rock creature, sitting inside its planet |
| Sphere Builders | teal | `SB` | A great banded sphere |
| Kzinti | tiger orange | `KZ` | Clawed, tiger-striped warships; their worlds (Kzin and whatever they've conquered) show in orange. The Ringworld is a dark band lit blue on its inner face, with seams between the sections and shadow squares turning inside |
| Dyson sphere | bronze | `DY` | A vast dark metal ring around a planet, with a hatch (two great doors under an emitter) that glows when open |

The status line shows **HUNTED** (vector mode: **PREY**) when the Hirogen are after you,
**TRIBBLES** (**TRIB**) when you're carrying them, and **NANITES** (**NANO**) when you're
infected.

**Alien vessels:**

| Vessel | Speed | Shields | Hull | Weapons | Kill credit |
|---|---|---|---|---|---|
| Augment ship (Khan) | 10 | 150 | 150 | torpedo 50, phaser 120 | 2 |
| Gorn raider | 7 | 120 | 170 | torpedo 45, phaser 90 | 1.5 |
| Tholian vessel | 12 | 70 | 80 | phaser 70, web | 1.5 |
| Terran Empire ships | as Starfleet CA / DD / BB | | | | 1.5 |
| Fesarius | 3 | 1,500 | 1,500 | phaser 140, tractor | 4 |
| Planet killer | 2 | 1,000 | 2,500 | antiproton beam 80, maw | 5 |
| Space amoeba | 3 | – | 1,400 | energy drain, tractor | 3.5 |
| Borg cube | 6 | 2,000 | 3,000 | cutting beam 120, torpedo 60, tractor, assimilation | 4 (new cubes 3) |
| V'Ger | 2 | invulnerable | | plasma bolts (instant kill), slowing cloud | 5 career kills for joining |
| Crystalline Entity | 4 | – | 600 | crystal beam 40; shattered by resonance | 4 |
| Whale probe | 3 | invulnerable | | power drain over 10,000 units | 3 for answering it |
| Species 8472 bioship | 11 | – | 300 | beam 110, planet destruction; 10% damage except plasma | 2.5 |
| Jem'Hadar fighter | 11 | 80 | 90 | polaron beam 90 (ignores shields), torpedo 30, ramming | 1.5 |
| Chang's Bird-of-Prey | 9 | 80 | 110 | torpedo 40, phaser 70, fires while cloaked | 3 |
| Hirogen hunter | 10 | 110 | 130 | torpedo 35, phaser 95 | 2 (3 for their prey) |
| Q | 4 | invulnerable | | trials | – |
| Q's champion | 9 | 200 | 250 | torpedo 50, phaser 110; only the accused can hurt it | 2.5 |
| Ferengi marauder | 10 | 90 | 110 | torpedo 25, phaser 70, tractor, fuel siphon | 1.5 (+0.1 per army carried) |
| Swarm ship | 12 | – | 20 | latches on, drains fuel | 0.2 |
| Tempest core | – | – | 1,600 | untouchable unless its web is clear; full damage from the rim, half from outside; regenerates | 6 |
| Flipper / tanker / pulsar / fuseball | climb the web | – | 30 / 60 / 40 / 45 | grab and drag / split / electrify a lane / burn on contact | 0.2 (tanker 0.5) |
| Nomad | 6 | invulnerable | | torpedo 30, phaser 70; destroyed by a logic bomb | 3 for talking it down |
| Armus | 1 | shots make it grow | | engulfs, holds and eats ships | – |
| Changeling ship | 9 | 90 | 100 | torpedo 35, phaser 85, disguise | 1.8 |
| Metron | – | invulnerable | | the arena | 3 for the winner |
| Pakled clunker | 5 | 140 | 160 | torpedo 20, phaser 40, tractor, theft | 1.5 |
| Dark matter anomaly (10-C) | 2 | invulnerable | | wipes planets, shear 7,000 | 2 per ship at a beacon |
| Caretaker's array | – | 300 | 600 | beam 30, displacement waves; takes 15% damage, +15% per wave | 4 |
| Horta | 3 | – | 150 | kills armies from inside a planet | 2 (or 2 for making peace) |
| Delphic sphere | – | – | 350 | pulse 15, plants anomalies | 1.8 |
| Kzinti dreadnought (Chuft-Captain) | 8 | 130 | 160 | torpedo 50, phaser 115, 6 warriors | 2 |
| Kzinti cruiser (Telepath) | 10 | 90 | 110 | torpedo 40, phaser 95, 4 warriors | 2 |
| Kzinti striker (Flyer) | 12 | 60 | 75 | torpedo 30, phaser 75, 3 warriors | 2 |
| Dyson sphere hatch | – | 600 | 1,200 | tractor beam 15,000 (no weapons); 6× damage from explosions in the doorway | 5 |

The monsters regenerate quickly. The planet killer only takes 40% of normal weapon
damage, and a Borg cube's resistance builds as it's hit.

### Surviving the aliens

- **Khan:** his ships outgun a single cruiser, so fight them together. Retaking his
  stronghold takes a lot of bombing first. It starts with 40 armies and fires on anyone
  who comes close.
- **Gorn:** they spend time in orbit wiping out each colony, which makes them sitting
  targets. Catch them while they're busy. They only turn to fight ships within about
  8,000 units.
- **Tholians:** don't fly through the web. It only hurts while you're touching a strand,
  so cross strands quickly or go around, and get your ships out of orbit at the three
  planets it spans. The Tholians are too fast to chase, but they fly a fixed triangle:
  wait near a corner and hit them with phasers or a torpedo spread as they come round.
  Their hulls are thin.
- **Fesarius:** its beams reach about 8,000 units and its tractor pulls you in, so keep
  your distance and shields up and hit it with torpedoes from long range. It moves at
  warp 3, so you can always outrun it.
- **Terran Empire:** ordinary Starfleet ships with ordinary stats. Treat them like any
  enemy fleet, and hurry to save planets they've bombed down before they conquer them.
- **Planet killer:** weapons barely dent it, and its maw eats anything in front of it.
  The reliable way to kill it is Commodore Decker's: a ship exploding right at its maw
  does 8× damage. A damaged ship that's going down anyway makes a fine last stand.
  Otherwise, torpedo it from the side and stay out of its antiproton beam.
- **Space amoeba:** it drains your fuel from 3,200 units away and pulls you in, so don't
  get close. Pound it with torpedoes from range. It's slow, and it has no shields.
- **V'Ger:** don't fight it: nothing works. Fly into the cloud (you'll crawl at warp 3)
  and park at the bright core. Ships at the core are never targeted by its bolts, so
  stay there for 10 seconds and you'll join with it. Everyone else should keep out of the
  cloud, since its bolts pick a random ship inside or near it every few seconds.
- **Crystalline Entity:** torpedoes are wasted. Get three ships (any empires: rivals
  can cooperate) within phaser range and fire together; three hits inside 2.5 seconds
  shatter it. Protect your agricultural worlds, which it goes for first.
- **Whale probe:** you can't hurt it. Pick up two armies (you need a kill first) and fly
  them within 4,000 units of it. Get there before its drain reaches you, because inside
  10,000 units you're stuck at warp 1 with no shields, a sitting duck for anyone else.
- **Species 8472:** only plasma works (`f`, in a DD, CA or BB with 2 kills). Their beams
  reach about 6,600 units and can't stop plasma, so launch from just outside that range.
  When the bioships gather around one of your planets, break them up before they finish
  charging.
  If the Borg are also in the galaxy, let the two fight.
- **Jem'Hadar:** watch for the wormhole warning. Shields don't help against their beams, so
  keep your distance and use torpedoes. Finish wounded fighters from range, or dodge
  them: a badly damaged one will try to ram you.
- **Tribbles:** don't orbit an infested planet (`T`) unless you're a Klingon, and if you
  pick some up, don't orbit your own planets until they're gone. Klingon players are the
  cure, so other empires may want to ask them for help. Robots don't know any of this.
- **General Chang:** watch where the torpedoes come from, then fire a spread or phasers
  that way. The first hit reveals him for 20 seconds, so call it out and pile on. He
  can't hide near enemy planets: their fire lights him up.
- **Hirogen:** if you're the prey (**HUNTED**), fly back to your team and let them
  escort you, or turn and fight: each hunter you kill is worth 3 kills to you. Anyone
  else can pick the hunters off while they chase.
- **Q:** read the trial carefully. For a tribute, beam up armies and fly them within
  3,000 units of Q. For the champion, only your empire's weapons count, so don't expect
  help. Rivals may try to make you fail a hold trial.
- **Ferengi:** they go for colonies with no ships nearby, so parking a ship over a planet
  protects it. Kill them while they're loaded, then race for the loot. A battleship
  simply makes them surrender.
- **Swarm:** as soon as they latch on, press `d`. One detonation clears everything within
  about 1,700 units.
- **Tempest:** stay away from its web unless you mean to fight. Once trapped, keep sliding
  around the rim (on a square or triangle web the rim has corners, and lanes are narrower
  near them) toward the flippers and shoot them *before* they reach you. A grabbed ship
  has 5 seconds, and teammates on the rim can shoot the flipper off. Save your Superzapper
  (`d`) for when the web is crowded. When the core is exposed, pour fire into it: you get 20
  seconds, and it heals a little between waves. Ships on the rim hit it full on and ships
  outside the web at half, so a mix of both works best.
- **Nomad:** don't fight it. If you're damaged, get away from it or get repaired, since
  it goes for the most damaged ship around. Then have a healthy ship fly within 3,500
  units and send "you are imperfect" (any message with *imperfect*, *error*,
  *mistake* or *flaw* will do).
- **Armus:** hold your fire. Every shot makes it bigger. Stay out of its path (it only
  oozes along at warp 1), and if it catches you, crawl out at warp 2 and wait. Ask
  everyone else to stop shooting: it withers 45 seconds after the last hit.
- **Nanites:** press `d` as soon as you're infected, and if a teammate is infected, fly
  alongside and detonate for both of you. Otherwise head for a repair world you own.
  Keep your distance from infected ships (they show on the status line of their pilot
  only, so ask).
- **Changelings:** a "friendly" cruiser that doesn't answer messages, or one orbiting
  your colony while its armies vanish, is suspect. A single phaser shot proves it one way
  or the other. Stick together: they go for ships flying alone.
- **Metrons:** if you're picked, it's a straight duel with a full tank and a fresh ship,
  so fight well: the prize is 3 kills. Everyone else can only watch.
- **Pakleds:** they're slow (warp 5), so keep moving and they rarely catch you. If one
  steals something, chase it down before it reaches the edge of the galaxy.
- **Species 10-C:** move your colonies' armies out of its path (you can't stop it), and
  organise: it takes three ships at the three beacons at once. Rival empires get the same
  reward, so this is a good time for a truce.
- **Caretaker:** if you're pulled in, fight your way clear of its beam (about 6,000 units)
  or join the attack. Early on, it barely takes damage; after four or five waves it's
  soft enough to kill.
- **Horta:** the quick fix is to kill it, but making peace is worth more: park a ship in
  orbit of its planet, don't fire (not even at other things) for 10 seconds, and the
  colony gains 10 armies and repair yards for good.
- **Sphere Builders:** fly around the anomalies they make, not through them, and knock
  the spheres out one at a time: each one you destroy takes its anomalies with it.
- **Kzinti:** the Ringworld is a land grab, but a gamble. Its sections are close together,
  rich and unclaimed, so bring armies early. Expect the Kzinti to jump anything near
  Kzin and to go for the weakest sections first, so garrison yours. Killing their ships
  buys you a minute each, and Kzin itself only starts with 12 armies. The catch: **the moment the Kzinti lose their last section,
  the Ringworld jumps away with every section on it, yours included.** So don't take Kzin
  unless you can afford to lose what you hold there. And if a rival has sunk a lot of
  armies into the ring, finishing off the Kzinti is a way to take all of it from them.
- **Dyson sphere:** give it a wide berth (15,000 units from the hatch). If the beam
  grabs a teammate, it takes several ships pouring fire into the hatch emitter to break it. If you're shut inside,
  the planet can repair and refuel you; wait by the hatch, and when it opens to take the
  next ship, hold the doorway so everyone can get out. Killing the emitter takes a
  coordinated attack: ships that are going down anyway can blow themselves up (`Q`) in the
  doorway, and two or three of those will finish it.
- **Borg:** a cube assimilates a ship it holds in its tractor beam within about 2,600
  units for four seconds. Its tractor is far stronger than any pressor, so don't try to
  push free. Instead stay out of range, and if you're caught, run at full speed: cubes
  only reach warp 6. Hit it hard and early, because every hit makes it tougher, and
  every ship it catches becomes another cube.

## Extras

Eight optional additions that go beyond fighting. Each is its own server option and all
are off by default. `--extras` switches on all eight. The same options work with `solo`.

```sh
netrek server --empires all --bots 12 --extras
netrek solo --terrain --supply --orders
netrek server --ranks /srv/netrek/records.tsv --diplomacy
```

Server commands are typed as a message starting with `/`. Press `/` in game to start
one. `/help` lists the commands the server accepts.

| Command | With | What it does |
|---|---|---|
| `/record` | `--ranks` | Your service record and honours |
| `/tech` | `--ranks` | Your advanced tech and what it does |
| `/orders` | `--orders` | Repeat your current orders |
| `/treaty <empire>` | `--diplomacy` | Offer a treaty (or accept one offered to you) |
| `/break` | `--diplomacy` | Break your treaty (10 seconds' notice) |
| `/treaties` | `--diplomacy` | List the treaties in force |
| `/supplies` | `--supply` | Your empire's stockpile and upgrade levels |
| `/upgrade <name>` | `--supply` | Buy the next level of an upgrade |
| `/build defence\|yard\|sensor` | `--outposts` | Build an outpost on the planet you're orbiting |
| `/fix <system>` | `--subsystems` | Have damage control repair that system first (`/fix` alone: all evenly) |

### Ranks and service records

With `--ranks`, the server keeps a **service record** for every callsign between games.
It tracks kills, deaths, planets taken, armies bombed, orders completed, derelicts
salvaged, time in space and **honours**. It's saved to `~/.netrek/service-records.tsv`,
or the file you give, every 30 seconds and whenever someone leaves. The file is plain
tab-separated text.

- **Points:** a kill's credit, plus half a point per planet taken, a point per order
  completed, and two per honour.
- **Ranks:** follow Netrek's classic ladder. Promotions are announced to everyone.

  | Rank | Points |
  |---|---|
  | Ensign | 0 |
  | Lieutenant | 3 |
  | Lieutenant Commander | 8 |
  | Commander | 15 |
  | Captain | 25 |
  | Fleet Captain | 40 |
  | Commodore | 60 |
  | Rear Admiral | 90 |
  | Admiral | 130 |

- **Starbases** need the rank of **Commander**. Robots are exempt.
- **Honours** are recorded once each: destroying one of the great alien ships (a Borg
  cube, the planet killer, the Crystalline Entity, Chang's Bird-of-Prey and so on),
  joining with V'Ger, answering the whale probe, biting back at the Hirogen, passing Q's
  trial, and clearing tribbles off a planet.
- **On screen:** your rank shows in the player list and `i` info. Your service record and
  the top five careers are on the outfit screen. You're welcomed back by rank when you
  join.

### Advanced tech

With `--ranks`, officers from **Captain** up get advanced tech. Each rank from Captain to
Admiral has three techs, and your ship is fitted with **one from every rank you've
reached**, drawn at random each time you launch. So a Captain has one tech and an Admiral
has five. You're told what's aboard when you launch. `/tech` repeats it. The controls panel lists each tech on its own line, with the key, whether it's ready (or the seconds until it is) and armor left. On a narrow window, where the player list sits under the controls, it's one line above the message log instead. Robots never get tech.
Turn it off with `--no-rank-tech`.

| Rank | Role | The three techs |
|---|---|---|
| **Captain** | Weapons (passive) | **Quantum torpedoes:** 20% more damage, 15% faster, glowing blue-white. **Photon spread:** every torpedo shot is a fan of three. **Phaser overcharge:** 25% more range, and a quarter of the damage goes through shields. |
| **Fleet Captain** | Defence (passive) | **Ablative armor:** 40 points of armor soak up hull damage and come back in repair mode or at repair planets. **Regenerative shields:** shields recharge 3× faster after 5 seconds without a hit. **Metaphasic shields:** immune to terrain hazards, V'Ger's slowing cloud and the whale probe's power drain. |
| **Commodore** | Sensors and tricks, key `v` | **Tachyon sweep:** for 10 s, cloaked and hidden ships within 12,000 show up for you and your allies (60 s cooldown). **Holographic decoy:** a copy of your ship flies on for 15 s; robots and plasma chase it, and one hit dispels it (60 s). **Graviton pulse:** enemies within 4,000 are shoved 2,500 away and their shields are knocked down and jammed for 3 s (45 s). |
| **Rear Admiral** | Heavy weapon, key `e` | **Tricobalt device:** a slow heavy warhead fired at the pointer: 150 damage and a 3,500 blast that **hurts everyone**, you included (90 s, 3,000 fuel). **Isokinetic cannon:** a beam at the pointer out to 9,000 that goes straight through shields for 120 damage (45 s). **Antiproton burst:** strikes every enemy within 4,500 at once (60 s). |
| **Admiral** | Escape and power, key `j` | **Transwarp jump:** charges for 2 s (everyone can see the glow), then jumps 15,000 the way you're heading, breaking tractor beams and orbits (120 s). **Phase cloak:** 6 s out of phase: nothing can touch you, and you can't fire (120 s). **Emergency reserve:** instantly refills shields and fuel and vents all heat (120 s). |

- **Starbases** keep the Captain and Fleet Captain techs but can't use the three
  active ones. An **Admiral who launches in a starbase** gets starbase-only tech in their
  place: one passive and one active (key `j`), each drawn from three:

  | Starbase passive | Starbase active (`j`) |
  |---|---|
  | **Point-defense grid:** shoots down enemy torpedoes and plasma within 3,000, about three a second | **Fighter wing:** launches 3 fighters that hunt enemies within 15,000 of the base, then return after 30 s (120 s cooldown) |
  | **Shield projector:** friendly and allied ships within 6,000 recharge shields as if in repair mode | **Tractor net:** every enemy within 6,000 is held to warp 1 for 6 s (90 s) |
  | **Mobile drydock:** friendly ships holding within 4,000 at warp 2 or less are repaired and refuelled, like at a repair and fuel planet | **Galactic scan:** for 20 s your team sees every cloaked and hidden ship in the galaxy, and every planet is charted (90 s) |

  Refitting into or out of a starbase at your home world refits the tech too.
- **Bounty:** senior officers are worth more to kill: +0.5 kill credit for each rank
  above Commander.
- **Promotions:** new tech is fitted at your next launch after a promotion.

### Special and relic ships

With `--ranks`, senior officers can fly ships nobody else can. Pick them on the outfit
screen, or when refitting at your home world. They show up as extra rows in the ship
table, with what's needed if you don't qualify yet.

**Special ships** (Captain and up, key `e`): each empire has one, in its own style.

| Empire | Ship | Speed | Shields | Hull | Torp | Phaser | What makes it special |
|---|---|---|---|---|---|---|---|
| Federation | **Defiant** | 10 | 120 | 110 | 40 | 90 | Pulse phasers recharge twice as fast |
| Romulan | **D'deridex Warbird** | 8 | 170 | 170 | 40 | 100 | A cheap cloak, and 150-point plasma after just **1** kill |
| Klingon | **Negh'Var** | 9 | 140 | 160 | 50 | 110 | 12 torpedoes in flight instead of 8 |
| Orion | **Corsair** | 12 | 80 | 90 | 30 | 80 | The fastest ship in the game; carries 3 armies per kill |

**Relic ships** (Commodore and up, key `u`): ships of long-vanished or far-off
civilizations, found, captured or salvaged. None of them are races you'll meet as
alien incursions. You don't choose which: one of thirteen is **drawn at random** each time you
launch, and it's drawn in your empire's colours.

| Relic | Speed | Shields | Hull | What makes it special |
|---|---|---|---|---|
| **Iconian gateway ship** | 9 | 120 | 120 | While orbiting one of your planets, lock onto (`l`) another of your planets to step through a gateway straight into orbit there. Recharges in 30 s |
| **Breen warship** | 9 | 130 | 130 | Energy-dampening torpedoes: a hit knocks the target's shields down, jams them for 3 s and drains 500 fuel |
| **Vidiian harvester** | 10 | 100 | 120 | Harvests its victims: 40% of all the damage it deals repairs its own hull |
| **Xindi-Reptilian warship** | 8 | 140 | 150 | Particle-beam phasers pierce, hitting every enemy along the beam, not just the first |
| **Preserver obelisk** | 7 | 200 | 160 | Its deflector turns most enemy torpedoes that come within 1,200 back the way they came, as its own |
| **Talosian illusion ship** | 10 | 90 | 90 | To enemy pilots more than 2,000 away it appears 1,500 to 2,500 from where it really is, drifting every few seconds. Robots aren't fooled |
| **Voth city ship** | 7 | 150 | 200 | A hauler: carries 4 armies per kill (up to 24) and beams armies up and down twice as fast |
| **Kazon raider** | 10 | 90 | 140 | Rams: at warp 6 or more it slams into any enemy within 700, doing 70 damage to them and 20 to itself (every 5 s at most) |
| **Sheliak colony ship** | 8 | 160 | 150 | While it's in orbit around one of your planets, that planet can't be bombed |
| **Kelvan ship** | 9 | 120 | 120 | Neural field: enemy ships within 2,500 can't fire torpedoes (phasers still work) |
| **Husnock warship** | 8 | 140 | 150 | Planet cracker: bombs 3 armies at a time, and can bomb a planet down to 1 army instead of 4 |
| **Suliban cell ship** (`SU`) | 11 | 70 | 80 | Enhanced reflexes: about 35% of shots and hits against it miss entirely |
| **Excalbian shapeshifter** | 9 | 110 | 120 | To enemy pilots more than 3,000 away it looks like one of their own cruisers. Up close, the disguise fails |

Special and relic ships get your advanced tech like any other hull. Robots never fly
them. While you fly one, the controls panel names it and says what it does, above your
advanced tech.

### Orders

With `--orders`, command gives each human player a short task every few minutes:
Starfleet Command, the Romulan High Command, the Klingon High Council or the Orion
Syndicate, depending on your empire. The order and its countdown show above the message
log. Complete it in time for **+1 kill and a full resupply** (fuel, shields and hull).

| Order | What to do | Time |
|---|---|---|
| Scout | Fly within 6,000 of three named planets you know least about | 2½ min |
| Guard | Stay within 5,000 of a frontier planet for 45 seconds | 2 min |
| Reinforce | Beam 3 armies down onto a frontier planet (needs kills) | 3 min |
| Bomb | Bomb a named enemy planet down by 4 armies | 2½ min |
| Capture | Take a weakly held enemy or neutral planet (needs kills) | 4 min |
| Escort | Stay within 4,000 of your supply freighter for 40 seconds (with `--supply`) | 2 min |
| Salvage | Salvage a named derelict (with `--terrain`) | 2½ min |
| Survey | Visit a named nebula, star, pulsar or other feature (with `--terrain`) | 2½ min |

Orders are chosen to fit: you won't be asked to reinforce or capture before you can carry
armies. A new order comes 20 seconds after the last one ends. Robots don't get orders.

### Diplomacy

With `--diplomacy`, empires can sign **treaties of alliance**.

- **Making one:** `/treaty rom` offers the Romulans a treaty. If they have players, any
  of them can accept within 60 seconds by answering `/treaty fed`. Empires run by robots
  decide on the spot, and usually accept unless you're the empire running away with the
  game. Robot empires also look for allies of their own now and then, so expect offers.
- **Allies:**
  - can't hurt each other with phasers, torpedoes or plasma
  - aren't fired on by each other's planets
  - share everything they scout
  - see each other's cloaked ships and army counts
  - can't bomb or invade each other's planets

  Allies also don't trip each other's red alert.
- **Limits:** an empire can have only **one ally** at a time. When no common enemy is
  left, the alliance dissolves so someone can still win.
- **Breaking one:** `/break` gives 10 seconds' notice to everyone before hostilities
  resume. Robot empires break a treaty when their ally grows 8 planets larger than they
  are.

### Space terrain

With `--terrain`, every galaxy gets a fresh set of terrain, placed clear of the planets.
There are **21 kinds**, and each galaxy uses **10 of them, picked at random**, so every game
(and every galaxy after a reset) has a different mix. Terrain is charted for everyone and
drawn on both maps. It affects the empires' ships; aliens ignore it, except the tachyon
grid, and anyone's weapons can set off a Metreon cloud.

| Feature | Looks like | What it does |
|---|---|---|
| **Nebula** ×3 (Mutara Nebula, Briar Patch...) | Purple clouds | Ships inside are **hidden** from anyone more than 3,000 away. Shields won't hold and top speed is warp 6. Good for ambushes and escapes. |
| **Ion storm** | A drifting blue cloud with lightning | Wanders the galaxy. Ships inside are hidden, **phasers are knocked out**, and lightning strikes now and then (12 damage). |
| **Asteroid field** ×2 | Scattered rocks | Faster than warp 4, you take hull damage. Torpedoes that fly in are often **soaked up**, so it's cover. |
| **Black hole** | A black disc in a spinning orange accretion disc | Pulls in ships and torpedoes within 7,000, harder the closer you get. The **event horizon** destroys anything that reaches it. Go to full impulse to break free. |
| **Pulsar** | A white core with sweeping beams | Every 10 seconds a radiation **pulse** hits every ship within 6,500 (up to 55 damage, less farther out). You get a 2-second warning. |
| **Wormhole** | Two swirling violet mouths | Fly into either mouth to come out of the other, across the galaxy. |
| **Derelict** ×3 | A drifting broken hull | Hold still (warp 2 or less) within 900 for 5 seconds to **salvage** it: full fuel and repairs, stranded colonists (up to 2 armies), or its tactical logs (+1 kill). Another wreck turns up elsewhere a minute later. |
| **Slipstream** ×2 | A lane of flowing chevrons | Flying along it (either way) adds **warp 3** and costs no fuel for your engines. |
| **Star** (Amargosa) | A blazing yellow sun | Its corona **refuels** you fast but heats your engines and weapons. The core burns. |
| **Comet** | A bright head with a long tail | Crosses the galaxy, then another comes. Flying through the **tail refuels** you; the head hurts. |
| **Tachyon grid** | A faint cyan grid | **Cloaked ships inside are revealed** to everyone, Chang's Bird-of-Prey included. |
| **Minefield** ×2 | A red dashed boundary with warning markers | Hidden mines (14 per field). Come within 450 of one and it blows: 40 damage. Then a new mine is laid somewhere else in the field, so a safe path doesn't stay safe. |
| **Chroniton field** | Teal rings and clock marks | Time runs slow: ships and torpedoes inside move at **half speed**. |
| **Graviton eddy** | Turning spiral arms | A whirlpool that **sweeps ships around** its centre, hardest near the middle. |
| **Magnetar** | A magenta core in magnetic field loops | **Tractor and pressor beams fail**, torpedoes **curve**, and the core crushes anything that gets too close. |
| **Metreon cloud** ×2 | Amber gas | Volatile: **firing any weapon inside ignites it**, doing 30 damage to the shooter and every ship within 2,500. The gas takes a few seconds to gather again. |
| **Tetryon field** | Flickering yellow-green sparks | Tetryon radiation **strips shields** and keeps them from recharging. |
| **Rogue planetoid** ×2 | Grey cratered rock | **Solid**: ships can't pass through, and it stops torpedoes. It's cover in a fight. |
| **Galactic barrier** | A long shimmering violet wall | **Crossing it** costs 2,000 fuel and 25 hull damage, straight through the shields, and it destroys torpedoes. |
| **Fluidic rift** | A swirling pink-white tear | Fly in and you're **flung to a random spot** in the galaxy. |
| **Abandoned station** | A K-7 style station | A neutral outpost: hold within 1,500 at warp 2 or less to be **repaired and refuelled**, whatever your empire. |

Robots steer clear of the black hole, the star and the comet, and slow down among
asteroids.

### Supply convoys and upgrades

With `--supply`, every fuel and farming world (except home worlds) produces a
**supply** every 20 seconds, holding up to 12. Each empire has a robot **freighter** that
collects them (10 at a time) and hauls them to the home world, where they go into the
empire's **stockpile**. If a freighter is attacked it runs for the nearest friendly
planet and shelters under its guns. A destroyed freighter loses its cargo and is
replaced 30 seconds later. Raiding enemy convoys and escorting your own is a whole new
front.

Supplies buy **upgrades** for every ship in the empire, three levels each. Levels cost
10, 20 and 30 supplies. Any player can buy with `/upgrade <name>`. Empires run by robots
buy automatically, and so do empires with players once the stockpile passes 60.

| Upgrade | Per level |
|---|---|
| `shields` | Shields absorb 10% more |
| `repair` | 25% faster repairs |
| `torps` | 10% more torpedo damage |
| `phasers` | 10% more phaser damage and range |
| `engines` | 20% faster fuel recharge |

Your stockpile and upgrade levels (S, R, T, P, E) show above the message log.

### Subsystem damage

With `--subsystems`, hits that get through your shields to the hull can knock out one of
your ship's **eight systems**. Each has its own health, from 100% down to out:

| System | Lamp | Damaged | Out |
|---|---|---|---|
| Warp drive | `WRP` | Lower top speed | Impulse only: warp 3 at most |
| Impulse engines | `IMP` | Slower to accelerate and turn | Barely manoeuvres |
| Phasers | `PHA` | Weaker, shorter beams | Can't fire |
| Torpedo tubes | `TOR` | Torpedoes misfire | Can't fire torpedoes or plasma |
| Shield generators | `SHD` | Shields recharge slowly | Shields drop and can't be raised |
| Transporters | `TRN` | Armies beam slowly | Can't beam armies |
| Cloaking device | `CLK` | | Can't cloak (and you decloak) |
| Tractor beam | `TRC` | | Can't tractor or pressor (and lets go) |

The bigger the hit, the likelier it is to damage a system and the worse the damage. With
shields up, most hits never reach the hull. A row of lamps under your status line shows
each system: **green** working, **yellow** damaged (with its health), **red** out. You're
told when a system is damaged, knocked out or back online.

**Damage control** repairs every system over time, like the hull: faster in repair mode
(`R`), faster again in orbit of a repair planet, and faster with the repair upgrade. Use
`/fix <system>` (`/fix warp`, `/fix phasers`, `/fix shd`...) to repair one first, marked
`*`; it gets three times the crew while the rest wait. A new ship starts with every
system working.

Robots limp home for repairs when their warp drive is out or they have no working
weapons, and overwatch doesn't try to fire weapons that are out. Aliens don't have
subsystems: their ships work by their own rules.

### Boarding parties

With `--boarding`, the armies you carry can storm an enemy ship. Point at it and press
**`B`**. You need to be within 1,500 units, carrying armies, uncloaked and with working
transporters, and **its shields must be down** (or below 10%). Knock them down first.

- **The fight.** Your marines beam across two at a time, and more follow every second
  while the target's shields stay down. Each round a marine or a defender falls: the
  more of you, the better your odds. Defenders are the ship's **crew** (scout 2,
  destroyer 3, cruiser 4, assault ship 5, battleship 6) plus any armies aboard. Both
  sides see the count each round, and the status line shows **BOARDING** or
  **BOARDED!**. Equal numbers are about a coin toss; twice as many marines almost always
  win.
- **Fighting back.** Raise shields to stop reinforcements (the marines already aboard
  keep fighting), and detonate (`d`) to kill a boarder with each blast.
- **The prize.** Take an empire ship and its pilot bails out (and respawns as normal).
  You get the kill plus a bonus kill, and the hull is yours to **tow home**: it follows
  on a tow line, labelled PRIZE, and holds you to warp 6. Orbit a repair world or your
  home world and its crew joins the garrison (2 to 6 armies, by class), with 10 supplies
  with `--supply` and another kill. Lose your ship on the way and the prize is lost.
- **Aliens.** Crewed alien ships can be boarded too: Khan's augments, the Gorn, the
  Tholians, the Jem'Hadar, Chang, the Hirogen, Q's champion, the Ferengi, the
  Changelings, the Pakleds and the Kzinti. They're taken (no prize), with the usual kill
  credit plus one. A Ferengi's stolen armies spill out and a Pakled gives back what it
  took, just as if it were destroyed. **Don't board a Borg cube**: your marines are
  assimilated.
- Starbases, freighters, monsters and anything weapons can't hurt can't be boarded.

Robots fight boarders off with detonations, and board a shieldless ship themselves when
they're carrying more armies than it has defenders.

### Outposts

With `--outposts`, empires can build on the planets they hold. Orbit one of your planets
and type **`/build defence`**, **`/build yard`** or **`/build sensor`**:

| Outpost | Marker | What it does |
|---|---|---|
| **Defence outpost** | gun tower, `D` | The planet fires 50% further and harder, and is bombed at half speed |
| **Shipyard** | gear, `Y` | Your empire's ships launch from the shipyard nearest where they were lost (instead of home), and can refit there |
| **Sensor array** | dish, `S` | Cloaked ships, and ships hidden in nebulae or ion storms, within 12,000 units show up for your empire (and your allies). Your own arrays draw their range as a faint ring |

- **Building** takes **30 seconds**, and you must stay in orbit: leave, or lose the planet,
  and the work is abandoned. Your status panel shows the time left.
- **Cost:** **15 supplies** from your empire's stockpile (with `--supply`), or, when the
  stockpile is short or supplies are off, **3 of the armies you're carrying**, who stay
  on as the builders. You pay when it's finished.
- **One per planet.** Building another type replaces the old one.
- **Lost with the planet:** an outpost is destroyed when its planet changes hands
  (including sections of the Ringworld, or when the ring jumps away).
- Outposts show as a small badge on the planet in vector mode, as a letter after the
  planet's tags in braille, in the planet list (`P`) and in a planet's info (`i`).

Robots build too: while in orbit of one of their colonies (or a home world near the
front) with the supplies or armies to spare and no enemy about, they put up a shipyard
first, then defence outposts and the odd sensor array.

## Sound

Sound effects are synthesized when the client starts (square waves, sweeps and filtered
noise) and played with the system's command-line player: **`afplay`** on macOS,
**`paplay`** or **`aplay`** on Linux. Nothing needs installing on macOS.

You'll hear:

- torpedo launches, phasers and plasma
- nearby torpedo bursts
- ship explosions, quieter with distance, and a bigger blast when you die
- hull and shield hits
- shields up and down, cloaking, entering orbit
- a red-alert klaxon when an enemy comes close, and when an alien incursion is announced
- a chirp for incoming messages and a beep for warnings
- a fanfare when a planet is captured

Toggle with `S`, or start muted with `--mute`. The temporary sound files live in
`$TMPDIR/netrek-sfx-<pid>` and are removed on exit.

## Hosting a server

```sh
netrek server --port 2592 --bots 6
```

- **Players:** up to **32** at once, in slots `0`–`9` and `a`–`v`. If the galaxy is full
  when a human connects, a robot gives up its slot.
- **Observers:** up to **8** more can watch (`netrek observe`) without taking a player
  slot. Start the server with `--no-observers` to turn them away.
- **Game speed:** the server runs at **10 updates per second** (the original Netrek
  rate) and sends every client its own view of the world each update. Cloaked enemies
  are fuzzed and unscouted planets hidden, so clients can't cheat by reading the
  protocol.
- **Bandwidth:** a frame is about 1–3 KB depending on how busy the galaxy is, so roughly
  10–30 KB/s per player. That's fine over any internet connection.
- **Firewall:** open TCP port 2592 (or whatever `--port` you chose). On macOS the first
  run may ask to allow incoming connections.
- **Local only:** use `--bind 127.0.0.1` to keep the server local.

To add alien incursions to a hosted game, start the server with `--aliens`. Clients
don't need any flag; they draw whatever the server sends.

Clients and servers must be built from the same version (the protocol version is
checked when a client connects).

## Troubleshooting

**It's in braille mode, but I'm using iTerm2.**
You're probably inside tmux without the SIXEL feature enabled. See [tmux](#tmux). Check
with `tmux display -p '#{client_termfeatures}'`: it should list `sixel`. Also check the
outer terminal really is iTerm2. `tmux show-environment -g TERM_PROGRAM` shows what tmux
was started from.

**The maps look stretched or don't fill the window.**
The client measures character cells in pixels. If your terminal doesn't report pixel
sizes it assumes 8×16 cells. Resize the window or press `Ctrl-L` to re-measure.

**Leftover images or garbage on screen.**
Press `Ctrl-L` to clear and redraw everything.

**The mouse doesn't aim.**
Enable mouse reporting in your terminal (iTerm2: *Enable mouse reporting* in the
profile's Terminal settings; Terminal.app: *View → Allow Mouse Reporting*). You can
always play from the keyboard: fire along your heading and turn with the arrow keys.

**No sound.**
Check `which afplay`, your system volume, and that you haven't pressed `S`. Over SSH,
sound plays on the machine running the client.

**"Netrek needs a terminal of at least 80x24."**
Enlarge the window or shrink the font.

**No aliens are showing up.**
Aliens only appear when the *server* was started with `--aliens` (for `solo`, pass it to
`netrek solo`). The first incursion arrives after half the interval (at most a minute),
then about every `--alien-interval` seconds. An incursion needs free player slots (up to
5), so a full 32-player galaxy delays them.

**Can't connect.**
Check the host and port, and that the server's firewall allows the port. Client and
server must be the same version.

## Differences from classic Netrek

This is a from-scratch reimplementation, not a port of the original C code:

- **Protocol:** it uses its own network protocol (length-prefixed bincode over TCP), so
  it can't connect to classic Netrek servers or talk to classic clients.
- **No accounts:** there's no login. With `--ranks` the server keeps a service record
  per callsign, but anyone can use any callsign.
- **Simpler team rules:** any empire that still owns planets can be joined. There's no
  T-mode restriction for humans. Robots play Federation vs Romulan unless you pass
  `--empires`.
- **Simplified mechanics:**
  - army growth, bombing odds and planet fire rates are simplified
  - refitting is instant
  - starbases can't be docked with
  - there's no self-destruct countdown or ghostbusting
- **Additions:** alien incursions (`--aliens`), the extras (`--orders`, `--diplomacy`,
  `--terrain`, `--supply`, `--subsystems`, `--boarding`, `--outposts`, and this version's
  take on ranks), overwatch, per-empire Star Trek ship designs and sound effects aren't
  part of classic Netrek. Observers are, though these ones see the whole galaxy.
- **What matches the original:** the core numbers (ship stats, weapon damage and range,
  explosion radii, orbit distances, fuel and heat costs, turn rates, the planet table)
  come from the original server.

## Project layout

```
Cargo.toml
src/
  main.rs               command-line interface (server / play / solo)
  consts.rs             ship stats, planet table, game constants (from Vanilla Netrek)
  proto.rs              wire protocol: client/server messages, framing
  server/
    mod.rs              TCP server, per-client threads, game loop, robot balancing
    world.rs            the game simulation (one tick = one Netrek update)
    bot.rs              robot pilots
    aliens.rs           alien incursions: scheduling, AI and special powers
    ranks.rs            careers: service records, ranks, honours, leaderboard (--ranks)
    orders.rs           personal orders from command (--orders)
    terrain.rs          space terrain: generation and effects (--terrain)
    supply.rs           supply convoys and upgrades (--supply)
  client/
    mod.rs              connection, input handling, graphics-mode detection, sound triggers
    render.rs           layout, outfit screen, text dashboard, player list, messages, popups
    render_vec.rs       vector-mode maps, graphical control panel and popups
    render_terrain.rs   space terrain, in vector and braille
    palette.rs          colours for empires, aliens and planets; terminal colour conversion
    shipart.rs          per-empire ship designs
    vg.rs               vector canvas on tiny-skia (paths, dashes, gradients, text)
    sixel.rs            SIXEL encoder with adaptive palette; font rendering (fontdue)
    canvas.rs           diffing terminal screen buffer; braille canvas
    sound.rs            sound synthesis and playback
```

**How a frame gets to your screen:**

1. **Server:** simulates the world at 10 Hz and sends each client a `Frame` with the
   ships, torpedoes, phasers and planets that client is allowed to see.
2. **Client, text layer:** draws into a character-cell buffer and writes only the cells
   that changed.
3. **Client, graphics:**
   - In **vector** mode the maps and control panel are drawn with tiny-skia, encoded as
     SIXEL and placed over reserved cells. The text layer never writes into those cells.
     Images are only re-sent when they change. A popup is its own image, painted last
     so it stays on top.
   - In **braille** mode the maps are drawn as braille-dot line art in the text layer.

## Development

```sh
cargo build --release       # optimized build
cargo test --release        # run the tests
cargo run --release -- solo # run straight from source
```

The tests include:

- **`four_empire_game`:** 16 robots across all four empires. Checks every empire gets
  its share of ships and that the fighting spreads across the galaxy.
- **`every_incursion_plays_out`:** runs each of the thirty-two alien incursions against a
  four-empire robot war and checks it arrives, acts and ends cleanly.
- **Alien mechanics:** `vger_merge_ends_the_threat`, `crystal_shatters_on_resonance`,
  `whale_probe_drains_and_is_answered`, `bioships_only_fear_plasma`,
  `polaron_beams_ignore_shields`, `borg_and_8472_fight_each_other`,
  `chang_fires_cloaked_until_hit`, `tribbles_spread_by_ship_and_flee_klingons`,
  `hirogen_trophy_and_bonus`, `q_champion_only_hurt_by_the_accused`, `q_tribute_trial`,
  `ferengi_loot_is_dropped_and_recovered`, `tholians_web_three_planets`,
  `swarm_latches_and_detonation_shakes_it_off`, `tempest_traps_ships_on_its_rim`,
  `tempest_core_is_only_exposed_when_the_web_is_clear`,
  `flippers_drag_ships_into_the_core`, `tempest_shapes`, `nomad_is_talked_to_death`,
  `armus_grows_when_shot_and_withers_when_ignored`, `nanites_spread_and_are_cured`,
  `changelings_look_like_your_own_until_hit`, `metrons_arena_duel`,
  `pakleds_steal_tech_and_give_it_back_when_destroyed`, `ten_c_first_contact`,
  `caretaker_pulls_ships_in_and_weakens`, `horta_peace` and
  `spheres_plant_anomalies_that_vanish_with_them` and
  `dyson_sphere_swallows_ships_and_the_jenolan_gambit`,
  `kzinti_ringworld_arrives_and_stays`, `ring_sections_are_planets_anyone_can_claim` and
  `ringworld_departs_when_the_kzinti_lose_it` and `robots_go_into_a_ring_frenzy`.
- **`retaking_alien_planets`:** liberated and resettled planets lose their alien mark.
- **Extras:** `galaxy_has_ten_kinds_of_terrain_clear_of_planets` (ten of the 21 kinds in
  every galaxy, all of them turning up over many) and a test for each terrain effect; `treaties_between_players`, `one_ally_at_a_time_and_robots_decide`,
  `upgrades_boost_torpedoes`, `starbase_needs_rank`, `tech_comes_with_rank`,
  `captain_weapons`, `fleet_captain_defences`, `commodore_tricks`,
  `rear_admiral_weapons`, `admiral_tech`, `admiral_starbase_tech`,
  `starbase_tech_works`, `special_and_relic_ships_need_rank`, `special_ship_traits`,
  `relic_traits`, `more_relic_traits`, `overwatch_fires_at_enemies_in_range`, `convoys_deliver_and_robots_buy`,
  `scouting_orders_pay_off` and `careers_persist_and_promote`.
- **`extras_game`:** a 20-minute four-empire robot war with every extra and every alien
  switched on, two robots standing in for human players.
- **`outposts_are_built_and_do_their_jobs`** and **`outpost_builds_need_the_builder_in_orbit`:**
  each outpost's effect, the cost, and losing them.
- **`boarding_party_captures_a_ship_and_tows_it_home`** and **`boarders_can_be_repelled`:**
  capture, the prize towed home, detonations and the Borg.
- **`subsystems_knocked_out_and_repaired`** and **`hull_hits_damage_systems`:** each
  system's effect when out, damage control and `/fix`, and that it's all off without
  `--subsystems`.
- **`observers_over_the_wire`** and **`observers_see_everything`:** observers get the whole
  galaxy, follow ships, talk to everyone, and switch between watching and playing.
- **`extras_over_the_wire`:** starts a server with every extra and checks, over a real
  connection, that terrain, supplies and the service record arrive and slash commands
  are answered.
- **`incursions_do_not_repeat_while_active`:** long games with all the aliens, checking
  at most two are active at once and none repeats while it's still active.
- **`robots_play_a_game`:** a headless 30-minute robot game. It checks the rules engine
  holds up (kills happen, no panics) and prints a summary of kills, planet captures and
  armies bombed.
- **`effects_render`:** every sound effect synthesizes to valid, non-silent audio.
- **`gallery`:** renders every empire's ship designs, plus the aliens, to
  `$TMPDIR/netrek-ships.ppm`, handy when tweaking silhouettes.

Dependencies:

- [crossterm](https://crates.io/crates/crossterm): terminal input, output and mouse
- [tiny-skia](https://crates.io/crates/tiny-skia): 2D vector rasterization
- [fontdue](https://crates.io/crates/fontdue): font rasterization
- [serde](https://crates.io/crates/serde) and [bincode](https://crates.io/crates/bincode): protocol encoding
- [clap](https://crates.io/crates/clap): the command-line interface
- [rand](https://crates.io/crates/rand)

## Credits

- **Netrek** was created in 1988–89 by Scott Silvey and Kevin Smith, building on
  *Xtrek* by Chris Guthrie and Ed James. Decades of players and maintainers kept it alive.
- The game constants, ship statistics and planet table were taken from the
  [Vanilla Netrek server](https://github.com/quozl/netrek-server) source, which carries
  its own copyright notices.
- Star Trek and the names of its ships and species are trademarks of their respective
  owners. The ship silhouettes here are loose, original homages.

The Vanilla Netrek source carries this notice:

> Copyright (c) 1986 Chris Guthrie. Copyright 1989 Kevin P. Smith, Scott Silvey.
>
> Permission to use, copy, modify, and distribute this software and its documentation
> for any purpose and without fee is hereby granted, provided that the above copyright
> notice appear in all copies and that both that copyright notice and this permission
> notice appear in supporting documentation. No representations are made about the
> suitability of this software for any purpose. It is provided "as is" without express
> or implied warranty.

## License

This project is released under the [MIT License](LICENSE).
