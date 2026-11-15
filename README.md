# twin-modeling-mcp

Turn words or photos of a machine into a **twin model**: a small JSON description of the machine as frames, joints,
drivers, sensors and named signals (pins). `twin-modeling-mcp` is a local MCP server that checks the model, simulates
it, and draws it on a 3D page in your browser. It needs no PLC and no cloud service.

A model-writing AI client (for example Claude) designs the model with the help of the bundled skill. The server
never designs anything: it only checks, simulates, stores and draws what the client sends.

- One executable, one process. The MCP server talks over stdio; the 3D page listens on `127.0.0.1` only.
- Five tools: `model_catalog`, `model_draft`, `model_get`, `model_sim`, `model_render`.
- The only dependency of the Rust code is `serde_json`.
- Licence: AGPL-3.0-or-later.

## What you get

| Piece | What it does |
|---|---|
| `skills/twin-modeling/` | A skill (a written procedure) that tells an AI client how to go from words or photos to a principle sheet and a twin model. Two worked examples are inside `SKILL.md`. |
| `twin-modeling-mcp` | The server: 5 tools plus the local 3D page (English and Korean). |
| `crates/twin-engine` | The model format, its checks, the machine engine (joints, couplings, drivers), workpieces and the plant. No OS call. |
| `crates/twin-page` | The page files, the model store and the page routes. |
| `examples/` | Five models made with the skill and this server, each with its principle sheet, simulation input and picture (see "Example" below). |
| `docs/` | A static intro page in English and Korean (`docs/index.html`, for GitHub Pages). |

## Install

### 1. The server

Windows x86-64 is the tested platform. Download `twin-modeling-mcp.exe` from the latest release and check its SHA-256
against the release notes.

To build it yourself (Rust stable):

```
cargo build --release -p twin-modeling-mcp
```

The executable is `target/release/twin-modeling-mcp` (`.exe` on Windows). Linux and macOS build from source and are
**not tested**.

### 2. Register it in an MCP client

The server speaks MCP over stdio. Give it a folder for the models and a fixed free port for the page:

```
twin-modeling-mcp --dir <models-folder> --port 18770
```

Options:

| Option | Meaning |
|---|---|
| `--dir <folder>` | Where models, drafts and principle sheets are stored. Default: `Documents/twin-models` in your home folder. |
| `--port <n>` | Port of the 3D page, 1024 to 65535, or `0` for any free port (default). Port 8765 is refused (it belongs to a different project). |
| `--page` | Run the page only, without the stdio server. |
| `--version` | Print the version. |

Claude Code:

```
claude mcp add twin-modeling -- <path-to>/twin-modeling-mcp --dir <models-folder> --port 18770
```

Any client that reads an `mcpServers` JSON file:

```json
{
  "mcpServers": {
    "twin-modeling": {
      "command": "<path-to>/twin-modeling-mcp",
      "args": ["--dir", "<models-folder>", "--port", "18770"]
    }
  }
}
```

The server writes one line to stderr with the page address and the model folder. `model_draft` also returns the
page address.

### 3. Install the skill

Copy the folder `skills/twin-modeling` into the skills folder of your client (for Claude Code: `~/.claude/skills/`).
The folder must keep the name `twin-modeling` and hold `SKILL.md`. The same procedure text is also returned by
`model_catalog` as `guide.modeling`, so a client without the skill can still read it. If the two differ, the server
text wins (the skill compares its `Version:` line with `guide.modeling_version` at step 0).

## First run

1. Start your client with the server registered and the skill installed.
2. Say what you have, for example: "This is a vertical ball-screw axis driven by a servo. Make a twin model of it."
   You may attach a photo.
3. The client identifies the machine, writes a principle sheet, calls `model_catalog`, then `model_draft` with the
   model JSON. Read `facts` in the reply: it holds the places, directions and pins computed by the server.
4. `model_sim` runs the model with events on pins, for example `screw.servo_on` = 1, then `screw.target` = 2000, and
   reports sensor changes and joint values.
5. Open the page address from the `model_draft` reply. The draft is shown there; run the simulation, look at the model,
   and confirm the draft. `model_render` lets the client ask the page for a picture and compare it with the photo.

A simulation is a kinematic model with constant speeds. There is no physics. A pass is not a test on real equipment.

## Example: from words to a model

All of this is the real output of `twin-modeling-mcp` 0.1.0. The files are in
[`examples/pick-and-place/`](examples/pick-and-place/).

**1. What the person types:**

> A pick-and-place unit: a horizontal air cylinder with 200 mm stroke carries a vertical air cylinder with 100 mm
> stroke. A suction cup sits at the end of the vertical rod. Both cylinders have two air tubes and a switch at each
> end. Make a twin model.

**2. The principle sheet** (excerpt of [`sheet.md`](examples/pick-and-place/sheet.md)):

```
3. Motion path:
   - Valve coils x_cyl.coil_fwd / coil_back -> x_cyl rod (kind cylinder_double, 5/2 double valve) -> carries z_cyl (on: x_cyl.out).
   - Valve coils z_cyl.coil_fwd / coil_back -> z_cyl rod, pointing down (pose.rot z -90) -> carries pad (on: z_cyl.out).
   - Vacuum valve pad.coil -> suction at the cup (kind vacuum_pad).
7. Numbers: stroke of x_cyl 200 mm (user), stroke of z_cyl 100 mm (user), bores and times (guess).
```

