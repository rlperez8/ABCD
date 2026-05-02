# Recovery Status - 2026-05-02

Recovered locations:
- GitHub base clone: `C:\Users\rpere\Desktop\abcd_local_v3_github_shallow_20260502_165339`
- VS Code local history snapshot: `C:\Users\rpere\Desktop\abcd_local_v3_vscode_history_recovered_20260502_164828`

Recovery branch:
- `recovery/rebuild-local-work`

Local recovery commit:
- `bbcc771 Recover futures engine local work`

What is restored in the recovery commit:
- Futures candle source selection with `ABCD_CANDLE_SOURCE=futures_contracts`
- Futures root filtering with `ABCD_FUTURES_ROOT`
- One-minute `DATETIME` support through the scanner/server models
- Delayed live-style entry after D pivot confirmation
- C-level target with mirrored stop from the actual entry
- Contract week/day fields for futures outcome rows
- Prop family refresh skip via `ABCD_SKIP_PROP_FAMILY_SUMMARIES`
- Legacy `xabcd_patterns` table drop/no runtime recreation

Verified:
- `cargo check --bin abcd` passes in `rust_sr\abcd`
- `cargo check` passes in `rust_sr\server`

Not fully recovered from disk:
- Exact uncommitted UI simulator implementation after the last GitHub push
- Exact deleted local file contents that were not in GitHub or VS Code local history
