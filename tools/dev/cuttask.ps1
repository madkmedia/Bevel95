# cuttask.ps1 <unpressed.png> <ux> <uy> <pressed.png> <px> <py> <name> <doc>
# Cuts a 16x16 task-button icon from a capture where its button is raised (plain face gray
# #C0C0C0 behind it: gray reachable from the edge = see-through), then checks it against a
# capture where the button is pressed (a checkerboard of gray and white behind it): every
# opaque pixel must be the same there, every see-through one must show the checkerboard.
# Prints the mismatches (none expected) and the picture as a `Rows` constant for bevel-art
# (art/src/taskbar.rs, where Windows' pictures live; give it a line in the module's ENTRIES).
param([string]$Up, [int]$ux, [int]$uy, [string]$Down, [int]$px, [int]$py, [string]$Name, [string]$Doc, [string]$Face = "C0C0C0")
$code = @'
using System; using System.Drawing; using System.Text; using System.Collections.Generic;
public static class CutTask {
  public static string Run(string up, int ux, int uy, string down, int dx, int dy, string name, string doc, int face) {
    const int n = 16; const int white = 0xFFFFFF;
    var a = new int[n, n]; var b = new int[n, n];
    int parityWhite;
    using (var u = new Bitmap(up)) for (int y = 0; y < n; y++) for (int x = 0; x < n; x++) a[x, y] = u.GetPixel(ux + x, uy + y).ToArgb() & 0xFFFFFF;
    using (var d = new Bitmap(down)) {
      for (int y = 0; y < n; y++) for (int x = 0; x < n; x++) b[x, y] = d.GetPixel(dx + x, dy + y).ToArgb() & 0xFFFFFF;
      // the checkerboard's phase, from the padding left of the icon
      int c0 = d.GetPixel(dx - 1, dy + 8).ToArgb() & 0xFFFFFF;
      parityWhite = c0 == white ? ((dx - 1 + dy + 8) & 1) : ((dx + dy + 8) & 1);
    }
    var clear = new bool[n, n]; var q = new Queue<int[]>();
    for (int i = 0; i < n; i++) foreach (var p in new[]{ new[]{i, 0}, new[]{i, n - 1}, new[]{0, i}, new[]{n - 1, i} }) if (a[p[0], p[1]] == face && !clear[p[0], p[1]]) { clear[p[0], p[1]] = true; q.Enqueue(p); }
    while (q.Count > 0) { var p = q.Dequeue(); foreach (var o in new[]{ new[]{1,0}, new[]{-1,0}, new[]{0,1}, new[]{0,-1} }) { int x = p[0] + o[0], y = p[1] + o[1]; if (x < 0 || y < 0 || x >= n || y >= n || clear[x, y] || a[x, y] != face) continue; clear[x, y] = true; q.Enqueue(new[]{x, y}); } }
    var report = new StringBuilder(); int bad = 0;
    for (int y = 0; y < n; y++) for (int x = 0; x < n; x++) {
      int expect = clear[x, y] ? ((((dx + x) + (dy + y)) & 1) == parityWhite ? white : face) : a[x, y];
      if (b[x, y] != expect) { bad++; report.AppendFormat("  mismatch at ({0},{1}): raised {2:X6}, pressed {3:X6}, expected {4:X6}{5}\n", x, y, a[x, y], b[x, y], expect, clear[x, y] ? " (see-through)" : ""); }
    }
    const string letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    var pal = new List<int>(); var rows = new List<string>();
    for (int y = 0; y < n; y++) { var sb = new StringBuilder(); for (int x = 0; x < n; x++) { if (clear[x, y]) { sb.Append('.'); continue; } int c = a[x, y]; int k = pal.IndexOf(c); if (k < 0) { pal.Add(c); k = pal.Count - 1; } sb.Append(letters[k]); } rows.Add(sb.ToString()); }
    var o2 = new StringBuilder();
    o2.AppendFormat("// {0}: {1} mismatches against the pressed button\n", name, bad);
    o2.Append(report);
    o2.AppendFormat("/// {0}\n#[rustfmt::skip]\nconst {1}: Rows = Rows {{\n    w: 16,\n    h: 16,\n    palette: &[\n", doc, name);
    for (int k = 0; k < pal.Count; k++) o2.AppendFormat("        (b'{0}', 0x{1:X6}),\n", letters[k], pal[k]);
    o2.Append("    ],\n    rows: &[\n");
    foreach (var r in rows) o2.AppendFormat("        \"{0}\",\n", r);
    o2.Append("    ],\n};\n");
    return o2.ToString();
  }
}
'@
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition $code
[CutTask]::Run($Up, $ux, $uy, $Down, $px, $py, $Name, $Doc, [Convert]::ToInt32($Face, 16))

