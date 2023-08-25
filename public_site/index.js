/** Detect the user's platform and estimate his seconday platform for his laptop/phone 
 * then recommend content accordingly
 * **/

let platforms = {
	Windows: {
    hrefGUI : "https://github.com/lockbook/lockbook/releases/latest/download/lockbook-windows-setup-x86_64.exe",
    hrefCLI : "https://github.com/lockbook/lockbook/releases/latest/download/lockbook-windows-cli.zip",
    header : "Secure e2e note-taking for Windows",
    subHeader: "Embrace our native app that blends with your Windows ecosystem. Enjoy polished, privacy-focused notes"
    
  },
  "macOS/iOS": {
    hrefGUI : "https://apps.apple.com/us/app/lockbook/id1526775001",
    hrefCLI: "https://github.com/lockbook/lockbook/releases/latest/download/lockbook-cli-macos.tar.gz",
    header: "Beautiful, native note-taking for your apple ecosystem",
    subHeader: "Embrace a secure, end to end encrypted notes with an luxurious native feel; built in Swift for Macs, iPhones, and iPads "
  },
	Linux: { 
    hrefGUI: "https://github.com/lockbook/lockbook/releases/latest/download/lockbook-egui",
    hrefCLI: "https://github.com/lockbook/lockbook/releases/latest/download/lockbook-cli",
    header: "Open source Linux client, written in Rust",
    subHeader: "Enjoy a lightweight and secure note-taking experience on our Egui app . No electron, no bloated webviews to clog up you system!",
  },
  Android:  {
    hrefGUI:"https://play.google.com/store/apps/details?id=app.lockbook",
    header: "Take notes on the fly",
    subHeader: "Get Lockbook's android client and enjoy simple, and secure note-taking."
  },
};
function getUserPlatform(){
  let result = [];
  if (window.navigator.userAgent.includes("Windows"))  result=["Windows", "Android", "Windows"];
  if (window.navigator.userAgent.includes("Mac")) result=["macOS/iOS", "macOS/iOS", "macOS/iOS"];
  if (window.navigator.userAgent.includes("Android")) result=["Android", "Windows", "Windows"];
  if (window.navigator.userAgent.includes("Linux")) result=["Linux", "Android", "Linux"];

  return result;
}

let [userPrimaryPlatform, userSecondaryPlatform, userCLIPlatform] = getUserPlatform()


document.querySelectorAll(".get-lockbook-gui").forEach((downloadBtn) => {
	downloadBtn.textContent = `Get for ${userPrimaryPlatform}`;
	downloadBtn.href = platforms[userPrimaryPlatform].hrefGUI;

  let hasOtherPlatformDownloads = downloadBtn.nextElementSibling !== null;
  if (!hasOtherPlatformDownloads) return // the navbar btn falls under this case

  let nonPrimaryPlatforms = {...platforms};
  delete nonPrimaryPlatforms[userPrimaryPlatform];
  nonPrimaryPlatforms = Object.entries(nonPrimaryPlatforms) 

  downloadBtn.nextElementSibling.querySelectorAll("a").forEach((nonPrimaryLink,i) =>{
    nonPrimaryLink.href = nonPrimaryPlatforms[i][1].hrefGUI;
    nonPrimaryLink.textContent = nonPrimaryPlatforms[i][0];
  })
});

document.querySelectorAll(".get-lockbook-cli").forEach((downloadBtn) => {
	downloadBtn.textContent = `Get for ${userCLIPlatform === "macOS/iOS" ? "macOS": userCLIPlatform}`;
	downloadBtn.href = platforms[userCLIPlatform].hrefCLI;

  let hasOtherPlatformDownloads = downloadBtn.nextElementSibling !== null;
  if (!hasOtherPlatformDownloads) return 

  let nonPrimaryPlatforms = {...platforms};
  delete nonPrimaryPlatforms[userPrimaryPlatform];
  nonPrimaryPlatforms = Object.entries(nonPrimaryPlatforms).filter(entery => entery[1].hrefCLI !== undefined) 

  downloadBtn.nextElementSibling.querySelectorAll("a").forEach((nonPrimaryLink,i) =>{
    nonPrimaryLink.href = nonPrimaryPlatforms[i][1].hrefCLI;
    nonPrimaryLink.textContent = nonPrimaryPlatforms[i][0] === "macOS/iOS" ? "macOS": nonPrimaryPlatforms[i][0];
  })
});

/** gui download center navigation **/
let header = document.querySelector("#gui-download-center h2");
let subHeader = document.querySelector("#gui-download-center .tagline")
let primaryDownload = document.querySelector("#gui-download-center .link")

let downloadNavLinks = document.querySelectorAll("#available-platforms-nav button")
function updateDownloadCenterInfo(activeLink){
    let activePlatform = platforms[activeLink.dataset.platform];

    downloadNavLinks.forEach(link => link.classList.remove("active"))
    activeLink.classList.add("active")
    primaryDownload.textContent = `Get for ${activeLink.dataset.platform}`
    primaryDownload.href = activePlatform.hrefGUI
    header.textContent = activePlatform.header;
    subHeader.textContent = activePlatform.subHeader;
}

downloadNavLinks.forEach(dLink =>{
  if (dLink.dataset.platform === userSecondaryPlatform){
    updateDownloadCenterInfo(dLink)
  }
  dLink.addEventListener("click", () =>{
    updateDownloadCenterInfo(dLink)
  })
})

