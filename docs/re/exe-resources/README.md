# Executable Resource Maps

Per-executable TOML files that tell `td-assets` where to find text and tables **inside the user's original executables**, so that the port never needs to contain game text (ADR-0003, MDG §9.1.4).

Example shape (offsets illustrative; real values come from M1-11):

```toml
exe = "file0001.exe"
sha256 = "<from FILE_INVENTORY.md>"

[[resource]]
id = "e1.mission_title.01"
kind = "pstring"          # pstring | pstring_seq | fixed_width_text | u8_table | i16_table | u16_table
offset = 0x4F39           # file offset
note = "title card before E1M01"

[[resource]]
id = "e1.objective_msgs"
kind = "pstring_seq"
offset = 0x18D91
count = 20
```

Rules: offsets are **file offsets**; every entry needs a stable `id` (the code refers to resources only by `id`); `tdtool exe-res --verify` must pass for every map. Never put the text itself in these files.
