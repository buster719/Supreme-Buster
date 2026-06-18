Set shell = CreateObject("WScript.Shell")

cmd = "powershell.exe"
For i = 0 To WScript.Arguments.Count - 1
  cmd = cmd & " " & QuoteArg(WScript.Arguments(i))
Next

shell.Run cmd, 0, False

Function QuoteArg(value)
  QuoteArg = """" & Replace(value, """", "\""") & """"
End Function
