# Navigation
Once a user is signed in, they are presented with a navigation menu. On some platforms it will be side-by-side with an editor.

The navigation view shows the user's file tree. By default, the root (but no other folders) is expanded. The root is not collapsible. Files display their name and relative last modified time. Users are able to sort by name or last modified, ascending or descending. Folders are displayed above files. The default sort is by name.

Users can fuzzy search for their files by title. Fuzzy search results are presented in order of their match strength with the search string. On platforms with keyboards, the user can navigate the search results with the arrow keys or select one of the top results with a displayted hotkey (e.g. cmd-1, cmd-2, cmd-3 for the top 3 results).

Users can create, move, rename, and delete files from the navigation view.

There is a status bar at the bottom of the navigation. It displays the first applicable status of:
* offline (if offline)
* number of dirty files (if any)
* storage warning (if you are >90% usage)
* relative last synced time

While a manually requested sync is in progress, a progress bar shows the progress. The bar does not move backwards.
