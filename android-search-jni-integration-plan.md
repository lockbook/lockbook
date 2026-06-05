# Android Search JNI Integration Plan

## Goal

Integrate the new JNI searcher functions into Android search so Android follows the same contract as iOS: long-lived path and content searchers, synchronous query calls from Kotlin-controlled coroutine contexts, and explicit native handle cleanup.

## Plan

1. Replace one-shot search ownership in `SearchDocumentsViewModel`.
   - Add `private var pathSearcher: PathSearcher?`.
   - Add `private var contentSearcher: ContentSearcher?`.
   - Create them when the search screen/model starts searching, probably in `init` or an explicit `startSearching()`.
   - Close them in `onCleared()`.

2. Introduce Android search modes.
   - Mirror iOS with `Filename` and `Content`.
   - Prefer separate modes over the current combined result list because the new contract returns path and content results separately.
   - Start with filename as the Android default, matching iOS.

   the ui should look like a two sections one that says file name matches and then lists a couple of file name matches followed by another section that has title of content matches with the content list. the user can click expand on any of the sections which then should make it such that all the results from that specifc results are returned and hides the other type of search. there need to be a way to return back from focusing on one search type. 
   
3. Change the query flow.
   - Current flow: `Lb.search(input)` returns mixed `SearchResult[]`.
   - New filename flow: `pathSearcher.query(input)`.
   - New content flow: `contentSearcher.query(input)`.
   - Keep these calls inside the existing `Dispatchers.IO` coroutine context. Do not add threading inside JNI.

4. Update Android result models.
   - Add view-holder data for path results: `id`, `parentPath`, `filename`, and `matchedIndices`.
   - Add view-holder data for content results: `id`, `parentPath`, `filename`, and `matches`.
   - For content rows, fetch snippets through `contentSearcher.snippet(id, match, contextChars)` while converting results for display.

5. Port path highlight behavior carefully.
   - Path indices are against the full path including the leading `/`.
   - Java JNI normalizes `parentPath` like iOS: root stays `/`, nested paths lose the leading `/`.
   - Android highlighting should use the same offsets as iOS:
     - Root filename offset: `1`.
     - Nested parent offset: `1`.
     - Nested filename offset: `parentPath.length + 2`.

6. Render content snippets.
   - Stop using old `DocumentMatch.ContentMatch.paragraph/matchedIndices`.
   - Render snippets as three spans:
     - Gray prefix.
     - Highlighted or bold matched text.
     - Gray suffix.
   - Initially show one or two snippets per document, then decide whether Android needs iOS's focused "show more" view.

7. Handle lifecycle and cancellation.
   - Track the active query job and cancel it before launching a new search, to prevent stale results racing newer input.
   - In `onCleared()`:
     - Cancel the active job.
     - Call `pathSearcher?.close()`.
     - Call `contentSearcher?.close()`.
     - Set both searchers to `null`.

8. Handle errors.
   - Catch `LbError` around searcher creation.
   - Catch `IllegalStateException` around query/snippet only as a defensive guard during lifecycle transitions.
   - Normal query errors should be unlikely because the new JNI searcher query APIs do not throw `LbError`.

9. Verify the integration.
   - Compile with `./gradlew :app:compileDebugKotlin`.
   - Manually test:
     - Open the search screen.
     - Type a filename query.
     - Switch to a content query.
     - Open a result.
     - Back out and reopen search.
     - Rotate or destroy the screen if applicable.
   - Watch specifically for closed-handle calls after navigating away.
