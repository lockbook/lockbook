# Launch
When users launch the app, it checks if there is a lockbook account on the device. If so, the app proceeds to naviation. Otherwise, it prompts for sign-up or import.

If the user opts to create an account, they enter a name and click to submit. While they are entering the name, the client checks with the server if the name is available. If it is available and they click to submit, they are presented with a readable TOS which explains self-managed keys. If they accept, the client sends the request to create the account, and if the operation succeeds, the app proceeds to navigation. Otherwise, an unobstructive message appears to represent the failure and the user can retry.

If the user opts to import an account string, they enter the string, and if valid it automatically submits and proceeds to navigation with no further button press required. If invalid, an unobstructive message appears and the user can retry.

If the user opts to import with a QR code (supported devices only), the camera is opened and as soon as the QR code is detected it is scanned, validated, and submitted. If invalid, they are returned to the launch page where an error message appears and they need to re-select QR code import if they wish to retry.