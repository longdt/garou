QUIC Connection
│
├── Control (bi) ───────────────────── auth, ping, shard management
│
├── CLIENT → SERVER
│   ├── Chat Commands (uni) ────────── text, reactions, edits
│   ├── Bulk Upload (uni, on-demand) ─ files, images, voice
│   └── ACKs (uni) ─────────────────── delivered, read
│
├── SERVER → CLIENT
│   ├── Shard 0 (uni) ──────────────── rooms where id % 8 == 0
│   ├── Shard 1 (uni) ──────────────── rooms where id % 8 == 1
│   ├── ...
│   ├── Shard 7 (uni) ──────────────── rooms where id % 8 == 7
│   └── Hot Room X (uni, temporary) ── promoted high-traffic room
│
└── DATAGRAMS ──────────────────────── typing, presence
