[Setup]
AppId={{C057580D-6DF9-4EB8-9333-B8C49E405A50}
AppName=scrivi
AppVersion={#AppVersion}
AppPublisher=scrivi
DefaultDirName={localappdata}\Programs\scrivi
DefaultGroupName=scrivi
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputPath}
OutputBaseFilename=scrivi-v{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\scrivi.ico
CloseApplications=yes
RestartApplications=no
SetupLogging=yes
SetupIconFile={#IconPath}

[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut"

[Files]
Source: "{#ExecutablePath}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#IconPath}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\scrivi"; Filename: "{app}\scrivi.exe"; IconFilename: "{app}\scrivi.ico"
Name: "{autodesktop}\scrivi"; Filename: "{app}\scrivi.exe"; IconFilename: "{app}\scrivi.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\scrivi.exe"; Description: "Launch scrivi"; Flags: nowait postinstall skipifsilent
Filename: "{app}\scrivi.exe"; Flags: nowait skipifnotsilent; Check: RestartScrivi

[Code]
function RestartScrivi: Boolean;
begin
  Result := ExpandConstant('{param:RESTARTSCRIVI|0}') = '1';
end;
