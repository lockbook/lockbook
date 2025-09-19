{ pkgs ? import <nixpkgs> { } }:

let
  buildToolsVersion = "35.0.0";
  androidComposition = pkgs.androidenv.composeAndroidPackages {
    cmdLineToolsVersion = "8.0";
    toolsVersion = "26.1.1";
    platformToolsVersion = "35.0.2";
    buildToolsVersions = [ buildToolsVersion ];
    includeEmulator = false;
    emulatorVersion = "30.3.4";
    platformVersions = [ "35" "36" ];
    includeSources = false;
    includeSystemImages = false;
    systemImageTypes = [ "google_apis_playstore" ];
    abiVersions = [ "armeabi-v7a" "arm64-v8a" ];
    cmakeVersions = [ "3.10.2" ];
    includeNDK = true;
    # ndkVersions = [ "22.0.7026061" ];
    useGoogleAPIs = false;
    useGoogleTVAddOns = false;
    includeExtras = [
      "extras;google;gcm"
    ];
  };
in
pkgs.mkShell rec {
  buildInputs = [
    pkgs.openjdk17
  ];

  ANDROID_HOME = "${androidComposition.androidsdk}/libexec/android-sdk";
  ANDROID_NDK_ROOT = "${ANDROID_HOME}/ndk-bundle";

  # Use the same buildToolsVersion here
  GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${ANDROID_HOME}/build-tools/${buildToolsVersion}/aapt2";

  shellHook = ''
    export ANDROID_SDK_ROOT="${androidComposition.androidsdk}/libexec/android-sdk";
    export JAVA_HOME="${pkgs.openjdk17}/lib/openjdk";
  '';
}
