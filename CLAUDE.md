Stack: Rust stable, `serde_json` only. A Cargo workspace: `twin-engine` (model format, checks, machine engine, plant), `twin-page` (3D page files, model store, routes), `twin-modeling-mcp` (the stdio MCP server and the local page). No PLC code.
Build: `~/.cargo/bin/cargo build --release -p twin-modeling-mcp` (`cargo` may not be on PATH). Test: `~/.cargo/bin/cargo test --workspace --offline`.
Run for a check: only with a temporary `--dir` and a free port; never port 8765.
Source of truth: this repository is exported from a private project. Do not edit the skill text or the page here without telling the user; `skills/twin-modeling/SKILL.md` and `guide.modeling` are one text.
Rules: commit messages in Korean; README.md Korean (the default), README.en.md English.
