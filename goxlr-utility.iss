; GoXLR Utility Installation Script

[Setup]
AppName=GoXLR Utility
AppVersion=0.10.2
WizardStyle=modern
DefaultDirName={autopf}\GoXLR Utility
DefaultGroupName=GoXLR Utility
UninstallDisplayIcon={app}\goxlr-daemon.exe
Compression=bzip
SolidCompression=no
LicenseFile=LICENSE
OutputBaseFilename=goxlr-utility-setup
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
SetupIconFile=daemon/resources/goxlr-utility.ico
CloseApplications=force
AppPublisher=The GoXLR on Linux Team
AppPublisherURL=http://github.com/GoXLR-on-Linux

[Files]
Source: "target\release\goxlr-daemon.exe";    DestDir: "{app}";       DestName: "goxlr-daemon.exe"
Source: "target\release\goxlr-client.exe";    DestDir: "{app}";       DestName: "goxlr-client.exe"
Source: "target\release\goxlr-defaults.exe";  DestDir: "{app}";       DestName: "goxlr-defaults.exe"
Source: "target\release\goxlr-launcher.exe";  DestDir: "{app}";       DestName: "goxlr-launcher.exe"
Source: "LICENSE";                            DestDir: "{app}";       DestName: "LICENSE"
Source: "LICENSE-3RD-PARTY";                  DestDir: "{app}";       DestName: "LICENSE-3RD-PARTY"

[Tasks]
Name: StartOnLogin; Description: Automatically start the GoXLR Utility on Login

[Icons]
Name: "{group}\GoXLR Utility"; Filename: "{app}\goxlr-launcher.exe";
Name: "{userstartup}\GoXLR Utility"; Filename: "{app}\goxlr-daemon.exe"; Tasks: StartOnLogin

[Run]
Filename: "{app}\goxlr-launcher.exe"; Description: "Run the GoXLR Utility"; Flags: shellexec skipifsilent nowait postinstall;

[UninstallRun]
Filename: "taskkill"; Parameters: "/im ""goxlr-daemon.exe"" /f"; Flags: runhidden; RunOnceId: Uninstaller

[Code]
// Check to see if the GoXLR API is available before installing..
function InitializeSetup(): Boolean;
begin
    if (FileExists('C:/Program Files/TC-HELICON/GoXLR_Audio_Driver/W10_x64/goxlr_audioapi_x64.dll')) then
    begin
        Result := True
    end
    else
    begin
        MsgBox('Unable to locate the GoXLR Driver, please ensure it is installed to the default location.', mbCriticalError, MB_OK);
        Result := False
    end
end;

// Display two license pages
// From: https://stackoverflow.com/questions/34592002/how-to-create-two-licensefile-pages-in-inno-setup
var
  SecondLicensePage: TOutputMsgMemoWizardPage;
  License2AcceptedRadio: TRadioButton;
  License2NotAcceptedRadio: TRadioButton;

procedure CheckLicense2Accepted(Sender: TObject);
begin
  // Update Next button when user (un)accepts the license
  WizardForm.NextButton.Enabled := License2AcceptedRadio.Checked;
end;

function CloneLicenseRadioButton(Source: TRadioButton): TRadioButton;
begin
  Result := TRadioButton.Create(WizardForm);
  Result.Parent := SecondLicensePage.Surface;
  Result.Caption := Source.Caption;
  Result.Left := Source.Left;
  Result.Top := Source.Top;
  Result.Width := Source.Width;
  Result.Height := Source.Height;
  // Needed for WizardStyle=modern / WizardResizable=yes
  Result.Anchors := Source.Anchors;
  Result.OnClick := @CheckLicense2Accepted;
end;

procedure InitializeWizard();
var
  LicenseFileName: string;
  LicenseFilePath: string;
begin
  // Create second license page, with the same labels as the original license page
  SecondLicensePage :=
    CreateOutputMsgMemoPage(
      wpLicense, SetupMessage(msgWizardLicense), SetupMessage(msgLicenseLabel),
      SetupMessage(msgLicenseLabel3), '');

  // Shrink license box to make space for radio buttons
  SecondLicensePage.RichEditViewer.Height := WizardForm.LicenseMemo.Height;

  // Load license
  // Loading ex-post, as Lines.LoadFromFile supports UTF-8,
  // contrary to LoadStringFromFile.
  LicenseFileName := 'LICENSE-3RD-PARTY';
  ExtractTemporaryFile(LicenseFileName);
  LicenseFilePath := ExpandConstant('{tmp}\' + LicenseFileName);
  SecondLicensePage.RichEditViewer.Lines.LoadFromFile(LicenseFilePath);
  DeleteFile(LicenseFilePath);

  // Clone accept/do not accept radio buttons for the second license
  License2AcceptedRadio :=
    CloneLicenseRadioButton(WizardForm.LicenseAcceptedRadio);
  License2NotAcceptedRadio :=
    CloneLicenseRadioButton(WizardForm.LicenseNotAcceptedRadio);

  // Initially not accepted
  License2NotAcceptedRadio.Checked := True;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  // Update Next button when user gets to second license page
  if CurPageID = SecondLicensePage.ID then
  begin
    CheckLicense2Accepted(nil);
  end;
end;
