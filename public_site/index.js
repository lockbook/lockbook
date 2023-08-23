let OSName = "Unknown";
let OSDownloadMap = {
	Windows:
		"https://github.com/lockbook/lockbook/releases/latest/download/lockbook-windows-setup-x86_64.exe",
	"Mac/iOS": "https://apps.apple.com/us/app/lockbook/id1526775001",
	Linux: "https://github.com/lockbook/lockbook/releases/latest/download/lockbook-egui",
	Android: "https://play.google.com/store/apps/details?id=app.lockbook",
};
if (window.navigator.userAgent.includes("Windows NT")) OSName = "Windows";
if (window.navigator.userAgent.includes("Mac")) OSName = "Mac/iOS";
if (window.navigator.userAgent.includes("Android")) OSName = "Android";
if (window.navigator.userAgent.includes("Linux")) OSName = "Linux";

document.querySelectorAll(".get-lockbook-gui").forEach((el) => {
	el.textContent = `Get for ${OSName}`;
	el.href = OSDownloadMap[OSName];
  if (el.nextElementSibling === null) return
  let secondaryDownloadMap = {...OSDownloadMap};
  delete secondaryDownloadMap[OSName];
  secondaryDownloadMap = Object.entries(secondaryDownloadMap) 

  el.nextElementSibling.querySelectorAll("a").forEach((secondaryLink,i) =>{
    console.log(secondaryDownloadMap[i])
 
    secondaryLink.href = secondaryDownloadMap[i][1];
    secondaryLink.textContent = secondaryDownloadMap[i][0];
  })
});


/** gui download center navigation **/

