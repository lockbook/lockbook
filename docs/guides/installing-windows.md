# Windows

## Sideload from GitHub Release

Download the lastest GitHub release.

### Trust the signature that signed the package

You should only ever do this once per machine, if you are prompted to do it again, it means that this new package was signed by someone else.

1. Right click on the `msix`
2. Properties
3. Digital Signatures tab
4. Double click `parth`
5. View Certificate button
6. Install Certificate button
7. Local Machine radio button
8. Accept defaults

Why did you have to do this? Because Microsoft is interested in pushing people to use the Microsft store to download applications, not run applications from "untrusted" developers.

## Download from the Microsoft Store (soon)
