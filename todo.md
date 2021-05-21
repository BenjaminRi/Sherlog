# TODOs

## Open:
Right click also sets anchor. Is this behaviour desired?
Copy-paste of log entries: Also paste severity!
Scroll bar if list is small: Bigger slider
Scroll bar if list fits into screen: Block slider (make it as big as the space it resides in)
Time zone selection via GUI (currently times are always shown in UTC)
Drag & drop file into Sherlog to open it
Add help text (--help option)
Minor GUI lag when entire screen is filled with long log lines (has to render too many characters?)
Add context menu and add File -> Open option with file picker
Add "jump to anchor" functionality, either triggered by GUI button or hotkey
Figure out exact type of things like SessionId, LogSource, etc. (u32? i32? u64?... This is largely done)

Mark malformed entries: broken timestamp, double message in same entry, various parsing issues, etc.
Recognize core dumps in sfile. Warn user about presence of core dumps.
At end: Curoffset problems! Overscroll!
Anchor: Overscroll when anchored to end of small subset
Offsets like `first_offset` and `last_offset` point to non-existing elements if log store is empty. These values aren't options. This is dangerous design and may lead to panics if the log store is empty.
Performance optimization in anchoring code, offset code (rel_to_abs_offset, abs_to_rel_offset) and render iter code. We can skip hidden elements thanks to `prev_offset`, `next_offset` in LogEntryExt.
How to render newline chars in log message? Currently they just render as a rectangle.
Search: "Match word" functionality, do not match substring inside word.
Go to date (nearest). Note this is difficult/impossible to implement if the list is not sorted by date, as it becomes ambiguous.
Fold log sources with same name and parent?

Just from looking at a log line, it is hard to tell from which log source it comes. Display log source or colourise it?
Tab support when opening multiple files
Merge multiple sfiles together into the same tree
Save interesting messages into a clue list for quick jumping between them
Open window with loading screen and only then start parsing sfile, so user gets feedback when he double clicks a large sfile.


## Unclear:
Default collapse of TreeView specified by producer (Why by producer? Who is the producer?)
Close file! (File closes just fine, can even be deleted while Sherlog has it open. What was the issue again?)
Copy-Paste logs does not work if we are searching (Seems to work?)

## Done:

Are the timestamps in Xlog always UTC? (yes, this is confirmed)
