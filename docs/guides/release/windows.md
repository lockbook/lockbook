# Windows Releasing

0. Set up your [build environment](/docs/guides/build/windows.md:5) and get the app signing certificate from Travis
1. In Visual Studio's Solution Explorer, right click the `lockbook` project > `Publish` > `Create App Packages...`
2. Select `Sideloading`; deselect `Enable automatic updates`
3. Next > `Yes, select a certificate` > `Select From File...` > select the certificate that Travis gave you
4. Next > Deselect `Automatically increment`; enter a version number; select `x64` architecture and deselect all others; choose `Release (x64)` in the dropdown by `x64`
5. Create

Upload the file to GitHub Releases