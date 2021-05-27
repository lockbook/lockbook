# Navigation
Once a user is signed in, they are presented with a navigation menu. On some platforms it will be side-by-side with an editor.

The navigation view shows the user's file tree. By default, the root (but no other folders) is expanded. The root is not collapsible. Files display their name and relative last modified time. Users are able to sort by name or last modified, ascending or descending. Folders are displayed above files. The default sort is by name.

There is a status bar at the bottom. If you are offline, it indicates that you are offline. If you are out-of-date, it indicates how many files need to be synced. If you are up-to-date, it indicates your relative last synced time.

While a manually requested sync is in progress, a progress bar shows the progress. The bar does not move backwards.

* todo: storage warning?