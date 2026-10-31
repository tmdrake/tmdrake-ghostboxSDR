' Double-click this file to open Drake's Box SDR.
' It skips Command Prompt AutoRun, which swallows run.cmd.
Option Explicit

Dim sh, fso, root, exe, cmd, extra, i, rc
Set sh = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")
root = fso.GetParentFolderName(WScript.ScriptFullName)
exe = root & "\target\release\shackhack.exe"
cmd = root & "\run.cmd"

extra = ""
For i = 0 To WScript.Arguments.Count - 1
  extra = extra & " """ & WScript.Arguments(i) & """"
Next

If Not fso.FileExists(exe) Then
  rc = sh.Run("cmd.exe /d /c """ & cmd & """" & extra, 1, True)
  If rc <> 0 Then
    MsgBox "Drake's Box SDR did not build.", 16, "Drake's Box SDR"
    WScript.Quit rc
  End If
End If

If fso.FileExists(exe) Then
  sh.CurrentDirectory = root
  sh.Run """" & exe & """" & extra, 1, False
Else
  MsgBox "shackhack.exe is still missing.", 16, "Drake's Box SDR"
End If
