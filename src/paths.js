/* The one copy of "what is this path called".

   It was three copies before, and they disagreed where it showed: `basename` in
   stores/projects.js split on both separators, `basenameOf` in stores/files.js
   split on `/` alone, and a third in the run dialog's tooltip answered '' for a
   root path where the other two answer the path itself. All three name a folder
   or a file on screen, so the disagreement was visible — a project at a root
   path rendered as an empty gap in a sentence.

   Not in a store, and not next to any one consumer: the consumers are two
   stores and a pure component module, and the family that module belongs to
   (branchChoice.js, columnOrder.js, panelWidths.js) is defined by having no Vue
   and no Tauri in it. Importing a store to borrow one regex would have taken
   both into it. */

/* The path separator differs per system, and WebView2 is among the target
   webviews: we split on both, otherwise on Windows the whole path would become
   the project's name.

   Splitting on `\` costs one exotic case on Unix, where a backslash is a legal
   character in a filename: `a\b.txt` is named `b.txt` here. That is the cheaper
   half of the trade — it misnames a file almost nobody has, in a modal's
   sentence, against misnaming every project on Windows.

   `filter(Boolean)` is what makes a trailing separator harmless; `?? path` is
   for the string that has nothing left after it — a bare `/` is called `/`,
   because a name is more use than an empty gap in a sentence. */
export const basename = (path) => path.split(/[/\\]/).filter(Boolean).pop() ?? path
