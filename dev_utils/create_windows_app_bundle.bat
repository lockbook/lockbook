call create_core_for_windows.bat
cd ..\clients\windows
msbuild windows.sln /p:Platform=x64;PlatformTarget=x64;Configuration=Release;AppxBundle=Always;AppxBundlePlatforms=x64
explorer bin