# cutmenu.ps1 <normal.png> <selected.png> <x> <y> <name> <doc> [-Face C0C0C0] [-Selection 000080]
# Cuts a 16x16 menu icon at (x, y) from two captures of the same menu: one where its entry is
# not selected (face color behind the icon) and one where it is (selection color behind it).
# A pixel that is the face color in the first and the selection color in the second is
# see-through; any other pixel must be the same in both (else it is reported: Windows would
# have drawn the selected icon differently). Also checks that the columns just left and right
# of the icon are background in both captures (the position is right).
# Windows 98 fills the icon's 16x16 cell with colors one step off the menu's (BFBFBF and
# 00007F): pass those as -CellFace and -CellSelection.
# Prints the picture as a `Rows` constant for bevel-art (art/src, where Windows' pictures live;
# give it a line in its module's ENTRIES, and the core looks it up).
param([string]$Normal, [string]$Selected, [int]$X, [int]$Y, [string]$Name, [string]$Doc,
    [string]$Face = "C0C0C0", [string]$Selection = "000080", [string]$CellFace = "", [string]$CellSelection = "")
if (-not $CellFace) { $CellFace = $Face }
if (-not $CellSelection) { $CellSelection = $Selection }
$code = @'
using System; using System.Drawing; using System.Text; using System.Collections.Generic;
public static class CutMenu {
  public static string Run(string normal, string selected, int x0, int y0, string name, string doc, int face, int sel, int cellFace, int cellSel) {
    const int n = 16;
    var a = new int[n + 2, n]; var b = new int[n + 2, n];
    using (var u = new Bitmap(normal)) for (int y = 0; y < n; y++) for (int x = 0; x < n + 2; x++) a[x, y] = u.GetPixel(x0 - 1 + x, y0 + y).ToArgb() & 0xFFFFFF;
    using (var d = new Bitmap(selected)) for (int y = 0; y < n; y++) for (int x = 0; x < n + 2; x++) b[x, y] = d.GetPixel(x0 - 1 + x, y0 + y).ToArgb() & 0xFFFFFF;
    var report = new StringBuilder(); int bad = 0;
    for (int y = 0; y < n; y++) foreach (int x in new[] { 0, n + 1 })
      if (a[x, y] != face || b[x, y] != sel) { bad++; report.AppendFormat("  padding ({0},{1}) is {2:X6} / {3:X6}, not background\n", x - 1, y, a[x, y], b[x, y]); }
    const string letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    var pal = new List<int>(); var rows = new List<string>();
    for (int y = 0; y < n; y++) {
      var sb = new StringBuilder();
      for (int x = 1; x <= n; x++) {
        int p = a[x, y], q = b[x, y];
        if (p == cellFace && q == cellSel) { sb.Append('.'); continue; }
        if (p != q) { bad++; report.AppendFormat("  ({0},{1}): {2:X6} normal, {3:X6} selected\n", x - 1, y, p, q); }
        int k = pal.IndexOf(p); if (k < 0) { pal.Add(p); k = pal.Count - 1; }
        sb.Append(letters[k]);
      }
      rows.Add(sb.ToString());
    }
    var o = new StringBuilder();
    o.AppendFormat("// {0}: {1} problems\n", name, bad);
    o.Append(report);
    o.AppendFormat("/// {0}\n#[rustfmt::skip]\nconst {1}: Rows = Rows {{\n    w: 16,\n    h: 16,\n    palette: &[\n", doc, name);
    for (int k = 0; k < pal.Count; k++) o.AppendFormat("        (b'{0}', 0x{1:X6}),\n", letters[k], pal[k]);
    o.Append("    ],\n    rows: &[\n");
    foreach (var r in rows) o.AppendFormat("        \"{0}\",\n", r);
    o.Append("    ],\n};\n");
    return o.ToString();
  }
}
'@
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition $code
[CutMenu]::Run((Resolve-Path $Normal).Path, (Resolve-Path $Selected).Path, $X, $Y, $Name, $Doc,
    [Convert]::ToInt32($Face, 16), [Convert]::ToInt32($Selection, 16),
    [Convert]::ToInt32($CellFace, 16), [Convert]::ToInt32($CellSelection, 16))
