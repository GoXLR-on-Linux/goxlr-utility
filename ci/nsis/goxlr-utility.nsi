Unicode True


; Before we start, lets define some variables..
!define /ifndef PRODUCT_VERSION "0.0.0"
!define PRODUCT_NAME "GoXLR Utility"
!define PRODUCT_PUBLISHER "The GoXLR on Linux Team"
!define PRODUCT_WEBSITE "https://github.com/goxlr-on-linux/goxlr-utility/"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"
!define PRODUCT_REGKEY "Software\GoXLR Utility"

; Basic Modern User Interface 2 Setup..
!include "MUI2.nsh"
!include "InstallOptions.nsh"
!include "WinMessages.nsh"
!include "x64.nsh"

!define MUI_ABORTWARNING
!define MUI_ICON "../../daemon/resources/goxlr-utility.ico"
!define MUI_UNICON "../../daemon/resources/goxlr-utility.ico"

; Perform the Initial Stuff..
!define MUI_CUSTOMFUNCTION_GUIINIT DoInit

; Ok, time to start with pages and stuff, firstly, welcome!
!insertmacro MUI_PAGE_WELCOME

; Display both the available licenses..
!define MUI_LICENSEPAGE_RADIOBUTTONS
!insertmacro MUI_PAGE_LICENSE "../../LICENSE"

!define MUI_LICENSEPAGE_RADIOBUTTONS
!insertmacro MUI_PAGE_LICENSE "../../LICENSE-3RD-PARTY"

; Ask where the installer is going to go..
!define MUI_PAGE_CUSTOMFUNCTION_PRE CheckDirectory
!insertmacro MUI_PAGE_DIRECTORY

;
Var StartMenuFolder ;Start menu folder
!define MUI_PAGE_CUSTOMFUNCTION_PRE CheckStartMenu
!define MUI_STARTMENUPAGE_NODISABLE
!define MUI_STARTMENUPAGE_DEFAULTFOLDER "${PRODUCT_NAME}"
!insertmacro MUI_PAGE_STARTMENU 0 $StartMenuFolder

; Ask for any 'Extras' after we're finished..
Page custom PerformActions PerformActionsLeave

; The next step is handling if the GoXLR Utility is already running..
Page custom IsUtilRunning

; Run the Install!
!insertmacro MUI_PAGE_INSTFILES

!define MUI_FINISHPAGE_RUN "$INSTDIR\goxlr-launcher.exe"
!insertmacro MUI_PAGE_FINISH

; Uninstaller pages
!insertmacro MUI_UNPAGE_INSTFILES

!system 'mkdir "../Output/"'

; -- UI End
Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "../Output/goxlr-utility-${PRODUCT_VERSION}.exe"
InstallDir "$PROGRAMFILES64\GoXLR Utility"
RequestExecutionLevel admin
ShowInstDetails show
ShowUnInstDetails show

!insertmacro MUI_LANGUAGE "English"

Function DoInit
${If} ${RunningX64}
${Else}
    MessageBox MB_OK|MB_ICONSTOP  "The GoXLR Utility is only available on 64bit Systems"
    Abort
${EndIf}


Call GetRegKeys

; Firstly, see if we can find the path in the registry..
ReadRegStr $0 HKCR64 "CLSID\{024D0372-641F-4B7B-8140-F4DFE458C982}\InprocServer32\" ""
${If} ${Errors}
    Goto DEFAULT
${EndIf}

; Check whether the registry path exists, otherwise try default.
IfFileExists $0 END DEFAULT

; Look for the file in the Default path..
DEFAULT:
    StrCpy $0 "C:\Program Files\TC-HELICON\GoXLR_Audio_Driver\W10_x64\goxlr_audioapi_x64.dll"
    IfFileExists $0 END ERROR

ERROR:
    MessageBox MB_OK|MB_ICONSTOP  "Unable to locate the GoXLR Driver, please ensure it is installed."
    Abort

END:
FunctionEnd

var KeyTest
var StartMenuPath
var StartMenuPathSet

var InstallDir
var InstallDirSet

var AutoStartRegSet
var AutoStartReg

var UseAppRegSet
var UseAppReg

