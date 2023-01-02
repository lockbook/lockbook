let OSName = "Unknown";
let OSDownloadMap = {
    "Windows":"https://github.com/lockbook/lockbook/releases/download/0.5.6/lockbook-windows-setup-x86_64.exe",
    "Mac/iOS":"https://apps.apple.com/us/app/lockbook/id1526775001",
    "Linux": "https://github.com/lockbook/lockbook/blob/master/docs/guides/install/linux.md",
    "Android": "https://play.google.com/store/apps/details?id=app.lockbook"
}
if (window.navigator.userAgent.includes("Windows NT")) OSName="Windows";
if (window.navigator.userAgent.includes("Mac")) OSName="Mac/iOS";
if (window.navigator.userAgent.includes("Android")) OSName="Android";
if (window.navigator.userAgent.includes("Linux")) OSName="Linux";

document.querySelectorAll(".get-lockbook").forEach(el => {
    el.textContent = `Get for ${OSName}`
    el.href = OSDownloadMap[OSName]
})