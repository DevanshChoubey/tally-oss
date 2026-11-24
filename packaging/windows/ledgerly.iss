; Inno Setup script for the Windows installer. Built by packaging/windows/package.ps1, which passes
; AppVersion and SourceDir on the iscc command line. Clean room: no third-party product names.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\dist\stage"
#endif

[Setup]
AppId={{B3E1C6A4-7D2F-4E19-8A5C-0F6D9E2B7C41}
AppName=Ledgerly
AppVersion={#AppVersion}
AppPublisher=Ledgerly contributors
DefaultDirName={autopf}\Ledgerly
DefaultGroupName=Ledgerly
DisableProgramGroupPage=yes
OutputDir=..\..\dist
OutputBaseFilename=ledgerly-{#AppVersion}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
LicenseFile={#SourceDir}\LICENSE-AGPL
UninstallDisplayName=Ledgerly
UninstallDisplayIcon={app}\ledgerly-app.exe

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{#SourceDir}\ledgerly-app.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\ledgerly.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-*"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\NOTICE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\FONTS-*"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Ledgerly"; Filename: "{app}\ledgerly-app.exe"
Name: "{autodesktop}\Ledgerly"; Filename: "{app}\ledgerly-app.exe"; Tasks: desktopicon
