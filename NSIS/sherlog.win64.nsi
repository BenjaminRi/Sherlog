#-------------------------------------------------------------------------------
# Use Unicode. Installer will not work on Windows 95/98/ME.
Unicode true

; https://gist.github.com/drewchapin/246de6d0c404a79ee66a5ead35b480bc
;-------------------------------------------------------------------------------
; Includes
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WinVer.nsh"
!include "x64.nsh"

;-------------------------------------------------------------------------------
; Metadata configuration

!define COMPILED_BIN_DIR "..\target\release\sherlog.exe"
!getdllversion "${COMPILED_BIN_DIR}" V_
!define COMPILED_BIN_VERSION "${V_1}.${V_2}.${V_3}.${V_4}"
!define COMPILED_BIN_RELEASE "${V_1}.${V_2}.${V_3}"

!define PRODUCT_NAME "Sherlog"
!define PRODUCT_DESCRIPTION "Log viewer and analysis tool"
!define COPYRIGHT "Copyright © 2020 Benjamin Richner"
!define PRODUCT_VERSION "${COMPILED_BIN_VERSION}"
!define FILE_VERSION "${COMPILED_BIN_VERSION}"

;-------------------------------------------------------------------------------
; Attributes
Name "Sherlog"
OutFile "Sherlog-Setup-v${COMPILED_BIN_RELEASE}.dev.exe"
InstallDir "$PROGRAMFILES64\Sherlog"
RequestExecutionLevel admin

;-------------------------------------------------------------------------------
; Version Info
VIProductVersion "${PRODUCT_VERSION}"
VIFileVersion "${FILE_VERSION}"
VIAddVersionKey "ProductName" "${PRODUCT_NAME}"
VIAddVersionKey "ProductVersion" "${PRODUCT_VERSION}"
VIAddVersionKey "FileDescription" "${PRODUCT_DESCRIPTION}"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${FILE_VERSION}"

;-------------------------------------------------------------------------------
; Modern UI Appearance
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\orange-install.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "${NSISDIR}\Contrib\Graphics\Header\orange.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${NSISDIR}\Contrib\Graphics\Wizard\orange.bmp"
!define MUI_FINISHPAGE_NOAUTOCLOSE

;-------------------------------------------------------------------------------
; Installer Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "EULA.txt"
;!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

;-------------------------------------------------------------------------------
; Uninstaller Pages
!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

;-------------------------------------------------------------------------------
; Languages
!insertmacro MUI_LANGUAGE "English"

;-------------------------------------------------------------------------------

!macro DIRECTORIES Operation PathPrefix
${Operation} "${PathPrefix}\bin\"
${Operation} "${PathPrefix}\share\icons\Adwaita\"
${Operation} "${PathPrefix}\share\icons\hicolor\"
${Operation} "${PathPrefix}\share\icons\"
${Operation} "${PathPrefix}\share\"
!macroend

