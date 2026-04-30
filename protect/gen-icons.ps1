$dir = "$PSScriptRoot\src-tauri\icons"
New-Item -ItemType Directory -Force $dir | Out-Null
Write-Host "Creating icons in $dir ..."

Add-Type -AssemblyName System.Drawing

function New-Icon($size, $name) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::FromArgb(255, 233, 69, 96))
    $g.Dispose()
    $bmp.Save("$dir\$name.png", [System.Drawing.Imaging.ImageFormat]::Png)
    if ($name -eq "32x32") {
        $ico = [System.Drawing.Icon]::FromHandle($bmp.GetHicon())
        $fs = [System.IO.File]::OpenWrite("$dir\icon.ico")
        $ico.Save($fs)
        $fs.Close()
    }
    $bmp.Dispose()
}

New-Icon 32 "32x32"
New-Icon 128 "128x128"
New-Icon 256 "128x128@2x"

Write-Host "Done: icon.ico + 3 PNGs created"
