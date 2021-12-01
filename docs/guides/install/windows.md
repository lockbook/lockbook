# Windows Installing

## Sideload from GitHub Release

Download the lastest GitHub release.

### Trust the signature that signed the package

This is to indicate to Windows that you trust the publisher of the application. You should only ever do this once per machine - if you are prompted to do it again, it means the new package was signed by someone else.

1. Right click on the `msix` file > `Properties`
1. `Digital Signatures` tab
1. Select the signature in the list (`lockbook`) > `Details`
1. `View Certificate`
1. `Install Certificate...`
1. Select `Store Location:` `Local Machine`
1. `Next` > `Place all certificates in the following store:` `Trusted Root Certification Authorities`
1. `Next` > `Finish`

## Download from the Microsoft Store (soon)
