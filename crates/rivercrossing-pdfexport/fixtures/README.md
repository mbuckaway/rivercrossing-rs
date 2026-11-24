# rivercrossing-pdfexport — fixtures

- `epic-2026-results.pdf` / `epic-2026-podium.pdf` — the Python fpdf2 output, copied from the
  oracle as **reference only**. They are never byte-compared (cross-engine byte parity is impossible);
  they document the oracle's layout and copy.
  sha256: `fa99751145017f4f4e7b0c6a6334e6cf740373fa49f5f449cfd9c4620a03c22a` (results),
  `e98ddbcca4a74ab18da42e71b1ff66a040e9d3aa49cd355b86bdd6084400a637` (podium).
- `content-oracle.json` — the fixed literal copy (section headings, captions, footer fragments) the
  Rust report/poster must render, captured from the oracle. The content-parity test asserts each
  string appears in the Rust renderer's text.
- `golden-results.pdf` / `golden-podium.pdf` — Rust-owned, deterministic byte goldens, generated once
  from the `src/fixtures.rs` mixed field and frozen. The committed tests
  (`report::tests::test_render_matches_the_committed_golden`,
  `poster::tests::test_podium_poster_matches_the_committed_golden`) only compare.
  sha256: `95303fc1db995602c2f7c3fa9146b5e21c15af8ac76ad765efc57bfc8689e10c` (results),
  `1c3249cc682690c3603f2a6b6bb3436c9f83d35cb1f0af1e583781f9211255de` (podium).

  **Regeneration procedure** (deliberate one-off, never an env-gated test):

  1. Add a temporary `#[test]` to `src/report/tests.rs`:

     ```rust
     #[test]
     fn test_generate_pdf_goldens_once() -> Result<(), Box<dyn std::error::Error>> {
         let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
             .join("target")
             .join("golden");
         std::fs::create_dir_all(&out)?;
         let input = crate::fixtures::golden_input();
         render(&input, &out.join("golden-results.pdf"))?;
         crate::poster::podium_poster(&input, &out.join("golden-podium.pdf"))?;
         Ok(())
     }
     ```

  2. Run it alone:
     `cargo nextest run -P unit --locked -E 'test(=report::tests::test_generate_pdf_goldens_once)'`.
  3. Review the diff by opening both PDFs, then copy
     `target/golden/golden-results.pdf` and `target/golden/golden-podium.pdf` into `fixtures/`.
  4. Delete the temporary test. The committed suite compares the bytes only; no test writes into
     `fixtures/`.
