# rivercrossing-htmlexport — fixtures

Byte-frozen oracles copied from the Python app (`/Users/mark/src/rivercrossing/tests/unit/fixtures/htmlexport/`).
Never regenerate these bytes; they are the EPIC-9 acceptance authority for the HTML exporter.

## Production goldens (byte-identical targets)

| File | sha256 | Bytes |
|---|---|---|
| `epic-2026-results.html` | `83c22cbd6302371bf6547c893a753aaee4bf9eb7f46bbc758ee39df9cc7c90f5` | 230,527 |
| `epic-2026-results-no-times.html` | `4c46c4dc8addf86250b7c042467b77be4091c3c253ea335f8f61109becce3459` | 219,464 |
| `epic-2026-results-solo.html` | `5b918a24dd657b2c4c9603ea99700f667a064570032fd6104c59f3da5e52cc88` | 191,698 |

These are the production pages (`dev=false`, inlined `fonts_css` + `compiled_css`, transparent 1×1 logo).
They are self-contained: one `<script>` (the `race-data` block), one `<style>`, zero external references.

## Payload records (value oracles)

| File | sha256 | Bytes |
|---|---|---|
| `payload-times.json` | `54f131bddcc6750f7606094a6f290c1aaeaed79e85500c86f92fde807d32ea33` | 19,186 |
| `payload-no-times.json` | `c2d2f5942cec0dcbe1311773a7ba7f1f61b1524c77bddd38d8f298a6b81b0462` | 15,877 |
| `payload-solo.json` | `d4b61173a4b6ca5af0f223d70dfcac0b96ecd88f77071c9eb80de8eb9c51a641` | 7,263 |

Each is `json.dumps(record, indent=2, ensure_ascii=False) + "\n"`. The `race-data` block in a golden is
the payload file minus its trailing newline.

## Render-only census (`GOLDEN_RIDERS`)

| payload | unique riders |
|---|---|
| `payload-times.json` | 262 |
| `payload-no-times.json` | 262 |
| `payload-solo.json` | 180 |

`riders` is render-only and is **not** in the record.

## Transparent 1×1 logo fallback

```
data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==
```

## Vendored template note

`templates/` is a byte-identical copy of `design/assets/htmlexport-templates/`, except for one
output-identical rewrite required by the minijinja engine (E11 in the EPIC-9 plan): the two
`{% set bragging = [] %} … .append(…)` lines in `base.html.j2` and `wordpress.html.j2` are replaced
by a plain `{% if %}`/`{% set %}` chain and the following `{{ bragging | join(' and ') }}` becomes
`{{ bragging }}`.