!macro GetRegKeys un
    Function ${un}GetRegKeys
        ReadRegStr $KeyTest HKLM64 "${PRODUCT_REGKEY}" "InstallPath"
        ${If} ${Errors}
            StrCpy $InstallDirSet 0
        ${Else}
            StrCpy $InstallDirSet 1
            StrCpy $InstallDir $KeyTest
        ${EndIf}

        ReadRegStr $KeyTest HKLM64 "${PRODUCT_REGKEY}" "StartMenu"
        ${If} ${Errors}
            StrCpy $StartMenuPathSet 0
        ${Else}
            StrCpy $StartMenuPathSet 1
            StrCpy $StartMenuPath $KeyTest
        ${EndIf}

        ReadRegStr $KeyTest HKLM64 "${PRODUCT_REGKEY}" "AutoStart"
        ${If} ${Errors}
            StrCpy $AutoStartRegSet 0
        ${Else}
            StrCpy $AutoStartRegSet 1
            StrCpy $AutoStartReg $KeyTest
        ${EndIf}

        ReadRegStr $KeyTest HKLM64 "${PRODUCT_REGKEY}" "UseApp"
        ${If} ${Errors}
            StrCpy $UseAppRegSet 0
        ${Else}
            StrCpy $UseAppRegSet 1
            StrCpy $UseAppReg $KeyTest
        ${EndIf}
    FunctionEnd
!macroend
!insertmacro GetRegKeys ""
!insertmacro GetRegKeys "un."

Function CheckDirectory
    StrCmp $InstallDirSet 1 0 END
        StrCpy $INSTDIR $InstallDir
        Abort
    END:
FunctionEnd

Function CheckStartMenu
    StrCmp $StartMenuPathSet 1 0 END
        StrCpy $StartMenuFolder $StartMenuPath
        Abort
    END:
FunctionEnd

Function PerformActions
    ReserveFile "post-install.ini"
    !insertmacro MUI_HEADER_TEXT "Select Additional Tasks" "Which additional tasks should be performed?"

    !insertmacro INSTALLOPTIONS_EXTRACT "post-install.ini"

    ; Set any cached values..
    AUTO_START:
        StrCmp $AutoStartRegSet 1 0 USE_APP
        !insertmacro INSTALLOPTIONS_WRITE "post-install.ini" "Field 2" "State" $AutoStartReg
    USE_APP:
        StrCmp $UseAppRegSet 1 0 END
        !insertmacro INSTALLOPTIONS_WRITE "post-install.ini" "Field 3" "State" $UseAppReg

    END:
    !insertmacro INSTALLOPTIONS_DISPLAY "post-install.ini"
FunctionEnd

Function PerformActionsLeave
    var /GLOBAL AUTO_START
    var /GLOBAL USE_APP
    !insertmacro INSTALLOPTIONS_READ $AUTO_START "post-install.ini" "Field 2" "State"
    !insertmacro INSTALLOPTIONS_READ $USE_APP "post-install.ini" "Field 3" "State"
FunctionEnd

Function IsUtilRunning
; The util spawns a window we can look for..
FindWindow $0 "goxlr-utility"
StrCmp $0 0 STOP
    ReserveFile "running-warn.ini"
    !insertmacro MUI_HEADER_TEXT "Preparing to Install" "Setup is preparing to install ${PRODUCT_NAME} on your computer."

    !insertmacro INSTALLOPTIONS_EXTRACT "running-warn.ini"
    !insertmacro INSTALLOPTIONS_DISPLAY "running-warn.ini"
    Goto END

STOP:
    Abort
    Goto END

END:
FunctionEnd

Var count
!macro StopUtility un
Function ${un}StopUtility
DetailPrint "Checking for GoXLR Utility.."

FindWindow $0 "goxlr-utility"
StrCmp $0 0 END
DetailPrint "GoXLR Utility Found, attempting to stop.."

; Util is running, send it a WM_CLOSE signal..
SendMessage $0 ${WM_CLOSE} 0 0
StrCpy $count 0

LOOP:
FindWindow $0 "goxlr-utility"
StrCmp $0 0 ENDSLEEP
Sleep 100
IntOp $count $count + 1
StrCmp $count 50 0 LOOP

DetailPrint "Graceful Stop failed, forcing Shutdown"
; If we get here, the Util hasn't closed after 5 seconds..
nsExec::Exec "TaskKill /F /IM goxlr-daemon.exe"

ENDSLEEP:
Sleep 500

END:
FunctionEnd
!macroend

!insertmacro StopUtility ""
!insertmacro StopUtility "un."

