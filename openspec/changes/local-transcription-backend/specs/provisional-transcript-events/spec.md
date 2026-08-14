## ADDED Requirements

### Requirement: A provisional update event replaces segment text
The event model SHALL gain a `SourceUpdate { item_id, text }` variant that replaces the text of the named segment without sealing it. `SourceDelta` SHALL continue to append and `SourceCompleted` SHALL continue to replace and seal.

#### Scenario: Successive hypotheses replace rather than accumulate
- **WHEN** a backend emits `SourceUpdate` for segment `local-0` with "the qui", then "the quick", then "the quick brown"
- **THEN** the assembled transcript for that segment reads "the quick brown"

#### Scenario: Revised earlier words are reflected
- **WHEN** a backend emits `SourceUpdate` with "recognise beach" and then `SourceUpdate` with "recognise speech" for the same segment
- **THEN** the assembled transcript reads "recognise speech" with no trace of the earlier hypothesis

#### Scenario: Append semantics are unchanged for the remote path
- **WHEN** the OpenAI backend emits `SourceDelta` fragments "Hel" and "lo" for the same item id
- **THEN** the assembled segment reads "Hello", exactly as before this change

### Requirement: Completed segments are sealed against later updates
`TranscriptAssembler` SHALL provide an `update` operation that sets segment text and leaves the segment provisional. Once a segment is completed, subsequent `update` and `delta` operations on that segment SHALL be ignored.

#### Scenario: Update after completion is ignored
- **WHEN** a segment receives `SourceCompleted` with "final text" and then a late `SourceUpdate` with "stale text"
- **THEN** the segment still reads "final text"

#### Scenario: Completion overwrites a provisional segment
- **WHEN** a segment holds the provisional text "the quick brow" and then receives `SourceCompleted` with "the quick brown fox"
- **THEN** the segment reads "the quick brown fox" and is sealed

#### Scenario: Segment ordering is preserved across mixed operations
- **WHEN** segment `local-0` is updated, segment `local-1` is updated, and then segment `local-0` is completed
- **THEN** the assembled transcript presents `local-0` before `local-1`, in first-seen order

### Requirement: Local backends assign stable segment identifiers
Local backends SHALL assign every emitted event a stable, non-empty `item_id`, monotonically increasing within a session (`local-0`, `local-1`, …). Local backends SHALL NOT emit events with an absent `item_id`. An absent `item_id` remains supported only for remote events that lack one.

#### Scenario: Ids are stable across a segment's lifetime
- **WHEN** a speech segment produces three provisional updates and one completion
- **THEN** all four events carry the same `item_id`, and the transcript contains exactly one segment for them

#### Scenario: A new segment gets a new id
- **WHEN** the engine closes one segment and opens another
- **THEN** the new segment's `item_id` differs from every previously used id in the session

#### Scenario: Anonymous accumulation is avoided
- **WHEN** a local session emits provisional text repeatedly
- **THEN** no event reaches the assembler's anonymous-segment path and the transcript does not accumulate duplicate fragments

### Requirement: The transcript renders as a single assembled text
The assembler SHALL continue to expose the transcript as one assembled string, and the transcript pane SHALL remain a plain editable text area. Provisional segments SHALL NOT be visually distinguished from finalized ones in this change.

#### Scenario: Provisional text is displayed like any other text
- **WHEN** a segment is provisional
- **THEN** its text appears in the transcript area with the same styling as finalized text

#### Scenario: Editing and saving behave identically in both modes
- **WHEN** the user edits, copies, or saves a transcript produced by a local session
- **THEN** the behaviour matches a transcript produced by a remote session
