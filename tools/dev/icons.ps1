# Dumps the icons stored in a Windows program file (.exe, .dll, .cpl; 32-bit PE) as PNGs, pixel
# for pixel as stored: every image of every icon group, its see-through pixels (AND mask) kept
# as transparency. Also writes <file>-sheet.png, all 16x16 images side by side at 4x with their
# group ids, for looking. Used to read the old Windows' network and battery icons out of the
# files copied from the reference PCs (private\reference\refs\files).
#
#   powershell -NoProfile -File tools\dev\icons.ps1 <file> <output folder> [-Size 16]
param([Parameter(Mandatory)][string]$File, [Parameter(Mandatory)][string]$Out, [int]$Size = 16)

Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Runtime.InteropServices;

public static class IconDump {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr LoadLibraryExW(string path, IntPtr file, uint flags);
    [DllImport("kernel32.dll")] static extern bool FreeLibrary(IntPtr module);
    delegate bool EnumNameProc(IntPtr module, IntPtr type, IntPtr name, IntPtr param);
    [DllImport("kernel32.dll")] static extern bool EnumResourceNamesW(IntPtr module, IntPtr type, EnumNameProc proc, IntPtr param);
    [DllImport("kernel32.dll")] static extern IntPtr FindResourceW(IntPtr module, IntPtr name, IntPtr type);
    [DllImport("kernel32.dll")] static extern IntPtr LoadResource(IntPtr module, IntPtr res);
    [DllImport("kernel32.dll")] static extern IntPtr LockResource(IntPtr data);
    [DllImport("kernel32.dll")] static extern uint SizeofResource(IntPtr module, IntPtr res);

    static byte[] Resource(IntPtr module, IntPtr name, int type) {
        IntPtr r = FindResourceW(module, name, (IntPtr)type);
        if (r == IntPtr.Zero) return null;
        uint size = SizeofResource(module, r);
        IntPtr p = LockResource(LoadResource(module, r));
        byte[] b = new byte[size];
        Marshal.Copy(p, b, 0, (int)size);
        return b;
    }

