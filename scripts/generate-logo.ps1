$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent $PSScriptRoot
$assets = Join-Path $root 'assets'
New-Item -ItemType Directory -Path $assets -Force | Out-Null
$bitmap = New-Object System.Drawing.Bitmap 512, 512
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
$graphics.Clear([System.Drawing.Color]::Transparent)
$graphics.TranslateTransform(0, -20)
$ink = [System.Drawing.ColorTranslator]::FromHtml('#263A36')
$pen = New-Object System.Drawing.Pen $ink, 22
$pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
$pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
$orbit = New-Object System.Drawing.Drawing2D.GraphicsPath
$orbit.AddBezier(138, 294, 18, 318, 34, 432, 208, 444)
$orbit.AddBezier(208, 444, 280, 450, 358, 430, 407, 401)
$graphics.DrawPath($pen, $orbit)
$back = New-Object System.Drawing.Drawing2D.GraphicsPath
$back.AddBezier(375, 294, 440, 304, 469, 341, 438, 373)
$graphics.DrawPath($pen, $back)
$brush = New-Object System.Drawing.SolidBrush $ink
$arrow = [System.Drawing.PointF[]]@(
    [System.Drawing.PointF]::new(365, 374),
    [System.Drawing.PointF]::new(426, 397),
    [System.Drawing.PointF]::new(392, 455),
    [System.Drawing.PointF]::new(375, 445),
    [System.Drawing.PointF]::new(396, 409),
    [System.Drawing.PointF]::new(358, 393)
)
$graphics.FillPolygon($brush, $arrow)
$brush.Dispose()
$faces = @(
    @{ Color = '#D6F2E6'; Points = @(140, 142, 284, 102, 396, 178, 252, 220) },
    @{ Color = '#395D53'; Points = @(140, 142, 252, 220, 252, 364, 140, 288) },
    @{ Color = '#79CBAA'; Points = @(252, 220, 396, 178, 396, 322, 252, 364) }
)
$outline = New-Object System.Drawing.Pen $ink, 10
$outline.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
foreach ($face in $faces) {
    $points = for ($index = 0; $index -lt $face.Points.Count; $index += 2) {
        [System.Drawing.PointF]::new($face.Points[$index], $face.Points[$index + 1])
    }
    $fill = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml($face.Color))
    $graphics.FillPolygon($fill, [System.Drawing.PointF[]]$points)
    $graphics.DrawPolygon($outline, [System.Drawing.PointF[]]$points)
    $fill.Dispose()
}
$letters = New-Object System.Drawing.Drawing2D.GraphicsPath
$family = New-Object System.Drawing.FontFamily 'Segoe UI'
$letters.AddString('3D', $family, [int][System.Drawing.FontStyle]::Bold, 66, [System.Drawing.PointF]::new(268, 300), [System.Drawing.StringFormat]::GenericTypographic)
$transform = New-Object System.Drawing.Drawing2D.Matrix 1, -0.29, 0, 1, 0, 0
$letters.Transform($transform)
$brush = New-Object System.Drawing.SolidBrush $ink
$graphics.FillPath($brush, $letters)
$bitmap.Save((Join-Path $assets '3dviewer.png'), [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Save((Join-Path $root 'vscode-extension\icon.png'), [System.Drawing.Imaging.ImageFormat]::Png)
$sizes = @(16, 24, 32, 48, 64, 128, 256)
$frames = foreach ($size in $sizes) {
    $frame = New-Object System.Drawing.Bitmap $size, $size
    $canvas = [System.Drawing.Graphics]::FromImage($frame)
    $canvas.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $canvas.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $canvas.DrawImage($bitmap, 0, 0, $size, $size)
    $stream = New-Object System.IO.MemoryStream
    $frame.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $bytes = $stream.ToArray()
    $canvas.Dispose(); $frame.Dispose(); $stream.Dispose()
    @{ Size = $size; Bytes = $bytes }
}
$file = [System.IO.File]::Create((Join-Path $assets '3dviewer.ico'))
$writer = New-Object System.IO.BinaryWriter $file
try {
    $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$frames.Count)
    $offset = 6 + 16 * $frames.Count
    foreach ($frame in $frames) {
        $dimension = if ($frame.Size -eq 256) { 0 } else { $frame.Size }
        $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
        $writer.Write([byte]0); $writer.Write([byte]0)
        $writer.Write([uint16]1); $writer.Write([uint16]32)
        $writer.Write([uint32]$frame.Bytes.Length); $writer.Write([uint32]$offset)
        $offset += $frame.Bytes.Length
    }
    foreach ($frame in $frames) { $writer.Write([byte[]]$frame.Bytes) }
} finally {
    $writer.Dispose(); $file.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
    $pen.Dispose(); $outline.Dispose(); $orbit.Dispose(); $back.Dispose()
    $letters.Dispose(); $family.Dispose(); $transform.Dispose(); $brush.Dispose()
}
Write-Output 'Generated 512px transparent logo and seven-resolution Windows icon.'