!macro FILES Operation PathPrefix
SetOutPath "$INSTDIR\bin"
${Operation} "${PathPrefix}\bin\gdbus.exe"
${Operation} "${PathPrefix}\bin\libLerc.dll"
${Operation} "${PathPrefix}\bin\libbrotlicommon.dll"
${Operation} "${PathPrefix}\bin\libbrotlidec.dll"
${Operation} "${PathPrefix}\bin\libcairo-2.dll"
${Operation} "${PathPrefix}\bin\libcairo-script-interpreter-2.dll"
${Operation} "${PathPrefix}\bin\libdatrie-1.dll"
${Operation} "${PathPrefix}\bin\libdeflate.dll"
${Operation} "${PathPrefix}\bin\libffi-7.dll"
${Operation} "${PathPrefix}\bin\libfontconfig-1.dll"
${Operation} "${PathPrefix}\bin\libfreetype-6.dll"
${Operation} "${PathPrefix}\bin\libfribidi-0.dll"
${Operation} "${PathPrefix}\bin\libgio-2.0-0.dll"
${Operation} "${PathPrefix}\bin\libglib-2.0-0.dll"
${Operation} "${PathPrefix}\bin\libgmodule-2.0-0.dll"
${Operation} "${PathPrefix}\bin\libgobject-2.0-0.dll"
${Operation} "${PathPrefix}\bin\libgraphene-1.0-0.dll"
${Operation} "${PathPrefix}\bin\libgraphite2.dll"
${Operation} "${PathPrefix}\bin\libgtk-4-1.dll"
${Operation} "${PathPrefix}\bin\libharfbuzz-0.dll"
${Operation} "${PathPrefix}\bin\libiconv-2.dll"
${Operation} "${PathPrefix}\bin\libintl-8.dll"
${Operation} "${PathPrefix}\bin\libjbig-0.dll"
${Operation} "${PathPrefix}\bin\liblzma-5.dll"
${Operation} "${PathPrefix}\bin\libpango-1.0-0.dll"
${Operation} "${PathPrefix}\bin\libpangocairo-1.0-0.dll"
${Operation} "${PathPrefix}\bin\libpangowin32-1.0-0.dll"
${Operation} "${PathPrefix}\bin\libstdc++-6.dll"
${Operation} "${PathPrefix}\bin\libthai-0.dll"
${Operation} "${PathPrefix}\bin\libtiff-5.dll"
${Operation} "${PathPrefix}\bin\libwebp-7.dll"
${Operation} "${PathPrefix}\bin\libzstd.dll"
SetOutPath "$INSTDIR"
!macroend

;-------------------------------------------------------------------------------
; Installer Sections
Section "Sherlog" Sherlog
	SetOutPath $INSTDIR

	IfFileExists "$INSTDIR\Uninstall.exe" 0 done_uninstall
	DetailPrint "Uninstall previous version..."
	SetDetailsPrint none
	ExecWait "$INSTDIR\Uninstall.exe /S /KEEPSETT _?=$INSTDIR"
	SetDetailsPrint lastused
	done_uninstall:

	WriteUninstaller "$INSTDIR\Uninstall.exe"

	!insertmacro DIRECTORIES CreateDirectory "$INSTDIR"
	!insertmacro FILES File "C:\msys64\mingw64"
	DetailPrint "Extract files..."
	SetDetailsPrint textonly
	SetOutPath "$INSTDIR\share\icons\Adwaita\"
	# File /r "C:\msys64\mingw64\share\icons\Adwaita\"
	SetOutPath "$INSTDIR\share\icons\hicolor\"
	# File /r "C:\msys64\mingw64\share\icons\hicolor\"
	SetDetailsPrint lastused
	SetOutPath "$INSTDIR\bin"
	File "${COMPILED_BIN_DIR}"
	CreateShortCut "$DESKTOP\Sherlog.lnk" "$INSTDIR\bin\sherlog.exe"

SectionEnd

;-------------------------------------------------------------------------------
; Uninstaller Sections
Section "Uninstall"
	ClearErrors
	Var /GLOBAL tmp
	${GetOptions} $CMDLINE "/KEEPSETT" $tmp
	IfErrors delete_sett keep_sett
	delete_sett:
	# Delete user settings (default if flag not found)
	# MessageBox MB_OK "Not found (delete settings by default)"
	# TODO: Implement once we actually have settings
	ClearErrors
	goto done_sett
	keep_sett:
	# Keep user settings (explicitly flagged to keep settings)
	# MessageBox MB_OK "Found (keep settings)"
	done_sett:
	
	Delete "$DESKTOP\Sherlog.lnk"
	Delete "$INSTDIR\bin\sherlog.exe"
	DetailPrint "Delete files..."
	SetDetailsPrint textonly
	RMDir /r "$INSTDIR\share\icons\hicolor\"
	RMDir /r "$INSTDIR\share\icons\Adwaita\"
	SetDetailsPrint lastused
	!insertmacro FILES Delete "$INSTDIR"
	!insertmacro DIRECTORIES RMDir "$INSTDIR"
	
	Delete "$INSTDIR\Uninstall.exe"
	
	SetOutPath "$DESKTOP" ; free outpath so $INSTDIR can be deleted
	RMDir "$INSTDIR"
SectionEnd
