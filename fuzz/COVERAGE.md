# Parser coverage comparison

LLVM line coverage, with `PROPTEST_RNG_SEED=8128`, using the 33 `sirene-core/src/**/parser.rs`
files (8,297 executable lines). This is a file scope, distinct from the 34 public parser entry
points. The property suite invokes every entry point; the parser replay corpus contains all 34
selector values. The `directive.rs` numbers below are supplemental and excluded from the totals.

| Run | Before | After |
| --- | ---: | ---: |
| Property tests | 5,831 / 8,297 (70.28%) | 5,991 / 8,297 (72.21%) |
| Fuzz corpus replay | 4,512 / 8,297 (54.38%) | 4,798 / 8,297 (57.83%) |

The saved starting reports in this workspace measured 70.28% and 54.38%; the requested earlier
baselines were 70.1% and 54.3%. The table uses the saved reports as the actual before comparison.
The fuzz comparison replays the earlier campaign corpus plus the checked-in seeds and new seeds;
it measures corpus replay, not coverage from the sanitizer smoke runs.

| File | Property lines before → after | Fuzz replay lines before → after | Total lines |
| --- | ---: | ---: | ---: |
| Wardley parser | 256 → 317 | 257 → 363 | 494 |
| EventModel parser | 219 → 274 | 191 → 251 | 355 |
| Railroad parser | 344 → 388 | 329 → 426 | 525 |
| Directive scanner (supplemental) | 141 → 142 | 142 → 142 | 161 |

Newly exercised paths include Wardley pipeline components and recovery, EventModel
`given`/`when`/`then` parsing and recovery, and Railroad grouped alternatives, optional and repeated
expressions; fuzz replay also reaches Wardley evolution and annotations and Railroad ABNF numeric
values and repetitions. The directive properties cover complete multiline settings and malformed
settings followed by a surviving statement; directive scanner coverage was already high.

Run `scripts/parser-coverage.sh PARSER_CORPUS DOCUMENT_CORPUS` to reproduce the exports, which
land in `target/parser-coverage/`. The fuzz numbers replay a local campaign corpus merged with the
checked-in corpora, so a run on the checked-in seeds alone reports lower fuzz coverage.
