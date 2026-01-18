Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

function Get-WindowScreenshot {
    param (
        [string]$ProcessName,
        [string]$OutputPath
    )

    $proc = Get-Process | Where-Object {$_.ProcessName -like $ProcessName} | Select-Object -First 1
    
    if (-not $proc) {
        Write-Output "Process '$ProcessName' not found."
        return
    }

    $hwnd = $proc.MainWindowHandle
    if ($hwnd -eq 0) {
        Write-Output "Process '$ProcessName' has no main window handle."
        return
    }

    $code = @'
    using System;
    using System.Runtime.InteropServices;
    
    namespace Native {
        public class Win32Utils {
            [DllImport("user32.dll")]
            public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
            [DllImport("user32.dll")]
            public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, int nFlags);

            [StructLayout(LayoutKind.Sequential)]
            public struct RECT {
                public int Left;
                public int Top;
                public int Right;
                public int Bottom;
            }
        }
    }
'@

    Add-Type -TypeDefinition $code -ErrorAction SilentlyContinue

    $rect = New-Object Native.Win32Utils+RECT
    [Native.Win32Utils]::GetWindowRect($hwnd, [ref]$rect)

    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    
    if ($width -le 0 -or $height -le 0) {
        Write-Output "Window dimensions invalid: ${width}x${height}"
        return
    }

    $bmp = New-Object System.Drawing.Bitmap($width, $height)
    $graphics = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $graphics.GetHdc()

    if ([Native.Win32Utils]::PrintWindow($hwnd, $hdc, 0)) {
        $graphics.ReleaseHdc($hdc)
        $bmp.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
        Write-Output "Screenshot saved to $OutputPath"
    } else {
        $graphics.ReleaseHdc($hdc)
        Write-Output "Failed to capture window."
    }
    
    $graphics.Dispose()
    $bmp.Dispose()
}

Get-WindowScreenshot -ProcessName "ultrawave_standalone" -OutputPath "C:\Users\jeffm\.gemini\antigravity\brain\ea97e0ba-639e-4acc-b9c8-221e00823520\ultrawave_gui.png"