Function CleanOldInstaller
    ; Basically, if the util has previously been installed by the InnoSetup binary, call a silent
    ; uninstall on it to remove the various traces of it.
    ReadRegStr $0 HKLM64 "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\GoXLR Utility_is1\" "QuietUninstallString"
    ${If} ${Errors}
        Goto END
    ${EndIf}
    DetailPrint "Uninstalling Previous InnoSetup install.."
    Exec $0

    ; Wait for the uninstall key to go away..
    Loop:
    ReadRegStr $0 HKLM64 "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\GoXLR Utility_is1\" "QuietUninstallString"
    ${If} ${Errors}
        Goto ENDSLEEP
    ${EndIf}
    Sleep 100
    Goto Loop

    ENDSLEEP:
    ; Just in case..
    Sleep 2000
    END:
FunctionEnd

Section "MainSection" SEC01
    Call StopUtility
    Call CleanOldInstaller

    ; This is the main installer section..
    SetOutPath "$INSTDIR"

    ; Ok, here come the files..
    File "..\..\target\release\goxlr-daemon.exe"
    File "..\..\target\release\goxlr-client.exe"
    File "..\..\target\release\goxlr-defaults.exe"
    File "..\..\target\release\goxlr-launcher.exe"
    File "..\..\target\release\goxlr-firmware.exe"
    File "..\..\target\release\goxlr-utility-ui.exe"
    File "..\..\target\release\SAAPI64.dll"
    File "..\..\target\release\nvdaControllerClient64.dll"
    File "..\..\LICENSE"
    File "..\..\LICENSE-3RD-PARTY"

    SetShellVarContext all
    CreateDirectory "$SMPROGRAMS\$StartMenuFolder"
    CreateShortCut "$SMPROGRAMS\$StartMenuFolder\GoXLR Utility.lnk" "$INSTDIR\goxlr-launcher.exe"

    StrCmp $AUTO_START 0 AUTO_START_OFF
        ; Switch to Current User..
        SetShellVarContext current
        CreateShortCut "$SMPROGRAMS\Startup\GoXLR Utility.lnk" "$INSTDIR\goxlr-daemon.exe"
        Goto POST_AUTO_START

    AUTO_START_OFF:
        ; Switch to Current User..
        SetShellVarContext current
        Delete "$SMPROGRAMS\Startup\GoXLR Utility.lnk"

    POST_AUTO_START:
        StrCmp $USE_APP 0 REMOVE_APP
        nsExec::Exec "$INSTDIR\goxlr-utility-ui.exe --install"
        Goto POST_OPTION

    REMOVE_APP:
        nsExec::Exec "$INSTDIR\goxlr-utility-ui.exe --remove"

    POST_OPTION:
SectionEnd

Section -Post
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM64 "${PRODUCT_UNINST_KEY}" "DisplayName" "$(^Name)"
  WriteRegStr HKLM64 "${PRODUCT_UNINST_KEY}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM64 "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR\goxlr-daemon.exe"
  WriteRegStr HKLM64 "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr HKLM64 "${PRODUCT_UNINST_KEY}" "URLInfoAbout" "${PRODUCT_WEBSITE}"
  WriteRegStr HKLM64 "${PRODUCT_UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"

  WriteRegStr HKLM64 "${PRODUCT_REGKEY}" "InstallPath" "$INSTDIR"
  WriteRegStr HKLM64 "${PRODUCT_REGKEY}" "StartMenu" "$StartMenuFolder"
  WriteRegStr HKLM64 "${PRODUCT_REGKEY}" "UseApp" "$USE_APP"
  WriteRegStr HKLM64 "${PRODUCT_REGKEY}" "AutoStart" "$AUTO_START"
SectionEnd

Function un.onUninstSuccess
  HideWindow
  MessageBox MB_ICONINFORMATION|MB_OK "$(^Name) was successfully removed from your computer."
FunctionEnd

Function un.onInit
  MessageBox MB_ICONQUESTION|MB_YESNO|MB_DEFBUTTON2 "Are you sure you want to completely remove $(^Name) and all of its components?" IDYES +2
  Abort
FunctionEnd

Section Uninstall
  Call un.StopUtility
  Call un.GetRegKeys

  ; Nuke the directory, and everything in it.
  RMDir /r $InstallDir

  SetShellVarContext all
  RMDir /r "$SMPROGRAMS\$StartMenuPath"

  SetShellVarContext current
  Delete "$SMPROGRAMS\Startup\GoXLR Utility.lnk"

  DeleteRegKey HKLM64 "${PRODUCT_UNINST_KEY}"
  DeleteRegKey HKLM64 "${PRODUCT_REGKEY}"
  SetAutoClose true
SectionEnd