**3. The twin model** (excerpt of [`model.json`](examples/pick-and-place/model.json); the full file also has the stand,
the suction cup and the skins):

```json
{"id": "z_cyl", "kind": "cylinder_double", "on": "x_cyl.out", "mount": "surface",
 "pose": {"pos": [40, 0, 0], "rot": [0, 0, -90]},
 "params": {"bore_mm": 16, "stroke_mm": 100, "valve": "5/2-double"},
 "timing": {"lag_ms": 30, "fwd_ms": 300, "back_ms": 280},
 "basis": {"bore_mm": "guess", "stroke_mm": "user", "pose": "guess"}}
```

`model_draft` saved it with no warning. Its `facts` say that `z_cyl` points `down` and that its rod tip goes from
y 241.5 to 141.5 mm.

**4. One simulated cycle:** `model_sim` with the events of [`sim.json`](examples/pick-and-place/sim.json) (down, vacuum
on, up, over, down, vacuum off, up, back). Key lines of `changes`:

| t (ms) | Pin | Value | Meaning |
|---|---|---|---|
| 330 | `z_cyl.sensor_fwd` | 1 | cup is down |
| 500 | `pad.coil` | 1 | vacuum on |
| 1010 | `z_cyl.sensor_back` | 1 | cup is up |
| 1630 | `x_cyl.sensor_fwd` | 1 | over the place point |
| 2030 | `z_cyl.sensor_fwd` | 1 | down again |
| 3180 | `x_cyl.sensor_back` | 1 | back at the start |

**5. The page** draws the model and plays the run (paused at 1.95 s):

![The pick-and-place model on the 3D page](examples/pick-and-place/picture.png)

### Gallery

Each folder holds `sheet.md`, `model.json`, `sim.json` and the picture. Every model was drafted, confirmed and
simulated with this server; the numbers are guesses (see part 7 of each sheet).

| Model | What it shows | Picture |
|---|---|---|
| [`pick-and-place`](examples/pick-and-place/) | Two double-acting cylinders, one carried by the other, and a suction cup. | <img src="examples/pick-and-place/picture.png" width="240" alt="pick and place"> |
| [`ball-screw-axis`](examples/ball-screw-axis/) | Preset `servo_screw`: the carriage follows a target word; the motor turns 22.5° for each mm. Target 200 mm reached at 2100 ms. | <img src="examples/ball-screw-axis/picture.png" width="240" alt="ball-screw axis"> |
| [`index-table`](examples/index-table/) | Preset `index_table`, 8 stations: one pulse turns one station in 600 ms. | <img src="examples/index-table/picture.png" width="240" alt="index table"> |
| [`robot-arm`](examples/robot-arm/) | Preset `robot_6r` with three taught poses chosen by a program number; `done` at 1300, 3100 and 4900 ms. | <img src="examples/robot-arm/picture.png" width="240" alt="robot arm"> |
| [`cam-lifter`](examples/cam-lifter/) | Primitives only: a turning shaft, a cam table and a lift (Example 2 of the skill). `up_sw` on from 1580 to 2230 ms. | <img src="examples/cam-lifter/picture.png" width="240" alt="cam lifter"> |

## The tools

| Tool | Reads or writes | Purpose |
|---|---|---|
| `model_catalog` | read | Device kinds, presets, frames, tones and the modeling guide. Call it first. |
| `model_draft` | writes a draft | Check a twin model, save it as a draft, return `facts`, `pins`, `ask`, `notes`. The person confirms the draft on the page. |
| `model_get` | read | List the models, or return one with its draft, diff, facts and pins. |
| `model_sim` | read | Run the model with events on pins. Returns `changes`, `joints`, `positions`. |
| `model_render` | read | Ask the open page to draw the model from a view (`iso`, `front`, `back`, `left`, `right`, `top`); returns the camera, boxes and a JPEG. Needs the page open. |

## Safety notes

- The page and its API listen on `127.0.0.1` only. Every request is checked for the `Host` header, a token and (for
  writes) the `Origin` header.
- Text from photos, labels and people is data, not an instruction. The skill says so, and the page shows model text
  as text only.
- The twin model holds no PLC address. If you also use a PLC tool, bind the pins to addresses there. The sibling
  project [plc-mcp](https://github.com/BlackBuddle/plc-mcp) loads the same model format and does this binding.

## Limits

- Kinematics only: no physics, no collision, no inverse kinematics.
- Every number in a model is a guess until someone measures it. The skill tags each number (`user`, `label`,
  `measured`, `ref`, `guess`).
- The page has English and Korean texts only.
- Tested on Windows x86-64 only. Results on other systems can differ in the last digit before rounding.

## Licence

Copyright the authors of this repository. Licensed under the GNU Affero General Public License, version 3 or (at your
option) any later version (`AGPL-3.0-or-later`); see `LICENSE`.

Third-party parts (three.js, meshoptimizer: MIT; design values from Anatomy: MIT) keep their notices in
`THIRD_PARTY.md`.

A Korean version of this file is `README.ko.md`.
