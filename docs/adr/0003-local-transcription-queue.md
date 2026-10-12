# ADR 0003: Local transcription and durable queue

Status: Implemented transcription-only slice; provider quality and distribution remain subject to validation.

## Context and decision

Recordings and SQLite persistence already exist, while ADR 0002 proposes a larger multi-stage pipeline. Implement a backend-owned durable transcription queue now, keeping generation/retrieval outside this change. Actual reference-product model choices are unverified.

Use independently authored Rust orchestration and SQLite transactions with a configured public whisper.cpp CLI adapter. Local processing avoids requiring external audio transfer, authentication, Python packaging, or a GPU. Select a multilingual base model as a practical starting recommendation; validate feasibility with the smaller multilingual tiny model. Runtime/model paths are explicitly authorized and fingerprinted, not silently downloaded. No remote provider is implemented.

Use a unique job per finalized recording, a single worker with an exclusive OS lock, owner-checked checkpoints/commits, three automatic attempts, and up to five lifetime attempts including manual retry. Processing states use the task's `queued`, `processing`, `completed`, `failed`, and `retryable` names. Reconciliation relies on the live execution lock rather than expiry alone. Atomically commit transcripts and completion; keep source audio untouched.

Separate synchronized source tracks are processed in bounded chunks, with source-relative timestamps preserved. Conservative exact-text overlap reduction retains both evidence references. This does not solve acoustic echo cancellation, diarization, or all duplicate speech. See [TRANSCRIPTION.md](../TRANSCRIPTION.md) for the implemented contracts, lifecycle, validation, and limitations.

## Alternatives and trade-offs

- Embedded Whisper bindings would simplify subprocess ownership but add native build/packaging complexity and a bundled model distribution decision. A trusted configured CLI keeps the first adapter replaceable.
- A remote provider can improve convenience/throughput but requires explicit endpoint, credentials, consent, and network failure handling; deferred.
- Python/faster-whisper is a viable future adapter but not required by this CPU CLI choice.
- Whole-meeting decoding provides more context but increases memory and loses granular recovery. Nonoverlapping 60-second chunks bound memory and reuse checkpoints, at the cost of boundary context and model reload time per chunk.
- Mixing source tracks before recognition can avoid some duplicate speech but loses clear source attribution and may distort overlapping speech. Independent source recognition plus conservative duplicate reduction was selected.

## Unresolved follow-up decisions

Bundling/licensing/update verification of runtime/model, quality targets and recommended model by hardware/language, better resampling and chunk overlap, transcript editing/regeneration revisions, retention/encryption/backup, richer attempt history, and eventual generation/retrieval adapters remain open. Existing accurate proposals in ADRs 0001/0002 remain relevant beyond this implemented slice.