    // One RT_ICON image (BITMAPINFOHEADER, palette, XOR bits, AND mask) as a 32-bit bitmap.
    public static Bitmap Decode(byte[] d) {
        int w = BitConverter.ToInt32(d, 4), h = BitConverter.ToInt32(d, 8) / 2;
        int bpp = BitConverter.ToUInt16(d, 14);
        int colors = BitConverter.ToInt32(d, 32);
        int hdr = BitConverter.ToInt32(d, 0);
        if (colors == 0 && bpp <= 8) colors = 1 << bpp;
        int pal = hdr;
        int xor = pal + colors * 4;
        int xorStride = ((w * bpp + 31) / 32) * 4;
        int and = xor + xorStride * h;
        int andStride = ((w + 31) / 32) * 4;
        var bmp = new Bitmap(w, h, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
        for (int y = 0; y < h; y++) {
            int row = h - 1 - y;
            for (int x = 0; x < w; x++) {
                int r, g, b, a = 255;
                if (bpp <= 8) {
                    int bit = x * bpp;
                    int v = (d[xor + row * xorStride + bit / 8] >> (8 - bpp - bit % 8)) & ((1 << bpp) - 1);
                    b = d[pal + v * 4]; g = d[pal + v * 4 + 1]; r = d[pal + v * 4 + 2];
                } else if (bpp == 24) {
                    int o = xor + row * xorStride + x * 3; b = d[o]; g = d[o + 1]; r = d[o + 2];
                } else {
                    int o = xor + row * xorStride + x * 4; b = d[o]; g = d[o + 1]; r = d[o + 2]; a = d[o + 3];
                }
                bool clear = and + row * andStride + x / 8 < d.Length && ((d[and + row * andStride + x / 8] >> (7 - x % 8)) & 1) == 1;
                if (bpp < 32 && clear) a = 0;
                bmp.SetPixel(x, y, Color.FromArgb(a, r, g, b));
            }
        }
        return bmp;
    }

    // Every icon group: (group id or name, width, height, bits per pixel, icon id, image).
    public static List<object[]> All(string path) {
        var list = new List<object[]>();
        IntPtr m = LoadLibraryExW(path, IntPtr.Zero, 0x2 | 0x20); // AS_DATAFILE | AS_IMAGE_RESOURCE
        if (m == IntPtr.Zero) throw new Exception("cannot load " + path + " (" + Marshal.GetLastWin32Error() + ")");
        var names = new List<IntPtr>();
        var keep = new List<string>();
        EnumResourceNamesW(m, (IntPtr)14, (mod, t, n, p) => {
            if (((long)n >> 16) == 0) names.Add(n);
            else { string s = Marshal.PtrToStringUni(n); keep.Add(s); names.Add(Marshal.StringToHGlobalUni(s)); }
            return true;
        }, IntPtr.Zero);
        foreach (var n in names) {
            byte[] g = Resource(m, n, 14);
            if (g == null) continue;
            string gid = ((long)n >> 16) == 0 ? ((long)n).ToString() : Marshal.PtrToStringUni(n);
            int count = BitConverter.ToUInt16(g, 4);
            for (int i = 0; i < count; i++) {
                int o = 6 + i * 14;
                int w = g[o] == 0 ? 256 : g[o], h = g[o + 1] == 0 ? 256 : g[o + 1];
                int bpp = BitConverter.ToUInt16(g, o + 6);
                int id = BitConverter.ToUInt16(g, o + 12);
                byte[] img = Resource(m, (IntPtr)id, 3);
                if (img == null || BitConverter.ToInt32(img, 0) != 40) continue; // PNG-compressed: not in old files
                if (bpp == 0) bpp = BitConverter.ToUInt16(img, 14);
                list.Add(new object[] { gid, w, h, bpp, id, Decode(img) });
            }
        }
        FreeLibrary(m);
        return list;
    }
}
'@

New-Item -ItemType Directory -Force $Out | Out-Null
$base = [IO.Path]::GetFileNameWithoutExtension($File).ToLower()
$all = [IconDump]::All((Resolve-Path $File).Path)
$small = @($all | Where-Object { $_[1] -eq $Size })
foreach ($e in $all) {
    $name = "{0}-{1}-{2}x{3}-{4}bit.png" -f $base, $e[0], $e[1], $e[2], $e[3]
    $e[5].Save((Join-Path $Out $name), [Drawing.Imaging.ImageFormat]::Png)
}
# contact sheet of the $Size images at 4x on a checkerboard, labeled with group id and depth
$cell = $Size * 4 + 8; $cols = 8; $rows = [Math]::Max(1, [Math]::Ceiling($small.Count / $cols))
$sheet = New-Object Drawing.Bitmap ($cols * $cell), ($rows * ($cell + 14))
$gfx = [Drawing.Graphics]::FromImage($sheet)
$gfx.InterpolationMode = 'NearestNeighbor'; $gfx.PixelOffsetMode = 'Half'
$gfx.Clear([Drawing.Color]::FromArgb(0, 128, 128))
$font = New-Object Drawing.Font 'Segoe UI', 7
for ($i = 0; $i -lt $small.Count; $i++) {
    $x = ($i % $cols) * $cell + 4; $y = [Math]::Floor($i / $cols) * ($cell + 14) + 2
    $gfx.DrawImage($small[$i][5], (New-Object Drawing.Rectangle $x, $y, ($Size * 4), ($Size * 4)))
    $gfx.DrawString(("{0} {1}b" -f $small[$i][0], $small[$i][3]), $font, [Drawing.Brushes]::White, $x, ($y + $Size * 4 + 1))
}
$gfx.Dispose()
$sheet.Save((Join-Path $Out "$base-sheet.png"), [Drawing.Imaging.ImageFormat]::Png)
"{0}: {1} images, {2} at {3}x{3}" -f $base, $all.Count, $small.Count, $Size
