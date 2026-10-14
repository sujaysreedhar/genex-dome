; Compile through scripts/package.ps1 so version and paths match the staged package.
#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef PackageDir
  #error PackageDir is required
#endif
#ifndef OutputDir
  #error OutputDir is required
#endif

[Setup]
AppId={{C1A9D85B-851D-40B9-B85E-39F14839AFD9}
AppName=GenexDome
AppVersion={#AppVersion}
AppPublisher=GenexDome contributors
AppPublisherURL=https://github.com/sujaysreedhar/genex-dome
AppSupportURL=https://github.com/sujaysreedhar/genex-dome/issues
DefaultDirName={localappdata}\Programs\GenexDome
DefaultGroupName=GenexDome
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.19045
DisableProgramGroupPage=yes
AllowNoIcons=yes
OutputDir={#OutputDir}
OutputBaseFilename=GenexDome-{#AppVersion}-windows-x64-setup
LicenseFile={#PackageDir}\LICENSE
InfoAfterFile={#PackageDir}\QUICK-START.txt
UninstallDisplayIcon={app}\GenexDomPlayer.exe
VersionInfoVersion={#AppVersion}.0
VersionInfoDescription=GenexDome Windows installer
Compression=lzma2/fast
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{#PackageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\GenexDome"; Filename: "{app}\GenexDomPlayer.exe"; WorkingDir: "{app}"
Name: "{group}\GenexDome demo show"; Filename: "{app}\GenexDomPlayer.exe"; Parameters: "--session ""{app}\assets\show.genexdomplayer.json"" --panel show --autoplay"; WorkingDir: "{app}"
Name: "{group}\Quick start"; Filename: "{app}\QUICK-START.txt"
Name: "{group}\Uninstall GenexDome"; Filename: "{uninstallexe}"
Name: "{autodesktop}\GenexDome"; Filename: "{app}\GenexDomPlayer.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\GenexDomPlayer.exe"; Description: "Launch GenexDome"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent

[InstallDelete]
; Retire the download helper from the earlier package when upgrading.
Type: files; Name: "{app}\Install-media-tools.cmd"
Type: files; Name: "{app}\scripts\install-media.ps1"
Type: files; Name: "{group}\Install media tools.lnk"

; The uninstall log tracks installed files. Never recursively delete {app}:
; user-created sessions, media and separately downloaded tools must be preserved.
