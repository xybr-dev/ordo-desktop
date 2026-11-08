# Drives one running ordo.exe for `cargo xtask recovery-drill`.
#
# Usage: recovery_drill.ps1 displays
#        recovery_drill.ps1 invoke <process id> <button name>
#        recovery_drill.ps1 keys <process id> <SendKeys text, or "space">
#
# `displays` prints the number of attached monitors. `invoke` presses a named
# button through UI Automation (the app publishes its controls to the
# accessibility tree), so it needs neither focus nor pixel coordinates. `keys`
# brings the process's window to the foreground and types into it with
# SendKeys, which is the real keyboard path. Exit code 0 is success; anything
# else is a failure whose reason is on stderr.
param(
    [Parameter(Mandatory = $true)][string]$Action,
    [int]$ProcessId = 0,
    [string]$Argument = ''
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class OrdoDrillNative {
    delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr lParam);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int max);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
    [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);

    // Visible top-level windows owned by `pid`.
    public static IntPtr[] WindowsOf(uint pid) {
        var found = new List<IntPtr>();
        EnumWindows((hWnd, lParam) => {
            uint owner;
            GetWindowThreadProcessId(hWnd, out owner);
            if (owner == pid && IsWindowVisible(hWnd)) found.Add(hWnd);
            return true;
        }, IntPtr.Zero);
        return found.ToArray();
    }

    public static string Title(IntPtr hWnd) {
        var text = new StringBuilder(256);
        GetWindowText(hWnd, text, text.Capacity);
        return text.ToString();
    }

    // A tap of Alt lets this process take the foreground from the caller.
    public static void AltTap() {
        keybd_event(0x12, 0, 0, UIntPtr.Zero);
        keybd_event(0x12, 0, 2, UIntPtr.Zero);
    }
}
'@

function Fail([string]$message, [int]$code = 1) {
    [Console]::Error.WriteLine($message)
    exit $code
}

# The operator window is the visible window of the process that publishes
# controls to the accessibility tree. The congregation window (opened by
# Enable Output, in the same process) publishes none and takes no keys.
function Get-OperatorWindow([int]$id) {
    $type = [System.Windows.Automation.ControlType]::Button
    $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, $type)
    foreach ($hwnd in [OrdoDrillNative]::WindowsOf([uint32]$id)) {
        try {
            $root = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
            if ($null -ne $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)) { return $hwnd }
        } catch { }
    }
    return [IntPtr]::Zero
}

function Wait-Operator([int]$id, [int]$seconds) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if ($null -eq (Get-Process -Id $id -ErrorAction SilentlyContinue)) { Fail "process $id is gone" 2 }
        $hwnd = Get-OperatorWindow $id
        if ($hwnd -ne [IntPtr]::Zero) { return $hwnd }
        Start-Sleep -Milliseconds 200
    }
    Fail "process $id showed no operator window within $seconds s" 3
}

function Describe-Windows([int]$id) {
    $foreground = [OrdoDrillNative]::GetForegroundWindow()
    $lines = foreach ($hwnd in [OrdoDrillNative]::WindowsOf([uint32]$id)) {
        "hwnd=$hwnd title='$([OrdoDrillNative]::Title($hwnd))' foreground=$($hwnd -eq $foreground)"
    }
    "windows of $id (foreground $foreground): " + ($lines -join '; ')
}

switch ($Action) {
    'displays' {
        [System.Windows.Forms.Screen]::AllScreens.Count
    }
    'invoke' {
        $name = [System.Windows.Automation.AutomationElement]::NameProperty
        $deadline = (Get-Date).AddSeconds(50)
        while ((Get-Date) -lt $deadline) {
            try {
                $hwnd = Get-OperatorWindow $ProcessId
                if ($hwnd -ne [IntPtr]::Zero) {
                    $root = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
                    $condition = New-Object System.Windows.Automation.PropertyCondition($name, $Argument)
                    $button = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
                    if ($null -ne $button -and $button.Current.IsEnabled) {
                        $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                        exit 0
                    }
                }
            } catch { }
            Start-Sleep -Milliseconds 250
        }
        Fail "no enabled button named '$Argument' within 50 s" 4
    }
    'keys' {
        $hwnd = Wait-Operator $ProcessId 30
        $deadline = (Get-Date).AddSeconds(10)
        while ((Get-Date) -lt $deadline) {
            [OrdoDrillNative]::AltTap()
            [void][OrdoDrillNative]::ShowWindow($hwnd, 9)
            [void][OrdoDrillNative]::SetForegroundWindow($hwnd)
            Start-Sleep -Milliseconds 300
            # Keys go out only when the operator window itself is foreground.
            if ([OrdoDrillNative]::GetForegroundWindow() -eq $hwnd) {
                $text = if ($Argument -eq 'space') { ' ' } else { $Argument }
                [System.Windows.Forms.SendKeys]::SendWait($text)
                if ($env:ORDO_DRILL_TRACE) { [Console]::Error.WriteLine("sent '$Argument' to $hwnd") }
                exit 0
            }
        }
        Fail "could not bring the operator window to the foreground within 10 s. $(Describe-Windows $ProcessId)" 5
    }
    default { Fail "unknown action '$Action'" 64 }
}
